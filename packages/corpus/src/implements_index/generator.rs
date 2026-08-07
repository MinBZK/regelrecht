//! The index generator, as a library so its contract is testable.
//!
//! The `implements-indexer` binary is a thin wrapper: it parses argv,
//! calls [`run`], prints, and maps the [`Outcome`] to an exit code.
//!
//! ## Unparseable laws
//!
//! A harvested corpus always contains some malformed YAML (at the time of
//! writing, 146 of 22.468 files on `regelrecht-corpus@development`). The
//! generator therefore **omits** those paths from the index instead of
//! refusing to emit one. Omitting is safe by construction: the consumer
//! treats a missing key as "not known, fetch this law and parse it
//! per-request" ([`crate::implements_index`] contract), which is exactly
//! the behaviour without any index. Recording them as "implements nothing"
//! would be the unsafe option, and that is what the scan never does.
//!
//! A *systematic* failure (wrong root, corrupt checkout, a schema change
//! that breaks every file) is a different thing, and would degrade the
//! index to near-useless while still looking successful. So the generator
//! fails when more than [`MAX_UNPARSEABLE_RATIO`] of the scanned files fail
//! to parse.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{
    scan_tree, ImplementsIndex, ScanFailure, IMPLEMENTS_INDEX_FILENAME, IMPLEMENTS_INDEX_VERSION,
};

/// Default subtree to scan, relative to the repo root.
pub const DEFAULT_SCAN_ROOT: &str = "regulation";

/// Fraction of scanned files that may fail to parse before the run is
/// treated as a systematic failure rather than ordinary corpus noise.
pub const MAX_UNPARSEABLE_RATIO: f64 = 0.05;

/// What the generator was asked to do.
#[derive(Debug, Clone)]
pub struct Args {
    /// Root of the corpus checkout (must be a git work tree).
    pub repo_root: PathBuf,
    /// Repo-relative subtree to scan.
    pub scan_root: String,
    /// Verify the committed index instead of writing one.
    pub check: bool,
}

impl Args {
    /// Generate against `repo_root`, scanning [`DEFAULT_SCAN_ROOT`].
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            scan_root: DEFAULT_SCAN_ROOT.to_string(),
            check: false,
        }
    }
}

/// Files the scan could not parse; carried on every successful outcome so
/// the caller can report them even when the run succeeds.
pub type Skipped = Vec<ScanFailure>;

/// Why a `--check` run considers the committed index unusable.
#[derive(Debug)]
pub enum Drift {
    /// No index file at the repo root.
    Missing { path: PathBuf },
    /// The committed index differs from what this checkout produces.
    OutOfDate {
        path: PathBuf,
        committed_tree: String,
        checkout_tree: String,
    },
}

/// Result of a successful generator run.
#[derive(Debug)]
pub enum Outcome {
    /// Index written to disk.
    Wrote {
        path: PathBuf,
        files: usize,
        declaring: usize,
        bytes: usize,
        tree_sha: String,
        skipped: Skipped,
    },
    /// `--check`: the committed index matches this checkout.
    InSync {
        files: usize,
        declaring: usize,
        tree_sha: String,
        skipped: Skipped,
    },
    /// `--check`: it does not.
    Drifted(Drift),
}

/// Run a git command in `repo_root`, returning trimmed stdout.
fn git(repo_root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Generate or verify the index. `Err` is an operational failure (dirty
/// tree, git error, unreadable index, systematic parse breakage); the
/// caller maps it to exit code 2.
/// Collapse a scan root to the form the consumer compares and strips
/// with: no leading or trailing slash, no empty interior segment.
fn normalise_root(root: &str) -> String {
    root.split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn run(args: &Args) -> Result<Outcome, String> {
    // An index over the repo root contains its own file, so writing it
    // changes the tree sha it just recorded and no consumer would ever
    // accept it. Refuse here rather than in the CLI's argument parsing:
    // `Args` is public, so a library caller reaches this too.
    if args.scan_root.trim_matches('/').is_empty() {
        return Err(
            "scan root must name a subtree: an index rooted at the repo root contains \
             its own file and so invalidates the tree sha it records"
                .to_string(),
        );
    }

    // The tree sha is only honest about what was scanned when the subtree
    // has no uncommitted changes. Refuse a dirty subtree — no bypass.
    let dirty = git(
        &args.repo_root,
        &["status", "--porcelain", "--", &args.scan_root],
    )?;
    if !dirty.is_empty() {
        return Err(format!(
            "scan subtree '{}' has uncommitted changes; commit or stash them first \
             (the recorded tree sha must describe exactly what was scanned):\n{dirty}",
            args.scan_root
        ));
    }

    let tree_sha = git(
        &args.repo_root,
        &["rev-parse", &format!("HEAD:{}", args.scan_root)],
    )
    .map_err(|e| format!("cannot resolve tree sha of '{}': {e}", args.scan_root))?;

    let outcome =
        scan_tree(&args.repo_root, &args.scan_root).map_err(|e| format!("scan failed: {e}"))?;

    let scanned = outcome.files.len() + outcome.failures.len();
    let failed = outcome.failures.len();
    #[allow(clippy::cast_precision_loss)]
    let ratio = if scanned == 0 {
        0.0
    } else {
        failed as f64 / scanned as f64
    };
    if ratio > MAX_UNPARSEABLE_RATIO {
        return Err(format!(
            "{failed} of {scanned} files under '{}' failed to parse ({:.1}%, limit {:.0}%) — \
             that is a systematic failure, not corpus noise; refusing to emit an index \
             that would omit them all",
            args.scan_root,
            ratio * 100.0,
            MAX_UNPARSEABLE_RATIO * 100.0
        ));
    }

    let declaring = outcome.files.values().filter(|v| !v.is_empty()).count();
    let index = ImplementsIndex {
        version: IMPLEMENTS_INDEX_VERSION,
        // Normalised, because the consumer compares this root against a
        // source root and strips it as a path prefix. A stored `foo//bar`
        // or `/foo/bar` would pass the coverage check and then strip
        // nothing, projecting to an empty map that reads as "this corpus
        // holds no laws".
        root: normalise_root(&args.scan_root),
        tree_sha,
        files: outcome.files,
    };
    let index_path = args.repo_root.join(IMPLEMENTS_INDEX_FILENAME);

    if args.check {
        let committed = match std::fs::read_to_string(&index_path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Outcome::Drifted(Drift::Missing { path: index_path }));
            }
            Err(e) => return Err(format!("cannot read {}: {e}", index_path.display())),
        };
        let committed = ImplementsIndex::parse(&committed)
            .map_err(|e| format!("committed index is unreadable: {e}"))?;
        if committed != index {
            return Ok(Outcome::Drifted(Drift::OutOfDate {
                path: index_path,
                committed_tree: committed.tree_sha,
                checkout_tree: index.tree_sha,
            }));
        }
        return Ok(Outcome::InSync {
            files: index.files.len(),
            declaring,
            tree_sha: index.tree_sha,
            skipped: outcome.failures,
        });
    }

    let json = index.to_json();
    std::fs::write(&index_path, &json)
        .map_err(|e| format!("cannot write {}: {e}", index_path.display()))?;
    Ok(Outcome::Wrote {
        path: index_path,
        files: index.files.len(),
        declaring,
        bytes: json.len(),
        tree_sha: index.tree_sha,
        skipped: outcome.failures,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn run_git(root: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn commit_all(root: &Path, message: &str) {
        run_git(root, &["add", "-A"]);
        // `--no-verify`: the ambient global git config may install a
        // template hooks dir, which would run this repo's hooks inside a
        // throwaway fixture repo.
        run_git(root, &["commit", "--no-verify", "-m", message]);
    }

    /// A git repo with `n_ok` parseable laws plus `n_broken` malformed
    /// ones, committed.
    fn fixture(n_ok: usize, n_broken: usize) -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        // Empty template dir: keeps the ambient `init.templateDir` from
        // injecting hooks into the fixture.
        let empty = root.join(".empty-template");
        fs::create_dir_all(&empty).unwrap();
        run_git(
            root,
            &[
                "init",
                "--initial-branch=development",
                &format!("--template={}", empty.display()),
            ],
        );
        run_git(root, &["config", "user.name", "test"]);
        run_git(root, &["config", "user.email", "test@test.nl"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);

        for i in 0..n_ok {
            write(
                root,
                &format!("regulation/nl/wet/wet_{i}/2025-01-01.yaml"),
                &format!("$id: wet_{i}\narticles: []\n"),
            );
        }
        for i in 0..n_broken {
            write(
                root,
                &format!("regulation/nl/wet/kapot_{i}/2025-01-01.yaml"),
                "name: \"unterminated\nfoo: bar\n",
            );
        }
        commit_all(root, "seed");
        dir
    }

    #[test]
    fn the_recorded_root_is_normalised() {
        assert_eq!(normalise_root("regulation"), "regulation");
        assert_eq!(normalise_root("/regulation/nl/"), "regulation/nl");
        assert_eq!(normalise_root("regulation//nl"), "regulation/nl");
    }

    #[test]
    fn a_repo_root_scan_is_refused() {
        let dir = TempDir::new().unwrap();
        for root in ["", "/"] {
            let args = Args {
                repo_root: dir.path().to_path_buf(),
                scan_root: root.to_string(),
                check: false,
            };
            let err = run(&args).expect_err("a repo-root index can never be served");
            assert!(err.contains("subtree"), "the message must say why: {err}");
        }
    }

    #[test]
    fn writes_an_index_for_a_clean_checkout() {
        let dir = fixture(3, 0);
        let outcome = run(&Args::new(dir.path())).unwrap();
        let Outcome::Wrote { path, files, .. } = outcome else {
            panic!("expected Wrote, got {outcome:?}");
        };
        assert_eq!(files, 3);
        assert!(path.exists());
    }

    #[test]
    fn a_few_unparseable_laws_are_skipped_not_fatal() {
        // 1 of 21 == 4.8%, just under the limit — this is the shape of the
        // real corpus, where fail-closed would mean no index ever exists.
        let dir = fixture(20, 1);
        let outcome = run(&Args::new(dir.path())).unwrap();
        let Outcome::Wrote { files, skipped, .. } = outcome else {
            panic!("expected Wrote, got {outcome:?}");
        };
        assert_eq!(files, 20, "the broken law must not be indexed as empty");
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].path.contains("kapot_0"));

        // And the broken path is absent, so the consumer falls through to a
        // per-law fetch rather than believing it implements nothing.
        let raw = fs::read_to_string(dir.path().join(IMPLEMENTS_INDEX_FILENAME)).unwrap();
        let index = ImplementsIndex::parse(&raw).unwrap();
        assert!(!index.files.keys().any(|k| k.contains("kapot_0")));
    }

    #[test]
    fn systematic_parse_breakage_is_fatal() {
        let dir = fixture(5, 5);
        let err = run(&Args::new(dir.path())).unwrap_err();
        assert!(err.contains("systematic failure"), "unexpected: {err}");
        assert!(!dir.path().join(IMPLEMENTS_INDEX_FILENAME).exists());
    }

    #[test]
    fn check_passes_directly_after_generation() {
        let dir = fixture(2, 0);
        run(&Args::new(dir.path())).unwrap();
        commit_all(dir.path(), "add index");

        let args = Args {
            check: true,
            ..Args::new(dir.path())
        };
        let outcome = run(&args).unwrap();
        assert!(matches!(outcome, Outcome::InSync { files: 2, .. }));
    }

    #[test]
    fn check_reports_drift_after_a_corpus_commit() {
        let dir = fixture(2, 0);
        run(&Args::new(dir.path())).unwrap();
        commit_all(dir.path(), "add index");

        write(
            dir.path(),
            "regulation/nl/wet/wet_nieuw/2025-01-01.yaml",
            "$id: wet_nieuw\narticles: []\n",
        );
        commit_all(dir.path(), "add a law without regenerating");

        let args = Args {
            check: true,
            ..Args::new(dir.path())
        };
        let outcome = run(&args).unwrap();
        let Outcome::Drifted(Drift::OutOfDate {
            committed_tree,
            checkout_tree,
            ..
        }) = outcome
        else {
            panic!("expected OutOfDate, got {outcome:?}");
        };
        assert_ne!(committed_tree, checkout_tree);
    }

    #[test]
    fn check_reports_a_missing_index() {
        let dir = fixture(1, 0);
        let args = Args {
            check: true,
            ..Args::new(dir.path())
        };
        assert!(matches!(
            run(&args).unwrap(),
            Outcome::Drifted(Drift::Missing { .. })
        ));
    }

    #[test]
    fn a_dirty_scan_subtree_is_refused() {
        let dir = fixture(1, 0);
        write(
            dir.path(),
            "regulation/nl/wet/wet_0/2025-01-01.yaml",
            "$id: wet_0\narticles: []\n# uncommitted\n",
        );
        let err = run(&Args::new(dir.path())).unwrap_err();
        assert!(err.contains("uncommitted changes"), "unexpected: {err}");
    }

    #[test]
    fn a_missing_scan_root_is_an_error_not_an_empty_index() {
        let dir = fixture(1, 0);
        let args = Args {
            scan_root: "geen-regulation".to_string(),
            ..Args::new(dir.path())
        };
        assert!(run(&args).is_err());
    }
}

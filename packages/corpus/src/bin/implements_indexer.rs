//! Generator for the precomputed implements-index
//! (`implements-index.json`) committed at the root of the corpus repo.
//!
//! Meant to be run by CI **in the corpus repo** against its own clean
//! checkout, on every branch that carries corpus content:
//!
//! ```text
//! # regenerate (writes <checkout>/implements-index.json):
//! implements-indexer <checkout>
//!
//! # verify the committed index matches the checkout (PR gate):
//! implements-indexer --check <checkout>
//! ```
//!
//! The generator refuses to run on a dirty scan subtree (the recorded
//! tree sha would not describe what was scanned) and refuses to emit an
//! index when any YAML file under the subtree fails to parse — a parse
//! failure recorded as "implements nothing" in a committed artefact would
//! be permanent and invisible.
//!
//! Exit codes: 0 = success / in sync; 1 = `--check` drift or missing
//! index; 2 = any other failure (parse errors, dirty tree, git errors).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use regelrecht_corpus::implements_index::{
    scan_tree, ImplementsIndex, IMPLEMENTS_INDEX_FILENAME, IMPLEMENTS_INDEX_VERSION,
};

const DEFAULT_SCAN_ROOT: &str = "regulation";

struct Args {
    repo_root: PathBuf,
    scan_root: String,
    check: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut repo_root = None;
    let mut scan_root = DEFAULT_SCAN_ROOT.to_string();
    let mut check = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--root" => {
                scan_root = args
                    .next()
                    .ok_or_else(|| "--root requires a value".to_string())?;
            }
            "--help" | "-h" => {
                return Err(format!(
                    "usage: implements-indexer [--check] [--root <subtree>] <repo-root>\n\
                     \n\
                     Scans <repo-root>/<subtree> (default: {DEFAULT_SCAN_ROOT}) for \
                     .yaml/.yml law files and writes {IMPLEMENTS_INDEX_FILENAME} at \
                     <repo-root>. With --check, verifies the committed index instead."
                ));
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option: {other}"));
            }
            other => {
                if repo_root.replace(PathBuf::from(other)).is_some() {
                    return Err("expected exactly one <repo-root> argument".to_string());
                }
            }
        }
    }

    Ok(Args {
        repo_root: repo_root.ok_or_else(|| "missing <repo-root> argument".to_string())?,
        scan_root,
        check,
    })
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

fn run(args: &Args) -> Result<ExitCode, String> {
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

    if !outcome.failures.is_empty() {
        eprintln!(
            "error: {} file(s) failed to parse — refusing to emit an index that would \
             record them as \"implements nothing\":",
            outcome.failures.len()
        );
        for failure in &outcome.failures {
            eprintln!("  {}: {}", failure.path, failure.error);
        }
        return Ok(ExitCode::from(2));
    }

    let declaring = outcome.files.values().filter(|v| !v.is_empty()).count();
    let index = ImplementsIndex {
        version: IMPLEMENTS_INDEX_VERSION,
        root: args.scan_root.clone(),
        tree_sha,
        files: outcome.files,
    };
    let json = index.to_json();
    let index_path = args.repo_root.join(IMPLEMENTS_INDEX_FILENAME);

    if args.check {
        let committed = match std::fs::read_to_string(&index_path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "check failed: {} does not exist; run implements-indexer to generate it",
                    index_path.display()
                );
                return Ok(ExitCode::FAILURE);
            }
            Err(e) => return Err(format!("cannot read {}: {e}", index_path.display())),
        };
        let committed = ImplementsIndex::parse(&committed)
            .map_err(|e| format!("committed index is unreadable: {e}"))?;
        if committed != index {
            eprintln!(
                "check failed: {} is out of date with the checkout \
                 (committed tree {}, checkout tree {}); regenerate it",
                index_path.display(),
                committed.tree_sha,
                index.tree_sha
            );
            return Ok(ExitCode::FAILURE);
        }
        println!(
            "index in sync: {} files ({} declaring implements), tree {}",
            index.files.len(),
            declaring,
            index.tree_sha
        );
        return Ok(ExitCode::SUCCESS);
    }

    std::fs::write(&index_path, &json)
        .map_err(|e| format!("cannot write {}: {e}", index_path.display()))?;
    println!(
        "wrote {}: {} files ({} declaring implements), {} bytes, tree {}",
        index_path.display(),
        index.files.len(),
        declaring,
        json.len(),
        index.tree_sha
    );
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}

//! The cached rust-analyzer index.
//!
//! `code-guide index` runs `rust-analyzer scip` over the workspace and keeps
//! the result under the shared cargo target directory, together with the key it
//! was built for. The key is a hash of everything that can change the index:
//! every Rust source and Cargo manifest of the workspace, `Cargo.lock`, the
//! rust-analyzer version and the configuration passed to it. Two runs over the
//! same inputs produce a byte-identical index, so a cached index whose key
//! matches is as good as a fresh one.
//!
//! A run takes about a minute and several GB of memory for this workspace, so it
//! only runs on request; the server reports a stale or missing index and says
//! which command rebuilds it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// The configuration rust-analyzer indexes with. `features: "all"` matters: the
/// engine's `wasm` module only exists with its feature on, and without it every
/// call made from there is missing from the index.
pub const RA_CONFIG: &str =
    r#"{"cargo":{"features":"all","buildScripts":{"enable":true}},"procMacro":{"enable":true}}"#;

/// The command that rebuilds the index, as the app shows it.
pub const REBUILD_COMMAND: &str = "just code-guide-index";

/// Where the workspace and the cached index live.
#[derive(Debug, Clone)]
pub struct Paths {
    /// The repository root (the parent of `packages/`).
    pub repo_root: PathBuf,
    /// The cargo workspace root, `packages/`. rust-analyzer indexes this, and
    /// paths in the index are relative to it.
    pub workspace: PathBuf,
    /// `<target dir>/code-guide/index.scip`.
    pub index_file: PathBuf,
    /// The key the index was built for, and a hash per input file, next to it
    /// (JSON, see [`Manifest`]). Written last: it marks a complete build.
    pub key_file: PathBuf,
    /// The source scan taken with the index (`crate::source::Sources`, JSON).
    pub sources_file: PathBuf,
}

impl Paths {
    pub fn new(workspace: PathBuf, target_dir: &Path) -> Self {
        let dir = target_dir.join("code-guide");
        let repo_root = workspace
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| workspace.clone());
        Self {
            repo_root,
            workspace,
            index_file: dir.join("index.scip"),
            key_file: dir.join("index.json"),
            sources_file: dir.join("index.sources.json"),
        }
    }
}

/// What the cached index is worth right now.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum IndexState {
    /// No index has been built.
    Missing,
    /// An index exists, but the sources or the tooling changed since.
    Stale,
    /// The index matches the current sources.
    Fresh,
}

/// Is a path one whose contents can change the index?
fn is_input(rel: &Path) -> bool {
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");
    rel.extension().and_then(|e| e.to_str()) == Some("rs")
        || name == "Cargo.toml"
        || name == "Cargo.lock"
}

/// Directories that hold no indexed source: build output, dependencies, the
/// apps' own JavaScript.
fn is_pruned(name: &str) -> bool {
    matches!(name, "target" | "node_modules" | ".git" | "dist" | ".cargo")
}

/// This tool's own crate: rust-analyzer indexes it, but the guide leaves it out
/// (`workspace.rs`), so editing the guide must not make the index stale.
const OWN_DIR: &str = "code-guide";

/// The files the key is computed over, sorted, relative to the workspace.
pub fn inputs(workspace: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = WalkDir::new(workspace)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir() && e.file_name().to_str().is_some_and(is_pruned))
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(workspace).ok().map(Path::to_path_buf))
        .filter(|rel| is_input(rel) && !rel.starts_with(OWN_DIR))
        .collect();
    out.sort();
    out
}

/// What an index was built from: the cache key, and a hash per input file, so
/// the guide can tell exactly which files changed since (see
/// [`changed_files`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub key: String,
    pub files: BTreeMap<String, String>,
}

/// The hash of every input file, by `/`-separated path.
pub fn file_hashes(workspace: &Path) -> std::io::Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for rel in inputs(workspace) {
        let bytes = std::fs::read(workspace.join(&rel))?;
        out.insert(
            rel.to_string_lossy().replace('\\', "/"),
            format!("{:x}", Sha256::digest(&bytes)),
        );
    }
    Ok(out)
}

/// The cache key: a hash over each input's path and contents (via
/// [`file_hashes`]), the rust-analyzer version and [`RA_CONFIG`]. Paths are
/// `/`-separated, so the key does not depend on the platform.
pub fn cache_key(workspace: &Path, ra_version: &str) -> std::io::Result<String> {
    Ok(manifest(workspace, ra_version)?.key)
}

/// The manifest the current sources would get.
pub fn manifest(workspace: &Path, ra_version: &str) -> std::io::Result<Manifest> {
    let files = file_hashes(workspace)?;
    let mut h = Sha256::new();
    h.update(b"code-guide index v1\n");
    h.update(ra_version.trim().as_bytes());
    h.update(b"\n");
    h.update(RA_CONFIG.as_bytes());
    h.update(b"\n");
    for (path, hash) in &files {
        h.update(path.as_bytes());
        h.update(b"\0");
        h.update(hash.as_bytes());
        h.update(b"\n");
    }
    Ok(Manifest {
        key: format!("{:x}", h.finalize()),
        files,
    })
}

fn read_manifest(paths: &Paths) -> Option<Manifest> {
    let text = std::fs::read_to_string(&paths.key_file).ok()?;
    serde_json::from_str(&text).ok()
}

/// The source files (`.rs`, `/`-separated) whose contents differ from when the
/// index was built: changed, added or removed. The guide still shows them as
/// they were indexed; this is what it names as out of date. `None` without a
/// complete index.
pub fn changed_files(paths: &Paths) -> std::io::Result<Option<BTreeSet<String>>> {
    let Some(built) = read_manifest(paths).filter(|_| paths.index_file.is_file()) else {
        return Ok(None);
    };
    let now = file_hashes(&paths.workspace)?;
    let changed = built
        .files
        .keys()
        .chain(now.keys())
        .filter(|p| p.ends_with(".rs") && built.files.get(*p) != now.get(*p))
        .cloned()
        .collect();
    Ok(Some(changed))
}

/// `rust-analyzer --version`, or an error that says how to install it.
pub fn ra_version() -> Result<String, String> {
    let out = Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map_err(|e| {
            format!("cannot run rust-analyzer ({e}); install it with `rustup component add rust-analyzer`")
        })?;
    if !out.status.success() {
        return Err(format!(
            "rust-analyzer --version failed: {}. Install it with `rustup component add rust-analyzer`.",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The state of the cached index against the current sources.
pub fn state(paths: &Paths, ra_version: &str) -> std::io::Result<IndexState> {
    if !paths.index_file.is_file() || !paths.sources_file.is_file() {
        return Ok(IndexState::Missing);
    }
    let built = read_manifest(paths).map(|m| m.key).unwrap_or_default();
    let current = cache_key(&paths.workspace, ra_version)?;
    Ok(if built == current {
        IndexState::Fresh
    } else {
        IndexState::Stale
    })
}

/// Rebuilds the index unless it is fresh (or `force`). The key is computed
/// before rust-analyzer runs: a source that changes during the run then makes the
/// next check report the index as stale, never as fresh.
pub fn build(paths: &Paths, crate_dirs: &[String], force: bool) -> Result<bool, String> {
    let version = ra_version()?;
    let manifest = manifest(&paths.workspace, &version).map_err(|e| e.to_string())?;
    if !force
        && paths.index_file.is_file()
        && paths.sources_file.is_file()
        && read_manifest(paths).is_some_and(|m| m.key == manifest.key)
    {
        return Ok(false);
    }
    // The source scan is taken now, with the key, before rust-analyzer reads
    // the files: the index, the scan and the key describe the same sources. A
    // file that changes during the run makes the next check report Stale.
    let sources = crate::source::Sources::scan(&paths.workspace, crate_dirs);
    let sources_json = serde_json::to_string(&sources).map_err(|e| e.to_string())?;
    let dir = paths
        .index_file
        .parent()
        .ok_or("index path has no directory")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let config = dir.join("rust-analyzer.json");
    std::fs::write(&config, RA_CONFIG).map_err(|e| e.to_string())?;
    let partial = dir.join("index.scip.partial");

    let status = Command::new("rust-analyzer")
        .arg("scip")
        .arg(&paths.workspace)
        .arg("--output")
        .arg(&partial)
        .arg("--config-path")
        .arg(&config)
        .arg("--exclude-vendored-libraries")
        .status()
        .map_err(|e| format!("cannot run rust-analyzer: {e}"))?;
    if !status.success() {
        return Err(format!("rust-analyzer scip failed ({status})"));
    }
    // Swap in the new index only once it is complete, and write its key after,
    // so an interrupted run leaves the old index and key consistent.
    std::fs::rename(&partial, &paths.index_file).map_err(|e| e.to_string())?;
    std::fs::write(&paths.sources_file, sources_json).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&paths.key_file, json).map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::testutil::Dir;

    fn workspace() -> Dir {
        let d = Dir::new();
        d.write("Cargo.toml", "[workspace]\n");
        d.write("Cargo.lock", "# lock\n");
        d.write("a/Cargo.toml", "[package]\n");
        d.write("a/src/lib.rs", "pub fn f() {}\n");
        d.write("a/ui/node_modules/x/index.rs", "not indexed\n");
        d.write("a/target/debug/build.rs", "not indexed\n");
        d.write("a/README.md", "not an input\n");
        d
    }

    #[test]
    fn inputs_are_sources_and_manifests_only_sorted() {
        let d = workspace();
        let got: Vec<String> = inputs(d.path())
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(
            got,
            ["Cargo.lock", "Cargo.toml", "a/Cargo.toml", "a/src/lib.rs"]
        );
    }

    #[test]
    fn the_key_is_stable_and_follows_every_input() {
        let d = workspace();
        let k1 = cache_key(d.path(), "ra 1").unwrap();
        assert_eq!(k1, cache_key(d.path(), "ra 1").unwrap());
        assert_ne!(
            k1,
            cache_key(d.path(), "ra 2").unwrap(),
            "rust-analyzer version"
        );
        d.write("a/src/lib.rs", "pub fn g() {}\n");
        let k2 = cache_key(d.path(), "ra 1").unwrap();
        assert_ne!(k1, k2, "a source");
        d.write("Cargo.lock", "# other lock\n");
        assert_ne!(k2, cache_key(d.path(), "ra 1").unwrap(), "the lock file");
    }

    #[test]
    fn files_that_are_not_inputs_do_not_change_the_key() {
        let d = workspace();
        let k1 = cache_key(d.path(), "ra").unwrap();
        d.write("a/README.md", "changed\n");
        d.write("a/ui/node_modules/x/index.rs", "changed\n");
        assert_eq!(k1, cache_key(d.path(), "ra").unwrap());
    }

    #[test]
    fn editing_the_guide_itself_does_not_change_the_key() {
        let d = workspace();
        let k1 = cache_key(d.path(), "ra").unwrap();
        d.write("code-guide/src/main.rs", "fn main() {}\n");
        assert_eq!(k1, cache_key(d.path(), "ra").unwrap());
    }

    #[test]
    fn a_renamed_file_changes_the_key() {
        let d = workspace();
        let k1 = cache_key(d.path(), "ra").unwrap();
        std::fs::rename(
            d.path().join("a/src/lib.rs"),
            d.path().join("a/src/main.rs"),
        )
        .unwrap();
        assert_ne!(k1, cache_key(d.path(), "ra").unwrap());
    }

    #[test]
    fn state_is_missing_stale_or_fresh() {
        let d = workspace();
        let target = Dir::new();
        let paths = Paths::new(d.path().to_path_buf(), target.path());
        assert_eq!(state(&paths, "ra").unwrap(), IndexState::Missing);
        std::fs::create_dir_all(paths.index_file.parent().unwrap()).unwrap();
        std::fs::write(&paths.index_file, b"index").unwrap();
        std::fs::write(&paths.sources_file, "{}").unwrap();
        std::fs::write(&paths.key_file, "not a manifest").unwrap();
        assert_eq!(state(&paths, "ra").unwrap(), IndexState::Stale);
        let m = manifest(d.path(), "ra").unwrap();
        std::fs::write(&paths.key_file, serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(state(&paths, "ra").unwrap(), IndexState::Fresh);
        d.write("a/src/lib.rs", "pub fn changed() {}\n");
        assert_eq!(state(&paths, "ra").unwrap(), IndexState::Stale);
    }

    #[test]
    fn changed_files_names_exactly_the_sources_that_differ() {
        let d = workspace();
        let target = Dir::new();
        let paths = Paths::new(d.path().to_path_buf(), target.path());
        assert_eq!(changed_files(&paths).unwrap(), None, "no index yet");
        std::fs::create_dir_all(paths.index_file.parent().unwrap()).unwrap();
        std::fs::write(&paths.index_file, b"index").unwrap();
        let m = manifest(d.path(), "ra").unwrap();
        std::fs::write(&paths.key_file, serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(changed_files(&paths).unwrap(), Some(BTreeSet::new()));
        d.write("a/src/lib.rs", "pub fn changed() {}\n");
        d.write("a/src/new.rs", "pub fn added() {}\n");
        d.write("Cargo.lock", "# not a source, so not listed\n");
        let got: Vec<String> = changed_files(&paths)
            .unwrap()
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(got, ["a/src/lib.rs", "a/src/new.rs"]);
    }
}

//! Feature-file discovery for the BDD runner.
//!
//! Split out of `main.rs` because that target is `harness = false` and can
//! therefore hold no `#[test]` of its own. `tests/bdd_discovery.rs` includes
//! this file and does run under `cargo test`.
//!
//! Discovery fails loudly. A walk error and an empty bucket both abort the run
//! instead of silently shrinking the suite: bucket A is the only place where
//! Dutch law is executed, and a renamed directory removes those scenarios
//! without a single line of output.
//!
//! One case stays quiet by design: `WalkDir` does not follow symlinks, so a
//! symlinked `scenarios/` directory is skipped without an error. The corpus in
//! this repository holds no such link, and following links would let a loop
//! hang the run.

use std::fmt;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Debug)]
pub enum DiscoveryError {
    Walk {
        bucket: &'static str,
        source: walkdir::Error,
    },
    Empty {
        bucket: &'static str,
        root: PathBuf,
    },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Walk { bucket, source } => {
                write!(f, "bucket {bucket}: cannot walk feature files: {source}")
            }
            Self::Empty { bucket, root } => write!(
                f,
                "bucket {bucket}: no feature files under {}",
                root.display()
            ),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Collect every feature file from both buckets:
/// - bucket A: any `*.feature` under a `scenarios/` directory in the corpus, and
/// - bucket B: `bdd/conformance/*.feature`.
pub fn collect_feature_paths(root: &Path) -> Result<Vec<PathBuf>, DiscoveryError> {
    let corpus = root.join("corpus/regulation");
    let mut features = collect_bucket("A (corpus scenarios)", &corpus, |p| {
        // Only components below the corpus root count: a checkout that happens
        // to live under a directory called `scenarios` must not turn every
        // corpus feature file into a bucket-A scenario.
        p.strip_prefix(&corpus)
            .unwrap_or(p)
            .components()
            .any(|c| c.as_os_str() == "scenarios")
    })?;

    features.extend(collect_bucket(
        "B (engine conformance)",
        &root.join("bdd/conformance"),
        |_| true,
    )?);

    features.sort();
    Ok(features)
}

fn collect_bucket(
    bucket: &'static str,
    root: &Path,
    accept: impl Fn(&Path) -> bool,
) -> Result<Vec<PathBuf>, DiscoveryError> {
    let mut features = Vec::new();

    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|source| DiscoveryError::Walk { bucket, source })?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "feature") && accept(path) {
            features.push(path.to_path_buf());
        }
    }

    if features.is_empty() {
        return Err(DiscoveryError::Empty {
            bucket,
            root: root.to_path_buf(),
        });
    }

    Ok(features)
}

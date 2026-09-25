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

/// Which bucket(s) a run covers. `main.rs` reads it from `BDD_BUCKET`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bucket {
    All,
    Corpus,
    Conformance,
}

impl Bucket {
    fn covers_corpus(self) -> bool {
        matches!(self, Self::All | Self::Corpus)
    }

    fn covers_conformance(self) -> bool {
        matches!(self, Self::All | Self::Conformance)
    }
}

/// Collect the feature files of the selected bucket(s):
/// - bucket A: any `*.feature` under a `scenarios/` directory in `corpus`, and
/// - bucket B: `bdd/conformance/*.feature` under `root`.
///
/// `corpus` is passed separately because bucket A follows `REGULATION_PATH`,
/// the same variable the engine loads its laws from, so the scenarios and the
/// laws they test always come from one corpus.
pub fn collect_feature_paths(
    root: &Path,
    corpus: &Path,
    bucket: Bucket,
) -> Result<Vec<PathBuf>, DiscoveryError> {
    let mut features = Vec::new();

    if bucket.covers_corpus() {
        features.extend(collect_bucket("A (corpus scenarios)", corpus, |p| {
            // Only components below the corpus root count: a checkout that
            // happens to live under a directory called `scenarios` must not
            // turn every corpus feature file into a bucket-A scenario.
            p.strip_prefix(corpus)
                .unwrap_or(p)
                .components()
                .any(|c| c.as_os_str() == "scenarios")
        })?);
    }

    if bucket.covers_conformance() {
        features.extend(collect_bucket(
            "B (engine conformance)",
            &root.join("bdd/conformance"),
            |_| true,
        )?);
    }

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

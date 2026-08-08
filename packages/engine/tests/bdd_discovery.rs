//! Tests for the BDD runner's feature-file discovery.
//!
//! The runner (`tests/bdd/main.rs`) is `harness = false`, so its own module
//! tree cannot carry `#[test]`s. This target includes the discovery module and
//! runs under plain `cargo test`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "bdd/discovery.rs"]
mod discovery;

use std::fs;
use std::path::Path;

use discovery::{collect_feature_paths, DiscoveryError};

/// Minimal stand-in for the repository layout: one bucket-A scenario next to a
/// law, one bucket-B conformance feature.
fn write_repo(root: &Path) {
    let scenarios = root.join("corpus/regulation/nl/wet/test_wet/scenarios");
    fs::create_dir_all(&scenarios).unwrap();
    fs::write(scenarios.join("uitkering.feature"), "Feature: uitkering\n").unwrap();

    let conformance = root.join("bdd/conformance");
    fs::create_dir_all(&conformance).unwrap();
    fs::write(conformance.join("dates.feature"), "Feature: dates\n").unwrap();
}

#[test]
fn collects_both_buckets_sorted() {
    let tmp = tempfile::tempdir().unwrap();
    write_repo(tmp.path());

    let features = collect_feature_paths(tmp.path()).expect("both buckets present");

    assert_eq!(features.len(), 2);
    assert!(features[0].ends_with("bdd/conformance/dates.feature"));
    assert!(features[1].ends_with("scenarios/uitkering.feature"));
}

#[test]
fn ignores_corpus_features_outside_a_scenarios_directory() {
    let tmp = tempfile::tempdir().unwrap();
    write_repo(tmp.path());

    let stray = tmp.path().join("corpus/regulation/nl/wet/test_wet");
    fs::write(stray.join("los.feature"), "Feature: los\n").unwrap();

    let features = collect_feature_paths(tmp.path()).unwrap();

    assert_eq!(features.len(), 2);
    assert!(!features.iter().any(|p| p.ends_with("los.feature")));
}

#[test]
fn a_renamed_scenarios_directory_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    write_repo(tmp.path());

    let law = tmp.path().join("corpus/regulation/nl/wet/test_wet");
    fs::rename(law.join("scenarios"), law.join("scenario")).unwrap();

    let err = collect_feature_paths(tmp.path()).expect_err("bucket A is empty");

    assert!(
        matches!(&err, DiscoveryError::Empty { bucket, .. } if bucket.starts_with('A')),
        "expected an empty bucket A, got {err}"
    );
}

#[test]
fn a_missing_conformance_directory_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    write_repo(tmp.path());

    fs::remove_dir_all(tmp.path().join("bdd/conformance")).unwrap();

    let err = collect_feature_paths(tmp.path()).expect_err("bucket B cannot be walked");

    assert!(
        matches!(&err, DiscoveryError::Walk { bucket, .. } if bucket.starts_with('B')),
        "expected a walk error on bucket B, got {err}"
    );
}

#[test]
fn the_real_repository_layout_resolves() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("project root");

    let features = collect_feature_paths(root).expect("both buckets exist in this repository");

    assert!(features
        .iter()
        .any(|p| p.components().any(|c| c.as_os_str() == "scenarios")));
    assert!(features
        .iter()
        .any(|p| p.components().any(|c| c.as_os_str() == "conformance")));
}

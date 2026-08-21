//! Conformance coverage test (RFC-014).
//!
//! Verifies that every schema operation in the engine appears in at least one
//! conformance level in the manifest. Fails CI when a new operation is added
//! to the engine without updating the conformance suite.
//!
//! The manifests are read from disk at test time rather than baked in with
//! `include_str!`. A hardcoded path silently keeps checking an old schema
//! version after a bump: the tests stay green while the manifest for the new
//! version is missing or empty. Here the version comes from the `schema/latest`
//! symlink, so a bump without a manifest is a failure, and the structural
//! invariants run over every manifest in the tree.

use regelrecht_engine::types::Operation;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Repo root, derived from this crate's manifest dir (`packages/engine`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve repo root")
}

/// The schema version `schema/latest` points at (e.g. `v0.5.6`).
fn latest_schema_version() -> String {
    let link = repo_root().join("schema/latest");
    let target = std::fs::read_link(&link).expect("schema/latest is a symlink");
    target
        .file_name()
        .expect("schema/latest points somewhere")
        .to_string_lossy()
        .to_string()
}

/// Every `conformance/v*/manifest.json`, as (version, parsed manifest).
fn all_manifests() -> Vec<(String, serde_json::Value)> {
    let dir = repo_root().join("conformance");
    let mut out: Vec<(String, serde_json::Value)> = std::fs::read_dir(&dir)
        .expect("conformance/ exists")
        .filter_map(Result::ok)
        .filter_map(|e| {
            let version = e.file_name().to_string_lossy().to_string();
            if !version.starts_with('v') {
                return None;
            }
            let path = e.path().join("manifest.json");
            let text = std::fs::read_to_string(&path).ok()?;
            let json = serde_json::from_str(&text)
                .unwrap_or_else(|err| panic!("{} is not valid JSON: {err}", path.display()));
            Some((version, json))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!out.is_empty(), "no conformance manifests found");
    out
}

/// All operation names declared across the levels of one manifest.
fn operations_in(manifest: &serde_json::Value) -> HashSet<String> {
    let levels = manifest["levels"]
        .as_object()
        .expect("manifest has 'levels' object");
    let mut covered = HashSet::new();
    for level_info in levels.values() {
        if let Some(ops) = level_info["operations"].as_array() {
            for op in ops {
                covered.insert(op.as_str().expect("operation name is a string").to_string());
            }
        }
    }
    covered
}

#[test]
fn latest_schema_version_has_a_manifest() {
    let version = latest_schema_version();
    let path = repo_root().join(format!("conformance/{version}/manifest.json"));
    assert!(
        path.exists(),
        "schema/latest points at {version} but {} does not exist — \
         a new schema version needs its own conformance manifest (RFC-014)",
        path.display()
    );
}

#[test]
fn manifest_schema_version_matches_its_directory() {
    for (version, manifest) in all_manifests() {
        let declared = manifest["schema_version"]
            .as_str()
            .unwrap_or_else(|| panic!("conformance/{version}/manifest.json has no schema_version"));
        assert_eq!(
            declared.trim_start_matches('v'),
            version.trim_start_matches('v'),
            "conformance/{version}/manifest.json declares schema_version {declared}"
        );
    }
}

#[test]
fn every_schema_operation_has_a_conformance_level() {
    // Only the manifest for the current schema is expected to be complete:
    // an older one describes an older operation set and must not be updated.
    let version = latest_schema_version();
    let manifests = all_manifests();
    let (_, manifest) = manifests
        .iter()
        .find(|(v, _)| *v == version)
        .unwrap_or_else(|| panic!("no conformance manifest for {version}"));

    let covered = operations_in(manifest);
    let missing: Vec<&str> = Operation::SCHEMA_OPERATIONS
        .iter()
        .map(|op| op.name())
        .filter(|name| !covered.contains(*name))
        .collect();

    assert!(
        missing.is_empty(),
        "Operations missing from conformance/{version}/manifest.json: {missing:?}. \
         Add them to the appropriate level in the conformance manifest."
    );
}

#[test]
fn manifest_contains_no_unknown_operations() {
    let known: HashSet<&str> = Operation::SCHEMA_OPERATIONS
        .iter()
        .map(|op| op.name())
        .collect();

    for (version, manifest) in all_manifests() {
        for name in operations_in(&manifest) {
            assert!(
                known.contains(name.as_str()),
                "conformance/{version}/manifest.json lists operation '{name}' \
                 which is not in Operation::SCHEMA_OPERATIONS. \
                 Either add it to the engine or remove it from the manifest."
            );
        }
    }
}

#[test]
fn no_operation_in_multiple_levels() {
    for (version, manifest) in all_manifests() {
        let levels = manifest["levels"]
            .as_object()
            .expect("manifest has 'levels' object");
        let mut seen: HashSet<String> = HashSet::new();
        for (level_name, level_info) in levels {
            if let Some(ops) = level_info["operations"].as_array() {
                for op in ops {
                    let name = op.as_str().expect("operation name is a string").to_string();
                    assert!(
                        seen.insert(name.clone()),
                        "Operation '{name}' appears in multiple conformance levels \
                         in conformance/{version}/manifest.json (duplicate found in \
                         '{level_name}'). Each operation belongs to exactly one level."
                    );
                }
            }
        }
    }
}

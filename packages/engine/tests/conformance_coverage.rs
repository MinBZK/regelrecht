//! Conformance coverage test (RFC-014).
//!
//! Verifies that every schema operation in the engine appears in at least one
//! conformance level in the manifest. Fails CI when a new operation is added
//! to the engine without updating the conformance suite.

use regelrecht_engine::types::Operation;
use std::collections::HashSet;

/// Path to the latest conformance manifest, relative to the engine crate root.
const MANIFEST: &str = include_str!("../../../conformance/v0.5.0/manifest.json");

#[test]
fn every_schema_operation_has_a_conformance_level() {
    let manifest: serde_json::Value =
        serde_json::from_str(MANIFEST).expect("conformance manifest is valid JSON");

    // Collect all operations declared across all conformance levels.
    let mut covered: HashSet<String> = HashSet::new();
    let levels = manifest["levels"]
        .as_object()
        .expect("manifest has 'levels' object");
    for (_level_name, level_info) in levels {
        if let Some(ops) = level_info["operations"].as_array() {
            for op in ops {
                covered.insert(op.as_str().expect("operation name is a string").to_string());
            }
        }
    }

    // Every schema operation must appear in at least one level.
    let mut missing: Vec<&str> = Vec::new();
    for op in Operation::SCHEMA_OPERATIONS {
        if !covered.contains(op.name()) {
            missing.push(op.name());
        }
    }

    assert!(
        missing.is_empty(),
        "Operations missing from conformance manifest: {:?}. \
         Add them to the appropriate level in the conformance manifest.",
        missing
    );
}

#[test]
fn manifest_contains_no_unknown_operations() {
    let manifest: serde_json::Value =
        serde_json::from_str(MANIFEST).expect("conformance manifest is valid JSON");

    let known: HashSet<&str> = Operation::SCHEMA_OPERATIONS
        .iter()
        .map(|op| op.name())
        .collect();

    let levels = manifest["levels"]
        .as_object()
        .expect("manifest has 'levels' object");
    for (level_name, level_info) in levels {
        if let Some(ops) = level_info["operations"].as_array() {
            for op in ops {
                let name = op.as_str().expect("operation name is a string");
                assert!(
                    known.contains(name),
                    "Conformance level '{level_name}' lists operation '{name}' \
                     which is not in Operation::SCHEMA_OPERATIONS. \
                     Either add it to the engine or remove it from the manifest."
                );
            }
        }
    }
}

#[test]
fn no_operation_in_multiple_levels() {
    let manifest: serde_json::Value =
        serde_json::from_str(MANIFEST).expect("conformance manifest is valid JSON");

    let mut seen: HashSet<String> = HashSet::new();
    let levels = manifest["levels"]
        .as_object()
        .expect("manifest has 'levels' object");
    for (level_name, level_info) in levels {
        if let Some(ops) = level_info["operations"].as_array() {
            for op in ops {
                let name = op.as_str().expect("operation name is a string").to_string();
                assert!(
                    seen.insert(name.clone()),
                    "Operation '{name}' appears in multiple conformance levels \
                     (duplicate found in '{level_name}'). Each operation belongs to exactly one level."
                );
            }
        }
    }
}

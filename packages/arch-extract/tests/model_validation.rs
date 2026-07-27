//! Guards the committed `model.json`: it must validate against the JSON schema
//! and must match the well-known crate layer graph. These run as part of
//! `cargo test` (hence `just check`), so a stale or malformed model — or a lost
//! dependency edge — fails CI rather than silently shipping to the docs site.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn arch_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/src/content/architecture")
}

fn load_json(name: &str) -> Value {
    let path = arch_dir().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
}

#[test]
fn model_validates_against_schema() {
    let schema = load_json("model.schema.json");
    let model = load_json("model.json");

    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&model)
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "model.json fails schema:\n{}",
        errors.join("\n")
    );
}

fn crate_short_names(model: &Value) -> BTreeSet<String> {
    model["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .filter(|n| n["kind"] == "crate")
        .filter_map(|n| n["name"].as_str().map(str::to_string))
        .collect()
}

/// `from` short-name -> set of `to` short-names over `depends-on` edges.
fn depends_on(model: &Value, from_short: &str) -> BTreeSet<String> {
    let from_id = format!("crate:{from_short}");
    model["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .filter(|e| e["kind"] == "depends-on" && e["from"] == from_id.as_str())
        .filter_map(|e| e["to"].as_str())
        .filter_map(|to| to.strip_prefix("crate:").map(str::to_string))
        .collect()
}

#[test]
fn product_crates_present() {
    let model = load_json("model.json");
    let crates = crate_short_names(&model);
    let expected: BTreeSet<String> = [
        "admin",
        "auth",
        "corpus",
        "editor-api",
        "engine",
        "github",
        "harvester",
        "law-model",
        "pipeline",
        "shared",
        "tui",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(
        crates, expected,
        "crate nodes must be exactly the product crates"
    );
}

#[test]
fn layer_graph_matches_known_dependencies() {
    let model = load_json("model.json");

    // `shared` is the foundation: it depends on no other workspace crate.
    assert!(depends_on(&model, "shared").is_empty());

    // `github` is a second foundation: the shared GitHub REST client is
    // deliberately standalone, so the crates that consolidated onto it
    // (corpus, editor-api) can depend on it without a cycle.
    assert!(depends_on(&model, "github").is_empty());

    // Spot-check the documented layers.
    assert_eq!(
        depends_on(&model, "engine"),
        BTreeSet::from(["shared".to_string(), "law-model".to_string()])
    );
    assert_eq!(
        depends_on(&model, "law-model"),
        BTreeSet::from(["shared".to_string()])
    );
    assert_eq!(
        depends_on(&model, "admin"),
        BTreeSet::from([
            "auth".to_string(),
            "shared".to_string(),
            "corpus".to_string(),
            "harvester".to_string(),
            "pipeline".to_string(),
        ])
    );
}

#[test]
fn source_level_extraction_ran() {
    let model = load_json("model.json");
    let nodes = model["nodes"].as_array().expect("nodes array");

    let count_kind = |kind: &str| nodes.iter().filter(|n| n["kind"] == kind).count();

    // The syn pass must have produced structure below the crate level.
    assert!(count_kind("module") > 0, "expected module nodes");
    assert!(count_kind("struct") > 0, "expected struct nodes");
    assert!(count_kind("method") > 0, "expected method nodes");

    // And the engine's execution service should be captured at type level.
    let has_service = nodes.iter().any(|n| {
        n["kind"] == "struct"
            && n["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("type:engine::"))
    });
    assert!(has_service, "expected engine type nodes");
}

/// The crate prefix of a `kind:crate::…` node id, e.g. `mod:engine::service`
/// -> `engine`. Returns `None` for a bare `crate:engine` (no `::`).
fn crate_prefix(id: &str) -> Option<&str> {
    let after = id.split_once(':')?.1;
    after.split_once("::").map(|(head, _)| head)
}

#[test]
fn deep_extraction_covers_all_crates() {
    // Phase 2: the deep source pass now runs for every crate, not just
    // engine + corpus. Assert that each of the ten product crates has at least
    // one module node below it (i.e. the syn pass actually walked its src).
    let model = load_json("model.json");
    let nodes = model["nodes"].as_array().expect("nodes array");

    let crates = crate_short_names(&model);
    let mut crates_with_modules: BTreeSet<String> = BTreeSet::new();
    for n in nodes {
        if n["kind"] != "module" {
            continue;
        }
        if let Some(id) = n["id"].as_str() {
            if let Some(c) = crate_prefix(id) {
                crates_with_modules.insert(c.to_string());
            }
        }
    }

    let missing: Vec<String> = crates.difference(&crates_with_modules).cloned().collect();
    assert!(
        missing.is_empty(),
        "every crate should have deep (module) extraction; missing: {missing:?}"
    );
}

fn arch_pages_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/src/content/docs/architecture")
}

#[test]
fn generated_c4_pages_present_and_derived() {
    // The C4 views are generated from the model, never hand-drawn: each carries
    // the generated-file banner and a fenced mermaid C4 block of its level.
    for (file, marker) in [
        ("context.md", "C4Context"),
        ("container.md", "C4Container"),
        ("component.md", "C4Component"),
    ] {
        let text = std::fs::read_to_string(arch_pages_dir().join(file))
            .unwrap_or_else(|e| panic!("reading {file}: {e}"));
        assert!(
            text.contains("Generated by packages/arch-extract"),
            "{file} must carry the generated-file banner"
        );
        assert!(
            text.contains("```mermaid") && text.contains(marker),
            "{file} must contain a fenced {marker} diagram"
        );
    }
}

#[test]
fn one_generated_page_per_crate() {
    let model = load_json("model.json");
    let crates = crate_short_names(&model);
    for c in &crates {
        let path = arch_pages_dir().join("crates").join(format!("{c}.md"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        assert!(
            text.contains("C4Component"),
            "per-crate page {c}.md must contain a C4Component diagram"
        );
    }
}

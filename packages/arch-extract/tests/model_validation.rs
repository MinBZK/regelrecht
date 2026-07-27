//! Guards the **generated** model: `arch-extract` is run on-demand (no committed
//! `model.json` exists), and its output must validate against the JSON schema
//! and match the well-known crate layer graph. These run as part of `cargo test`
//! (hence `just check`), so a malformed model — or a lost dependency edge —
//! fails CI rather than silently breaking the architecture explorer.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

fn arch_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/src/content/architecture")
}

/// The workspace manifest (`packages/Cargo.toml`), so the generator's
/// `cargo metadata` resolves the whole workspace regardless of the test's cwd.
fn workspace_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml")
}

fn load_schema() -> Value {
    let path = arch_dir().join("model.schema.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
}

/// Runs the extractor once (deep pass over every crate, the default) and returns
/// its `model.json` on stdout, parsed. Cached so the ~2s generation happens once
/// for the whole test binary rather than per test.
fn model() -> &'static Value {
    static MODEL: OnceLock<Value> = OnceLock::new();
    MODEL.get_or_init(|| {
        let out = Command::new(env!("CARGO_BIN_EXE_arch-extract"))
            .args([
                "generate",
                "--stdout",
                "--manifest-path",
                workspace_manifest().to_str().expect("utf-8 manifest path"),
            ])
            .output()
            .expect("running arch-extract");
        assert!(
            out.status.success(),
            "arch-extract failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("parsing generated model.json")
    })
}

#[test]
fn model_validates_against_schema() {
    let schema = load_schema();
    let model = model();

    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(model)
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "generated model fails schema:\n{}",
        errors.join("\n")
    );
}

fn crate_short_names(model: &Value) -> std::collections::BTreeSet<String> {
    model["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .filter(|n| n["kind"] == "crate")
        .filter_map(|n| n["name"].as_str().map(str::to_string))
        .collect()
}

/// `from` short-name -> set of `to` short-names over `depends-on` edges.
fn depends_on(model: &Value, from_short: &str) -> std::collections::BTreeSet<String> {
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
    let crates = crate_short_names(model());
    let expected: std::collections::BTreeSet<String> = [
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
    let model = model();

    // `shared` is the foundation: it depends on no other workspace crate.
    assert!(depends_on(model, "shared").is_empty());

    // `github` is a second foundation: the shared GitHub REST client is
    // deliberately standalone, so the crates that consolidated onto it
    // (corpus, editor-api) can depend on it without a cycle.
    assert!(depends_on(model, "github").is_empty());

    // Spot-check the documented layers.
    assert_eq!(
        depends_on(model, "engine"),
        std::collections::BTreeSet::from(["shared".to_string(), "law-model".to_string()])
    );
    assert_eq!(
        depends_on(model, "law-model"),
        std::collections::BTreeSet::from(["shared".to_string()])
    );
    assert_eq!(
        depends_on(model, "admin"),
        std::collections::BTreeSet::from([
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
    let model = model();
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
    // The deep source pass runs for every crate by default, not just
    // engine + corpus. Assert that each product crate has at least one module
    // node below it (i.e. the syn pass actually walked its src).
    let model = model();
    let nodes = model["nodes"].as_array().expect("nodes array");

    let crates = crate_short_names(model);
    let mut crates_with_modules: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
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

#[test]
fn generation_is_deterministic() {
    // The explorer leans on stable node identity between runs, so two
    // generations must be byte-for-byte identical (nodes sorted by id, edges
    // sorted + deduped, no timestamp).
    let first = serde_json::to_string(model()).expect("serialize");

    let out = Command::new(env!("CARGO_BIN_EXE_arch-extract"))
        .args([
            "generate",
            "--stdout",
            "--manifest-path",
            workspace_manifest().to_str().expect("utf-8 manifest path"),
        ])
        .output()
        .expect("running arch-extract");
    assert!(out.status.success(), "arch-extract failed on second run");
    let second: Value = serde_json::from_slice(&out.stdout).expect("parse second run");
    let second = serde_json::to_string(&second).expect("serialize");

    assert_eq!(first, second, "model generation must be deterministic");
}

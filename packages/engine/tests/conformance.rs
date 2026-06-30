//! Schema ↔ law-model conformance suite.
//!
//! The canonical, hand-authored `schema/*/schema.json` is the public contract
//! for the law-YAML format; the Rust `law-model` is one implementation that must
//! provably *conform* to it. This suite proves that — it is the structural twin
//! of the BDD bucket-B engine-conformance suite (which proves an engine speaks
//! the whole language behaviourally).
//!
//! Only built with the `validate` feature (which pulls in `jsonschema` via
//! `regelrecht_engine::schema`). Run with `just conformance`. See
//! `tests/conformance/README.md` for the contract and how to add fixtures.
//!
//! Two tiers (mirroring BDD bucket-A/B):
//!   - Tier A — differential over the real corpus + a roundtrip fidelity report.
//!   - Tier B — synthetic fixtures exercising constructs the corpus may not hit.
#![cfg(feature = "validate")]

use std::path::{Path, PathBuf};

use regelrecht_engine::article::{ArticleBasedLaw, LawLoad};
use regelrecht_engine::schema::{detect_version, validation_errors_for};
use serde_json::Value;
use walkdir::WalkDir;

/// Invalid (schema-rejected) Tier-B fixtures that the lenient `law-model`
/// currently *accepts* anyway — i.e. the model is more permissive than the
/// schema for these. This list IS the Phase-1 measurement of the soundness gap;
/// each entry is a candidate to resolve in Phase 2 (tighten the model, or
/// consciously declare the model lenient). Keep it in sync: an undocumented gap
/// fails the suite, and so does a stale entry the model now rejects.
const KNOWN_GAPS: &[&str] = &[
    // Measured 2026-06-30 (Phase-1 MVP). The lenient `law-model` accepts these
    // three schema-rejected shapes; only `bad_regulatory_layer` is conformant
    // (the model rejects an unknown enum variant).
    "missing_required_url.yaml", // schema requires top-level `url`; model treats it as optional
    "unknown_field_in_article.yaml", // article is additionalProperties:false; serde silently drops the field
    "wrong_type_publication_date.yaml", // schema wants a date string; model coerces the YAML integer
];

/// Repo root, derived from this crate's manifest dir (`packages/engine`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve repo root")
}

/// Recursively sort object keys and drop nulls, so two structurally-equal
/// documents compare equal regardless of key order or omitted-vs-null noise.
fn normalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                let nv = normalize(&map[k]);
                if !nv.is_null() {
                    out.insert(k.clone(), nv);
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
        other => other.clone(),
    }
}

fn schema_accepts(version: &str, value: &Value) -> bool {
    matches!(validation_errors_for(version, value), Ok(errs) if errs.is_empty())
}

/// Tier A — every real corpus law must round-trip through schema ⋂ model.
///
/// Hard assertions (already guaranteed by the `just validate` CI gate, made
/// explicit here): every corpus law with a recognised `$schema` is accepted by
/// that schema (1) and parses into the model (2). Reported, non-fatal in this
/// MVP: whether the re-serialized model is still schema-valid (3) and value-
/// stable (4) — these quantify lossy serialization for the Phase-2 decision.
#[test]
fn tier_a_corpus_differential() {
    let root = repo_root();
    let corpus = root.join("corpus/regulation");

    let mut checked = 0usize;
    let mut hard_failures: Vec<String> = Vec::new();
    let mut not_revalid: Vec<String> = Vec::new();
    let mut value_drift: Vec<String> = Vec::new();

    for entry in WalkDir::new(&corpus).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let content = std::fs::read_to_string(path).expect("read corpus file");
        let Ok(mut value) = serde_yaml_ng::from_str::<Value>(&content) else {
            continue; // not a YAML mapping we can reason about
        };
        let Some(version) = detect_version(&value) else {
            continue; // no/unknown $schema → not a versioned law document
        };
        checked += 1;
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();

        // (1) schema accepts the published law.
        let errs = validation_errors_for(version, &value).expect("compile schema");
        if !errs.is_empty() {
            hard_failures.push(format!(
                "{rel}: schema {version} rejected a corpus law: {errs:?}"
            ));
            continue;
        }
        // (2) model parses the schema-valid law.
        let law = match ArticleBasedLaw::from_yaml_str(&content) {
            Ok(law) => law,
            Err(e) => {
                hard_failures.push(format!(
                    "{rel}: model failed to parse a schema-valid law: {e}"
                ));
                continue;
            }
        };
        // (3) re-serialized model still schema-valid (reported). Normalize first:
        // the model emits `None` as explicit `null` (no skip_serializing_if), and
        // `null` ≡ absent for an optional field — normalizing isolates real
        // structural problems from that serialization quirk.
        let reserialized = normalize(&serde_json::to_value(&law).expect("serialize model"));
        if !validation_errors_for(version, &reserialized)
            .expect("compile schema")
            .is_empty()
        {
            not_revalid.push(rel.clone());
        }
        // (4) value-stability (reported): compare modulo $schema meta + key order.
        if let Value::Object(map) = &mut value {
            map.remove("$schema");
        }
        if normalize(&value) != reserialized {
            value_drift.push(rel);
        }
    }

    eprintln!(
        "Tier A: checked {checked} corpus laws | reported: {} not-revalidating, {} value-drift",
        not_revalid.len(),
        value_drift.len()
    );
    for r in &not_revalid {
        eprintln!("  not-revalidating: {r}");
    }
    for r in &value_drift {
        eprintln!("  value-drift: {r}");
    }

    assert!(checked > 0, "no corpus laws checked — corpus path wrong?");
    assert!(
        hard_failures.is_empty(),
        "Tier A hard failures (schema⋂model disagreement on real laws):\n{}",
        hard_failures.join("\n")
    );
}

/// Tier B — synthetic fixtures exercising constructs the corpus may not hit.
///
/// `valid/`   : schema accepts ∧ model parses ∧ re-serialized still schema-valid.
/// `invalid/` : schema rejects (asserted). The model verdict is measured: a
///              wrongly-accepted fixture must be listed in `KNOWN_GAPS`, and a
///              listed fixture the model now rejects must be removed.
#[test]
fn tier_b_fixtures() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance");

    // valid/
    for path in fixtures_in(&base.join("valid")) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let content = std::fs::read_to_string(&path).expect("read fixture");
        let value: Value = serde_yaml_ng::from_str(&content).expect("fixture is YAML");
        let version = detect_version(&value)
            .unwrap_or_else(|| panic!("valid fixture {name} lacks a recognised $schema"));

        assert!(
            schema_accepts(version, &value),
            "valid fixture {name}: schema {version} should accept it but did not: {:?}",
            validation_errors_for(version, &value).unwrap()
        );
        let law = ArticleBasedLaw::from_yaml_str(&content)
            .unwrap_or_else(|e| panic!("valid fixture {name}: model should parse it: {e}"));
        // Normalize away `null`-for-None before re-validating (see Tier A note).
        let reserialized = normalize(&serde_json::to_value(&law).expect("serialize"));
        assert!(
            schema_accepts(version, &reserialized),
            "valid fixture {name}: re-serialized model no longer schema-valid: {:?}",
            validation_errors_for(version, &reserialized).unwrap()
        );
    }

    // invalid/
    let mut undocumented: Vec<String> = Vec::new();
    let mut stale: Vec<String> = Vec::new();
    let mut documented = 0usize;
    let mut conformant = 0usize;
    let mut seen: Vec<String> = Vec::new();
    for path in fixtures_in(&base.join("invalid")) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        seen.push(name.clone());
        let content = std::fs::read_to_string(&path).expect("read fixture");
        let value: Value = serde_yaml_ng::from_str(&content).expect("fixture is YAML");
        let version = detect_version(&value)
            .unwrap_or_else(|| panic!("invalid fixture {name} lacks a recognised $schema"));

        assert!(
            !schema_accepts(version, &value),
            "invalid fixture {name}: schema {version} unexpectedly accepted it — fixture is not actually invalid"
        );

        let model_accepts = ArticleBasedLaw::from_yaml_str(&content).is_ok();
        let listed = KNOWN_GAPS.contains(&name.as_str());
        match (model_accepts, listed) {
            (true, false) => undocumented.push(name), // new soundness gap
            (false, true) => stale.push(name),        // gap closed, list is stale
            (false, false) => conformant += 1,        // model agrees with schema
            (true, true) => documented += 1,          // documented gap
        }
    }

    // Every KNOWN_GAPS entry must name a real invalid/ fixture, else a typo'd or
    // orphaned entry rots silently (it can never be flagged stale/undocumented).
    let orphaned: Vec<&&str> = KNOWN_GAPS
        .iter()
        .filter(|g| !seen.contains(&g.to_string()))
        .collect();

    eprintln!(
        "Tier B: invalid fixtures — {conformant} conformant (model also rejects), {documented} documented gaps"
    );

    assert!(
        orphaned.is_empty(),
        "KNOWN_GAPS entries with no matching invalid/ fixture (remove or fix the filename): {orphaned:?}"
    );
    assert!(
        undocumented.is_empty(),
        "soundness gap(s) not documented in KNOWN_GAPS (model accepts what the schema rejects): {undocumented:?}"
    );
    assert!(
        stale.is_empty(),
        "stale KNOWN_GAPS entries — model now rejects these, remove them from KNOWN_GAPS: {stale:?}"
    );
}

fn fixtures_in(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read fixture dir {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yaml"))
        .collect();
    out.sort();
    out
}

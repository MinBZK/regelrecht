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
//! Three tiers (the first two mirroring BDD bucket-A/B):
//!   - Tier A — differential over the real corpus + a roundtrip fidelity report.
//!   - Tier B — synthetic fixtures exercising constructs the corpus may not hit.
//!   - Tier C — coverage gate deciding which fixtures must exist, read off the
//!     schema rather than off what someone thought to write down.
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

/// Paths where `after` fails to carry what `before` stated — a dropped key, a
/// shortened array, or a changed scalar. Keys `after` adds are ignored (the
/// model may materialize a schema default).
fn lost_values(before: &Value, after: &Value, path: &str) -> Vec<String> {
    let mut lost = Vec::new();
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                let child = format!("{path}.{key}");
                match b.get(key) {
                    Some(other) => lost.extend(lost_values(value, other, &child)),
                    None => lost.push(format!("  {child}: dropped")),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (value, other)) in a.iter().zip(b).enumerate() {
                lost.extend(lost_values(value, other, &format!("{path}[{index}]")));
            }
        }
        // `100000000` vs `100000000.0` is the same number written two ways —
        // JSON has one number type and YAML's integer/float split does not
        // survive the trip. Only a changed magnitude is a loss.
        (Value::Number(a), Value::Number(b)) if a.as_f64() == b.as_f64() => {}
        (a, b) if a != b => lost.push(format!("  {path}: {a} became {b}")),
        _ => {}
    }
    lost
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
        // No-loss is a hard assertion here (unlike Tier A, where value-stability
        // is only reported). This is what gives Tier C its teeth: a field the
        // model silently drops survives every other check — the document still
        // parses, and the re-serialized document is still schema-valid, because
        // every one of these fields is optional. Only comparing the values
        // catches the loss.
        //
        // Containment, not equality: the model may *add* a schema default it
        // materializes (`accepted: false`), which is not a loss. It may not drop
        // or change anything the fixture states.
        let lost = lost_values(&normalize(&value), &reserialized, "");
        assert!(
            lost.is_empty(),
            "valid fixture {name}: model round-trip loses values the fixture states:\n{}",
            lost.join("\n")
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

// ---------------------------------------------------------------------------
// Tier C — schema-property coverage
// ---------------------------------------------------------------------------

/// Schema properties deliberately not exercised by a `valid/` fixture, each with
/// the reason. Everything else must appear in a fixture — that is the gate.
///
/// An entry here is a claim that the model may lose this field. Removing an
/// entry means writing a fixture that uses it; the Tier-B round-trip assertion
/// then proves the model actually carries it.
const UNCOVERED_SCHEMA_KEYS: &[(&str, &str)] = &[
    // The 13 operation definitions each carry an optional `legal_basis`. The
    // model's `ActionOperation` is an internally-tagged enum whose variants
    // would each need the field, forcing `..` into every pattern in the
    // evaluator. Measured, not fixed — the field is metadata, dropped silently.
    (
        "ageOperation.legal_basis",
        "model drops it (see note above)",
    ),
    ("arithmeticOperation.legal_basis", "model drops it"),
    ("comparisonOperation.legal_basis", "model drops it"),
    ("dateAddOperation.legal_basis", "model drops it"),
    ("dateConstructOperation.legal_basis", "model drops it"),
    ("dateDiffOperation.legal_basis", "model drops it"),
    ("dayOfWeekOperation.legal_basis", "model drops it"),
    ("ifOperation.legal_basis", "model drops it"),
    ("inOperation.legal_basis", "model drops it"),
    ("listOperation.legal_basis", "model drops it"),
    ("logicalOperation.legal_basis", "model drops it"),
    ("notOperation.legal_basis", "model drops it"),
    ("roundingOperation.legal_basis", "model drops it"),
];

/// Every `(owner, property)` pair the schema defines, where `owner` is the
/// nearest enclosing named `definitions/*` entry (or `#root`).
///
/// Positional paths (`articles[].machine_readable…`) were the obvious unit and
/// the wrong one: a definition reused at fifteen places produces fifteen paths
/// that a single fixture can never all reach. Keying on the definition collapses
/// them to what is actually under test — the shape.
fn schema_property_keys(schema: &Value) -> std::collections::BTreeSet<String> {
    let mut keys = std::collections::BTreeSet::new();
    collect_keys(schema, schema, "#root", &mut Vec::new(), &mut keys);
    keys
}

fn definition<'a>(schema: &'a Value, reference: &str) -> Option<&'a Value> {
    schema
        .get("definitions")?
        .get(reference.rsplit('/').next()?)
}

fn collect_keys(
    schema: &Value,
    node: &Value,
    owner: &str,
    stack: &mut Vec<String>,
    keys: &mut std::collections::BTreeSet<String>,
) {
    let Some(map) = node.as_object() else { return };

    if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
        let name = reference
            .rsplit('/')
            .next()
            .unwrap_or(reference)
            .to_string();
        if stack.contains(&name) {
            return; // recursive definition: the shape below is already recorded
        }
        if let Some(target) = definition(schema, reference) {
            stack.push(name.clone());
            collect_keys(schema, target, &name, stack, keys);
            stack.pop();
        }
        return;
    }
    for keyword in ["oneOf", "anyOf", "allOf"] {
        if let Some(branches) = map.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                collect_keys(schema, branch, owner, stack, keys);
            }
        }
    }
    if let Some(then) = map.get("then") {
        collect_keys(schema, then, owner, stack, keys);
    }
    if let Some(properties) = map.get("properties").and_then(Value::as_object) {
        for (key, sub) in properties {
            keys.insert(format!("{owner}.{key}"));
            collect_keys(schema, sub, owner, stack, keys);
        }
    }
    if let Some(additional) = map.get("additionalProperties").filter(|v| v.is_object()) {
        collect_keys(schema, additional, owner, stack, keys);
    }
    if let Some(items) = map.get("items") {
        collect_keys(schema, items, owner, stack, keys);
    }
}

/// The `(owner, property)` pairs a concrete document exercises, found by walking
/// schema and instance together.
fn covered_schema_keys(schema: &Value, instance: &Value) -> std::collections::BTreeSet<String> {
    let mut keys = std::collections::BTreeSet::new();
    walk_covered(schema, schema, instance, "#root", 0, &mut keys);
    keys
}

/// Cheap discriminator for `oneOf`/`anyOf` branches: a branch is only descended
/// into when the instance satisfies its `required` list and any `const`/`enum`
/// on the properties it does carry. That is enough to pick the right arm of the
/// operation union, which discriminates on the `operation` enum.
fn branch_matches(schema: &Value, branch: &Value, instance: &Value) -> bool {
    let Some(object) = instance.as_object() else {
        return true;
    };
    let resolved = branch
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| definition(schema, r))
        .unwrap_or(branch);
    let Some(map) = resolved.as_object() else {
        return true;
    };
    if let Some(required) = map.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(name) {
                return false;
            }
        }
    }
    if let Some(properties) = map.get("properties").and_then(Value::as_object) {
        for (key, sub) in properties {
            let Some(present) = object.get(key) else {
                continue;
            };
            if let Some(constant) = sub.get("const") {
                if present != constant {
                    return false;
                }
            }
            if let Some(allowed) = sub.get("enum").and_then(Value::as_array) {
                if !allowed.contains(present) {
                    return false;
                }
            }
        }
    }
    true
}

fn walk_covered(
    schema: &Value,
    node: &Value,
    instance: &Value,
    owner: &str,
    depth: usize,
    keys: &mut std::collections::BTreeSet<String>,
) {
    const MAX_DEPTH: usize = 64; // recursive definitions (nested operations)
    let Some(map) = node.as_object() else { return };
    if depth > MAX_DEPTH {
        return;
    }

    if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
        if let Some(target) = definition(schema, reference) {
            let name = reference.rsplit('/').next().unwrap_or(reference);
            walk_covered(schema, target, instance, name, depth + 1, keys);
        }
        return;
    }
    for keyword in ["oneOf", "anyOf"] {
        if let Some(branches) = map.get(keyword).and_then(Value::as_array) {
            for branch in branches
                .iter()
                .filter(|b| branch_matches(schema, b, instance))
            {
                walk_covered(schema, branch, instance, owner, depth + 1, keys);
            }
        }
    }
    if let Some(branches) = map.get("allOf").and_then(Value::as_array) {
        for branch in branches {
            walk_covered(schema, branch, instance, owner, depth + 1, keys);
        }
    }
    if let Some(then) = map.get("then") {
        let condition = map.get("if").cloned().unwrap_or(Value::Null);
        if branch_matches(schema, &condition, instance) {
            walk_covered(schema, then, instance, owner, depth + 1, keys);
        }
    }
    if let Some(object) = instance.as_object() {
        let properties = map.get("properties").and_then(Value::as_object);
        if let Some(properties) = properties {
            for (key, sub) in properties {
                if let Some(present) = object.get(key) {
                    keys.insert(format!("{owner}.{key}"));
                    walk_covered(schema, sub, present, owner, depth + 1, keys);
                }
            }
        }
        if let Some(additional) = map.get("additionalProperties").filter(|v| v.is_object()) {
            for (key, present) in object {
                if !properties.is_some_and(|p| p.contains_key(key)) {
                    walk_covered(schema, additional, present, owner, depth + 1, keys);
                }
            }
        }
    }
    if let (Some(array), Some(items)) = (instance.as_array(), map.get("items")) {
        for element in array {
            walk_covered(schema, items, element, owner, depth + 1, keys);
        }
    }
}

/// Tier C — every schema property must be exercised by a `valid/` fixture.
///
/// This is the structural answer to the failure mode the fixture set cannot
/// catch by itself: a schema-valid law the model refuses (or silently mangles)
/// stays invisible for as long as no fixture happens to use that field. Tier B
/// only proves things about the fixtures that exist; Tier C decides which
/// fixtures must exist, from the schema rather than from what someone thought
/// of. Together with Tier B's round-trip equality, a new schema property cannot
/// land without either a fixture proving the model carries it or an explicit,
/// reasoned entry in `UNCOVERED_SCHEMA_KEYS`.
///
/// The corpus deliberately does not count as coverage: only fixtures carry the
/// hard round-trip assertion, and a corpus law can stop using a field at any
/// time.
#[test]
fn tier_c_schema_property_coverage() {
    let schemas = regelrecht_engine::schema::load_schemas().expect("load schemas");
    let latest = *regelrecht_engine::schema::embedded_versions()
        .last()
        .expect("at least one embedded schema");
    let schema = &schemas[latest];

    let all = schema_property_keys(schema);
    let mut covered = std::collections::BTreeSet::new();
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/valid");
    for path in fixtures_in(&base) {
        let content = std::fs::read_to_string(&path).expect("read fixture");
        let value: Value = serde_yaml_ng::from_str(&content).expect("fixture is YAML");
        // Coverage is measured against the latest schema, so only fixtures
        // pinned to it contribute.
        if detect_version(&value) == Some(latest) {
            covered.extend(covered_schema_keys(schema, &value));
        }
    }

    let allowed: std::collections::BTreeSet<&str> =
        UNCOVERED_SCHEMA_KEYS.iter().map(|(k, _)| *k).collect();
    let missing: Vec<&String> = all
        .iter()
        .filter(|k| !covered.contains(*k) && !allowed.contains(k.as_str()))
        .collect();
    let stale: Vec<&str> = allowed
        .iter()
        .copied()
        .filter(|k| covered.contains(*k))
        .collect();
    let orphaned: Vec<&str> = allowed
        .iter()
        .copied()
        .filter(|k| !all.contains(*k))
        .collect();

    eprintln!(
        "Tier C ({latest}): {}/{} schema properties covered by fixtures, {} allowed uncovered",
        covered.intersection(&all).count(),
        all.len(),
        allowed.len()
    );

    assert!(
        orphaned.is_empty(),
        "UNCOVERED_SCHEMA_KEYS entries that name no schema property (typo, or the schema moved on): {orphaned:?}"
    );
    assert!(
        stale.is_empty(),
        "stale UNCOVERED_SCHEMA_KEYS entries — a fixture now covers these, remove them: {stale:?}"
    );
    assert!(
        missing.is_empty(),
        "schema properties no valid/ fixture exercises. Add a fixture using each \
         (that is the point of the tier), or document it in UNCOVERED_SCHEMA_KEYS with a reason:\n{}",
        missing
            .iter()
            .map(|k| format!("  {k}"))
            .collect::<Vec<_>>()
            .join("\n")
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

//! Integration tests for execution tracing.
//!
//! Verifies that the trace output matches the expected box-drawing format
//! for the zorgtoeslag (healthcare allowance) scenario.

mod common;

use regelrecht_engine::{LawExecutionService, Value};
use std::collections::BTreeMap;
use walkdir::WalkDir;

/// Load all regulation YAML files into the service.
fn load_all_regulations(service: &mut LawExecutionService) -> Result<usize, String> {
    let regulation_dir = common::regulation_base_path().join("nl");

    if !regulation_dir.exists() {
        return Err(format!(
            "Regulation directory not found: {}",
            regulation_dir.display()
        ));
    }

    let mut count = 0;
    for entry in WalkDir::new(&regulation_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            if service.load_law(&content).is_ok() {
                count += 1;
            }
        }
    }

    Ok(count)
}

/// Helper to create a record HashMap from key-value pairs.
fn record(entries: Vec<(&str, Value)>) -> BTreeMap<String, Value> {
    entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}

/// Set up a service with all regulations loaded and zorgtoeslag data registered.
fn setup_zorgtoeslag_service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    load_all_regulations(&mut service).expect("Failed to load regulations");

    // Register raw data sources (matching BDD scenario data)
    let personal = record(vec![
        ("bsn", Value::String("999993653".to_string())),
        ("geboortedatum", Value::String("2005-01-01".to_string())),
    ]);
    let relationship = record(vec![
        ("bsn", Value::String("999993653".to_string())),
        ("partnerschap_type", Value::String("GEEN".to_string())),
    ]);
    let insurance = record(vec![
        ("bsn", Value::String("999993653".to_string())),
        ("polis_status", Value::String("ACTIEF".to_string())),
        ("verdragsinschrijving", Value::Bool(false)),
    ]);
    let box1 = record(vec![
        ("bsn", Value::String("999993653".to_string())),
        ("loon_uit_dienstbetrekking", Value::Int(79547)),
        ("uitkeringen_en_pensioenen", Value::Int(0)),
        ("winst_uit_onderneming", Value::Int(0)),
        ("resultaat_overige_werkzaamheden", Value::Int(0)),
        ("eigen_woning", Value::Int(0)),
        ("buitenlands_inkomen", Value::Int(0)),
    ]);
    let box2 = record(vec![
        ("bsn", Value::String("999993653".to_string())),
        ("reguliere_voordelen", Value::Int(0)),
        ("vervreemdingsvoordelen", Value::Int(0)),
    ]);
    let box3 = record(vec![
        ("bsn", Value::String("999993653".to_string())),
        ("spaargeld", Value::Int(0)),
        ("beleggingen", Value::Int(0)),
        ("onroerend_goed", Value::Int(0)),
        ("schulden", Value::Int(0)),
    ]);
    let detenties = record(vec![
        ("bsn", Value::String("999993653".to_string())),
        ("detentiestatus", Value::Null),
        ("inrichting_type", Value::Null),
        ("zorgtype", Value::Null),
        ("juridische_grondslag", Value::Null),
    ]);

    service
        .register_dict_source("personal_data", "bsn", vec![personal])
        .expect("Failed to register personal_data");
    service
        .register_dict_source("relationship_data", "bsn", vec![relationship])
        .expect("Failed to register relationship_data");
    service
        .register_dict_source("insurance", "bsn", vec![insurance])
        .expect("Failed to register insurance");
    service
        .register_dict_source("box1", "bsn", vec![box1])
        .expect("Failed to register box1");
    service
        .register_dict_source("box2", "bsn", vec![box2])
        .expect("Failed to register box2");
    service
        .register_dict_source("box3", "bsn", vec![box3])
        .expect("Failed to register box3");
    service
        .register_dict_source("detenties", "bsn", vec![detenties])
        .expect("Failed to register detenties");

    service
}

#[test]
fn test_zorgtoeslag_trace_output_format() {
    let service = setup_zorgtoeslag_service();

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    let result = service
        .evaluate_law_output_with_trace(
            "wet_op_de_zorgtoeslag",
            "hoogte_zorgtoeslag",
            params,
            "2025-01-01",
        )
        .expect("Law evaluation should succeed");

    // Verify the computation result
    let hoogte = result.outputs.get("hoogte_zorgtoeslag");
    assert!(
        hoogte.is_some(),
        "Expected hoogte_zorgtoeslag output, got: {:?}",
        result.outputs.keys().collect::<Vec<_>>()
    );

    // The trace should be populated
    let trace = result.trace.expect("Trace should be populated");

    // Render the box-drawing trace
    let rendered = trace.render_box_drawing();

    // Snapshot comparison against expected trace output.
    // BTreeMap ensures deterministic iteration order, so trace output is
    // identical across platforms without any normalization.
    let expected = include_str!("expected_zorgtoeslag_trace.txt");
    assert_eq!(
        rendered.trim(),
        expected.trim(),
        "Trace output does not match expected snapshot.\n\n--- ACTUAL ---\n{}\n--- EXPECTED ---\n{}",
        rendered,
        expected
    );
}

#[test]
fn test_zorgtoeslag_trace_result_matches_non_trace() {
    let service = setup_zorgtoeslag_service();

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    // Execute with trace
    let traced_result = service
        .evaluate_law_output_with_trace(
            "wet_op_de_zorgtoeslag",
            "hoogte_zorgtoeslag",
            params.clone(),
            "2025-01-01",
        )
        .expect("Traced evaluation should succeed");

    // Execute without trace
    let normal_result = service
        .evaluate_law_output(
            "wet_op_de_zorgtoeslag",
            "hoogte_zorgtoeslag",
            params,
            "2025-01-01",
        )
        .expect("Normal evaluation should succeed");

    // Results should be identical
    assert_eq!(
        traced_result.outputs, normal_result.outputs,
        "Traced and non-traced results should be identical"
    );
}

#[test]
fn test_trace_disabled_by_default() {
    let service = setup_zorgtoeslag_service();

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    // Normal evaluation should not have a trace
    let result = service
        .evaluate_law_output(
            "wet_op_de_zorgtoeslag",
            "hoogte_zorgtoeslag",
            params,
            "2025-01-01",
        )
        .expect("Evaluation should succeed");

    assert!(
        result.trace.is_none(),
        "Normal evaluation should not produce a trace"
    );
}

#[test]
fn test_simple_law_trace() {
    // Test tracing with a simple single-law scenario (standard premium)
    let mut service = LawExecutionService::new();
    load_all_regulations(&mut service).expect("Failed to load regulations");

    let result = service
        .evaluate_law_output_with_trace(
            "regeling_standaardpremie",
            "standaardpremie",
            BTreeMap::new(),
            "2025-01-01",
        )
        .expect("Standard premium evaluation should succeed");

    let trace = result.trace.expect("Trace should be populated");
    let rendered = trace.render_box_drawing();

    // Snapshot comparison against expected trace output
    let expected = include_str!("expected_standaardpremie_trace.txt");
    assert_eq!(
        rendered.trim(),
        expected.trim(),
        "Trace output does not match expected snapshot.\n\n--- ACTUAL ---\n{}\n--- EXPECTED ---\n{}",
        rendered,
        expected
    );
}

/// Every step of a real cross-law evaluation names the provision it came from,
/// and a step inside another law names *that* law (RFC-039).
///
/// This is the claim the RFC rests on, so it is asserted against the zorgtoeslag
/// chain rather than a synthetic tree: the anchor has to survive a cross-law
/// hop and come back, which is where a naive implementation loses it.
#[test]
fn every_step_is_anchored_to_the_provision_it_came_from() {
    let service = setup_zorgtoeslag_service();

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    let result = service
        .evaluate_law_output_with_trace(
            "wet_op_de_zorgtoeslag",
            "hoogte_zorgtoeslag",
            params,
            "2025-01-01",
        )
        .expect("Law evaluation should succeed");
    let root = result.trace.expect("traced evaluation produces a trace");

    fn walk<'a>(
        node: &'a regelrecht_engine::trace::PathNode,
        out: &mut Vec<&'a regelrecht_engine::trace::PathNode>,
    ) {
        out.push(node);
        for child in &node.children {
            walk(child, out);
        }
    }
    let mut nodes = Vec::new();
    walk(&root, &mut nodes);

    // The root is pushed before any article is selected, so it is the one step
    // that legitimately has no provision yet. Everything under it has one.
    let unanchored: Vec<&str> = nodes
        .iter()
        .skip(1)
        .filter(|n| n.anchor.is_none())
        .map(|n| n.name.as_str())
        .collect();
    assert!(
        unanchored.is_empty(),
        "steps without an anchor: {unanchored:?}"
    );

    // An anchor names a law and an article, and the article is the one the
    // engine was in, not the one the caller started from.
    let laws: std::collections::BTreeSet<&str> = nodes
        .iter()
        .filter_map(|n| n.anchor.as_ref()?.law_id.as_deref())
        .collect();
    assert!(
        laws.contains("wet_op_de_zorgtoeslag"),
        "the calling law should appear: {laws:?}"
    );
    assert!(
        laws.len() > 1,
        "a cross-law chain should anchor steps in more than one law, got {laws:?}"
    );

    // The anchor carries what makes a step clickable through to the statute.
    let with_article = nodes
        .iter()
        .filter(|n| n.anchor.as_ref().is_some_and(|a| a.article.is_some()))
        .count();
    assert!(
        with_article > 0,
        "at least one step should name its article number"
    );
}

/// What the corpus cites travels with the step that carries it out (RFC-039),
/// and stays distinct from the anchor: the anchor is where the engine was, the
/// legal basis is the provision the modeller holds the action to. Here they
/// refer to the same article and the citation is the finer of the two, naming
/// the lid the engine cannot infer on its own.
#[test]
fn an_action_carries_the_provision_the_corpus_cites() {
    let service = setup_zorgtoeslag_service();

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    let result = service
        .evaluate_law_output_with_trace(
            "wet_op_de_zorgtoeslag",
            "hoogte_zorgtoeslag",
            params,
            "2025-01-01",
        )
        .expect("Law evaluation should succeed");
    let root = result.trace.expect("traced evaluation produces a trace");

    fn walk<'a>(
        node: &'a regelrecht_engine::trace::PathNode,
        out: &mut Vec<&'a regelrecht_engine::trace::PathNode>,
    ) {
        out.push(node);
        for child in &node.children {
            walk(child, out);
        }
    }
    let mut nodes = Vec::new();
    walk(&root, &mut nodes);

    let step = nodes
        .iter()
        .find(|n| n.name == "hoogte_zorgtoeslag" && n.legal_basis.is_some())
        .expect("the toeslag action states the provision it carries out");
    let basis = step.legal_basis.as_ref().unwrap();

    assert_eq!(basis.article.as_deref(), Some("2"));
    assert_eq!(basis.paragraph.as_deref(), Some("1"));
    assert_eq!(basis.bwb_id.as_deref(), Some("BWBR0018451"));
    // The juriconnect carries `z`/`g`, so it resolves to the text as it stood
    // on this law version's date rather than to the text of today. It is the
    // only part of a citation that carries time at all.
    assert_eq!(
        basis.juriconnect.as_deref(),
        Some("jci1.3:c:BWBR0018451&artikel=2&lid=1&z=2025-01-01&g=2025-01-01")
    );
    assert!(
        basis
            .explanation
            .as_deref()
            .is_some_and(|e| e.contains("standaardpremie")),
        "the citation carries the modeller's wording: {:?}",
        basis.explanation
    );

    // A citation names a provision, not the file it was loaded from.
    assert_eq!(basis.law_id, None, "a citation carries no corpus id");
    assert_eq!(basis.valid_from, None, "a citation carries no load date");

    // The anchor is the engine's own account, and it is coarser: it knows the
    // article it was evaluating, never the lid.
    let anchor = step.anchor.as_ref().expect("the step is anchored too");
    assert_eq!(anchor.law_id.as_deref(), Some("wet_op_de_zorgtoeslag"));
    assert_eq!(anchor.article.as_deref(), Some("2"));
    assert_eq!(
        anchor.paragraph, None,
        "the engine does not guess a lid it was never told"
    );

    // Every action that states a basis gets one, not just the first: stamping
    // only the first action would otherwise pass unnoticed.
    let cited: Vec<&str> = nodes
        .iter()
        .filter(|n| n.legal_basis.is_some())
        .map(|n| n.name.as_str())
        .collect();
    assert_eq!(
        cited,
        vec!["hoogte_zorgtoeslag", "heeft_recht_op_zorgtoeslag"],
        "both actions of article 2 cite their basis"
    );

    // The entitlement test draws on three provisions, so it cites the article
    // and no lid. A citation is allowed to be coarse; it is not allowed to
    // claim a precision the rule does not have.
    let recht = nodes
        .iter()
        .find(|n| n.name == "heeft_recht_op_zorgtoeslag")
        .and_then(|n| n.legal_basis.as_ref())
        .expect("the entitlement action cites its basis");
    assert_eq!(recht.article.as_deref(), Some("2"));
    assert_eq!(
        recht.paragraph, None,
        "a rule spanning three provisions cites no single lid"
    );
}

/// A trace travels as a document with its version inside it, not as a bare
/// step (RFC-039). Consumers read `root`.
#[test]
fn a_trace_is_published_as_a_versioned_document() {
    let service = setup_zorgtoeslag_service();

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    let result = service
        .evaluate_law_output_with_trace(
            "wet_op_de_zorgtoeslag",
            "hoogte_zorgtoeslag",
            params,
            "2025-01-01",
        )
        .expect("Law evaluation should succeed");

    let doc = regelrecht_engine::trace::TraceDocument::new(
        result.trace.expect("traced evaluation produces a trace"),
    );
    let json = serde_json::to_value(&doc).expect("serializes");

    // The two top-level keys the schema requires, and nothing else.
    let obj = json.as_object().expect("an object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["root", "trace_version"]);
    assert_eq!(obj["trace_version"], serde_json::json!(1));

    // The root is a step, and it is anchored all the way down.
    assert!(obj["root"].get("node_type").is_some(), "root is a step");
    assert!(obj["root"].get("node_id").is_some(), "root is addressable");

    // It reads back as a document.
    let parsed: regelrecht_engine::trace::TraceDocument =
        serde_json::from_value(json).expect("deserializes");
    assert_eq!(parsed.trace_version, doc.trace_version);
    assert_eq!(parsed.root.node_id, doc.root.node_id);
}

/// A step whose value the law declares with a unit reports that unit, so a
/// reader sees an amount rather than a bare count of cents (RFC-023, RFC-039).
#[test]
fn a_declared_value_reports_its_unit() {
    let service = setup_zorgtoeslag_service();

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    let result = service
        .evaluate_law_output_with_trace(
            "wet_op_de_zorgtoeslag",
            "hoogte_zorgtoeslag",
            params,
            "2025-01-01",
        )
        .expect("Law evaluation should succeed");
    let root = result.trace.expect("traced evaluation produces a trace");

    fn walk<'a>(
        n: &'a regelrecht_engine::trace::PathNode,
        out: &mut Vec<&'a regelrecht_engine::trace::PathNode>,
    ) {
        out.push(n);
        for c in &n.children {
            walk(c, out);
        }
    }
    let mut nodes = Vec::new();
    walk(&root, &mut nodes);

    let with_unit: Vec<(&str, &str)> = nodes
        .iter()
        .filter_map(|n| Some((n.name.as_str(), n.type_spec.as_ref()?.unit.as_deref()?)))
        .collect();
    assert!(
        !with_unit.is_empty(),
        "no step reported a declared unit; zorgtoeslag has amounts in eurocent"
    );
    assert!(
        with_unit.iter().any(|(_, u)| *u == "eurocent"),
        "expected an amount in eurocent, got {with_unit:?}"
    );
}

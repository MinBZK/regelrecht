//! Integration tests for execution tracing.
//!
//! Verifies that the trace output matches the expected box-drawing format
//! for the zorgtoeslag (healthcare allowance) scenario.

use regelrecht_engine::{LawExecutionService, Value};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// Load all regulation YAML files into the service.
fn load_all_regulations(service: &mut LawExecutionService) -> Result<usize, String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let regulation_dir = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("regulation").join("nl"))
        .ok_or_else(|| "Could not find regulation directory".to_string())?;

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
fn record(entries: Vec<(&str, Value)>) -> HashMap<String, Value> {
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

    let mut params = HashMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    let result = service
        .evaluate_law_output_with_trace(
            "zorgtoeslagwet",
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

    // Snapshot comparison against expected trace output
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

    let mut params = HashMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    // Execute with trace
    let traced_result = service
        .evaluate_law_output_with_trace(
            "zorgtoeslagwet",
            "hoogte_zorgtoeslag",
            params.clone(),
            "2025-01-01",
        )
        .expect("Traced evaluation should succeed");

    // Execute without trace
    let normal_result = service
        .evaluate_law_output("zorgtoeslagwet", "hoogte_zorgtoeslag", params, "2025-01-01")
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

    let mut params = HashMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    // Normal evaluation should not have a trace
    let result = service
        .evaluate_law_output("zorgtoeslagwet", "hoogte_zorgtoeslag", params, "2025-01-01")
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
            HashMap::new(),
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

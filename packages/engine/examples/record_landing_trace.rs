//! Record the trace the landing page replays.
//!
//! The landing page shows a law being executed. It does not run the engine: the
//! docs site ships no engine, and a full evaluation takes about three
//! milliseconds anyway, which is far too fast to watch. So the engine runs here,
//! at build time, and the page replays what it recorded at a pace a person can
//! follow. Every step the visitor sees was really taken.
//!
//! The inputs are the ones in the scenario that already lives beside the law
//! (`corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature`),
//! which CI runs on every push. So the number on the landing page is the number
//! a failing test would catch, and the Gherkin the page displays is a real file
//! rather than an illustration. The BSN is the fictional test number that
//! scenario uses.
//!
//! Usage:
//!   cargo run --example record_landing_trace -- <output.json>

use regelrecht_engine::{LawExecutionService, Value};
use std::collections::BTreeMap;
use std::path::Path;
use walkdir::WalkDir;

const BSN: &str = "999993653";
const LAW: &str = "wet_op_de_zorgtoeslag";
const OUTPUT: &str = "hoogte_zorgtoeslag";
const DATE: &str = "2025-01-01";

/// What the scenario asserts. Recording a trace that computes something else
/// means the corpus moved, and the landing page should not quietly show the new
/// number: a demo that silently re-baselines proves nothing.
const EXPECTED: i64 = 209692;

fn record(entries: Vec<(&str, Value)>) -> BTreeMap<String, Value> {
    entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(out_path) = args.first() else {
        eprintln!("Usage: record_landing_trace <output.json>");
        std::process::exit(1);
    };

    let mut service = LawExecutionService::new();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let regulation_dir = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("corpus").join("regulation").join("nl"))
        .expect("Could not find regulation directory");

    let mut loaded = 0;
    for entry in WalkDir::new(&regulation_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            if let Ok(content) = std::fs::read_to_string(path) {
                if service.load_law(&content).is_ok() {
                    loaded += 1;
                }
            }
        }
    }
    eprintln!("Loaded {loaded} regulations");

    for (name, row) in [
        (
            "personal_data",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("geboortedatum", Value::String("2005-01-01".to_string())),
            ]),
        ),
        (
            "relationship_data",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("partnerschap_type", Value::String("GEEN".to_string())),
            ]),
        ),
        (
            "insurance",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("polis_status", Value::String("ACTIEF".to_string())),
                ("verdragsinschrijving", Value::Bool(false)),
            ]),
        ),
        (
            "box1",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("loon_uit_dienstbetrekking", Value::Int(79547)),
                ("uitkeringen_en_pensioenen", Value::Int(0)),
                ("winst_uit_onderneming", Value::Int(0)),
                ("resultaat_overige_werkzaamheden", Value::Int(0)),
                ("eigen_woning", Value::Int(0)),
                ("buitenlands_inkomen", Value::Int(0)),
            ]),
        ),
        (
            "box2",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("reguliere_voordelen", Value::Int(0)),
                ("vervreemdingsvoordelen", Value::Int(0)),
            ]),
        ),
        (
            "box3",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("spaargeld", Value::Int(0)),
                ("beleggingen", Value::Int(0)),
                ("onroerend_goed", Value::Int(0)),
                ("schulden", Value::Int(0)),
            ]),
        ),
        (
            "detenties",
            record(vec![
                ("bsn", Value::String(BSN.to_string())),
                ("detentiestatus", Value::Null),
                ("inrichting_type", Value::Null),
                ("zorgtype", Value::Null),
                ("juridische_grondslag", Value::Null),
            ]),
        ),
    ] {
        service
            .register_dict_source(name, "bsn", vec![row])
            .unwrap_or_else(|e| panic!("could not register {name}: {e}"));
    }

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String(BSN.to_string()));

    let result = service
        .evaluate_law_output_with_trace(LAW, OUTPUT, params, DATE)
        .expect("the landing-page evaluation should succeed");

    match result.outputs.get(OUTPUT) {
        Some(Value::Int(n)) if *n == EXPECTED => {}
        other => {
            eprintln!(
                "refusing to record: expected {OUTPUT} = {EXPECTED}, got {other:?}.\n\
                 The scenario beside the law asserts {EXPECTED}. Fix the corpus or \n\
                 update both the scenario and this recorder together."
            );
            std::process::exit(1);
        }
    }

    let trace = result
        .trace
        .expect("a traced evaluation produces a trace document");
    let document = regelrecht_engine::trace::TraceDocument::new(trace);

    let json = serde_json::to_string_pretty(&document).expect("the trace serializes");
    std::fs::write(out_path, json).unwrap_or_else(|e| panic!("could not write {out_path}: {e}"));

    eprintln!("Recorded {OUTPUT} = {EXPECTED} to {out_path}");
}

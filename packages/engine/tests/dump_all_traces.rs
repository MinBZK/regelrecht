//! Dump execution traces for all zorgtoeslag scenarios.
//!
//! Generates box-drawing traces for each scenario matching the POC,
//! printing them to stdout for comparison and analysis.

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

/// Register a dict source on the service, replacing any previous registration.
fn register_source(
    service: &mut LawExecutionService,
    name: &str,
    records: Vec<HashMap<String, Value>>,
) {
    service
        .register_dict_source(name, "bsn", records)
        .unwrap_or_else(|e| panic!("Failed to register {}: {}", name, e));
}

/// Scenario configuration
struct Scenario {
    name: &'static str,
    date: &'static str,
    /// Which output to trace
    output: &'static str,
    geboortedatum: &'static str,
    partnerschap_type: &'static str,
    polis_status: &'static str,
    loon: i64,
    uitkeringen: i64,
    winst: i64,
    resultaat: i64,
    eigen_woning: i64,
    reguliere_voordelen: i64,
    vervreemdingsvoordelen: i64,
    spaargeld: i64,
    beleggingen: i64,
    onroerend_goed: i64,
    schulden: i64,
    /// Optional DUO data
    duo_inschrijvingen: Option<(&'static str,)>,
    duo_studiefinanciering: Option<(i64,)>,
}

fn run_scenario(scenario: &Scenario) {
    let bsn = "999993653";

    let mut service = LawExecutionService::new();
    load_all_regulations(&mut service).expect("Failed to load regulations");

    // Register personal_data
    register_source(
        &mut service,
        "personal_data",
        vec![record(vec![
            ("bsn", Value::String(bsn.to_string())),
            (
                "geboortedatum",
                Value::String(scenario.geboortedatum.to_string()),
            ),
        ])],
    );

    // Register relationship_data
    register_source(
        &mut service,
        "relationship_data",
        vec![record(vec![
            ("bsn", Value::String(bsn.to_string())),
            (
                "partnerschap_type",
                Value::String(scenario.partnerschap_type.to_string()),
            ),
        ])],
    );

    // Register insurance
    register_source(
        &mut service,
        "insurance",
        vec![record(vec![
            ("bsn", Value::String(bsn.to_string())),
            (
                "polis_status",
                Value::String(scenario.polis_status.to_string()),
            ),
            ("verdragsinschrijving", Value::Bool(false)),
        ])],
    );

    // Register box1
    register_source(
        &mut service,
        "box1",
        vec![record(vec![
            ("bsn", Value::String(bsn.to_string())),
            ("loon_uit_dienstbetrekking", Value::Int(scenario.loon)),
            (
                "uitkeringen_en_pensioenen",
                Value::Int(scenario.uitkeringen),
            ),
            ("winst_uit_onderneming", Value::Int(scenario.winst)),
            (
                "resultaat_overige_werkzaamheden",
                Value::Int(scenario.resultaat),
            ),
            ("eigen_woning", Value::Int(scenario.eigen_woning)),
            ("buitenlands_inkomen", Value::Int(0)),
        ])],
    );

    // Register box2
    register_source(
        &mut service,
        "box2",
        vec![record(vec![
            ("bsn", Value::String(bsn.to_string())),
            (
                "reguliere_voordelen",
                Value::Int(scenario.reguliere_voordelen),
            ),
            (
                "vervreemdingsvoordelen",
                Value::Int(scenario.vervreemdingsvoordelen),
            ),
        ])],
    );

    // Register box3
    register_source(
        &mut service,
        "box3",
        vec![record(vec![
            ("bsn", Value::String(bsn.to_string())),
            ("spaargeld", Value::Int(scenario.spaargeld)),
            ("beleggingen", Value::Int(scenario.beleggingen)),
            ("onroerend_goed", Value::Int(scenario.onroerend_goed)),
            ("schulden", Value::Int(scenario.schulden)),
        ])],
    );

    // Register detenties
    register_source(
        &mut service,
        "detenties",
        vec![record(vec![
            ("bsn", Value::String(bsn.to_string())),
            ("detentiestatus", Value::Null),
            ("inrichting_type", Value::Null),
            ("zorgtype", Value::Null),
            ("juridische_grondslag", Value::Null),
        ])],
    );

    // Register optional DUO data
    if let Some((onderwijstype,)) = scenario.duo_inschrijvingen {
        register_source(
            &mut service,
            "inschrijvingen",
            vec![record(vec![
                ("bsn", Value::String(bsn.to_string())),
                ("onderwijstype", Value::String(onderwijstype.to_string())),
            ])],
        );
    }

    if let Some((aantal,)) = scenario.duo_studiefinanciering {
        register_source(
            &mut service,
            "studiefinanciering",
            vec![record(vec![
                ("bsn", Value::String(bsn.to_string())),
                ("aantal_studerend_gezin", Value::Int(aantal)),
            ])],
        );
    }

    // Execute with trace
    let mut params = HashMap::new();
    params.insert("bsn".to_string(), Value::String(bsn.to_string()));

    let result = service
        .evaluate_law_output_with_trace("zorgtoeslagwet", scenario.output, params, scenario.date)
        .unwrap_or_else(|e| panic!("Evaluation failed for '{}': {}", scenario.name, e));

    // Print header
    println!();
    println!("{}", "=".repeat(80));
    println!("SCENARIO: {}", scenario.name);
    println!("Date: {}, Output: {}", scenario.date, scenario.output);
    println!("{}", "=".repeat(80));

    // Print trace
    if let Some(trace) = &result.trace {
        println!("{}", trace.render_box_drawing());
    } else {
        println!("[NO TRACE GENERATED]");
    }

    // Print result
    println!("--- Result ---");
    for (key, value) in &result.outputs {
        match value {
            Value::Int(n) => println!(
                "  {}: {} (eurocent = {:.2} euro)",
                key,
                n,
                *n as f64 / 100.0
            ),
            Value::Float(f) => {
                let rounded = f.round() as i64;
                println!(
                    "  {}: {:.5} (rounded: {} eurocent = {:.2} euro)",
                    key,
                    f,
                    rounded,
                    rounded as f64 / 100.0
                );
            }
            Value::Bool(b) => println!("  {}: {}", key, b),
            other => println!("  {}: {:?}", key, other),
        }
    }
    println!("{}", "=".repeat(80));
}

#[test]
fn dump_all_zorgtoeslag_traces() {
    let scenarios = vec![
        // =====================================================================
        // 2025 scenarios
        // =====================================================================
        Scenario {
            name: "Person over 18 entitled (2025)",
            date: "2025-01-01",
            output: "hoogte_zorgtoeslag",
            geboortedatum: "2005-01-01",
            partnerschap_type: "GEEN",
            polis_status: "ACTIEF",
            loon: 79547,
            uitkeringen: 0,
            winst: 0,
            resultaat: 0,
            eigen_woning: 0,
            reguliere_voordelen: 0,
            vervreemdingsvoordelen: 0,
            spaargeld: 0,
            beleggingen: 0,
            onroerend_goed: 0,
            schulden: 0,
            duo_inschrijvingen: None,
            duo_studiefinanciering: None,
        },
        Scenario {
            name: "Person under 18 not entitled (2025)",
            date: "2025-01-01",
            output: "heeft_recht_op_zorgtoeslag",
            geboortedatum: "2008-01-01",
            partnerschap_type: "GEEN",
            polis_status: "ACTIEF",
            loon: 0,
            uitkeringen: 0,
            winst: 0,
            resultaat: 0,
            eigen_woning: 0,
            reguliere_voordelen: 0,
            vervreemdingsvoordelen: 0,
            spaargeld: 0,
            beleggingen: 0,
            onroerend_goed: 0,
            schulden: 0,
            duo_inschrijvingen: None,
            duo_studiefinanciering: None,
        },
        Scenario {
            name: "Low income single entitled (2025)",
            date: "2025-01-01",
            output: "hoogte_zorgtoeslag",
            geboortedatum: "1998-01-01",
            partnerschap_type: "GEEN",
            polis_status: "ACTIEF",
            loon: 20000,
            uitkeringen: 0,
            winst: 0,
            resultaat: 0,
            eigen_woning: 0,
            reguliere_voordelen: 0,
            vervreemdingsvoordelen: 0,
            spaargeld: 10000,
            beleggingen: 0,
            onroerend_goed: 0,
            schulden: 0,
            duo_inschrijvingen: None,
            duo_studiefinanciering: None,
        },
        Scenario {
            name: "Student with study financing entitled (2025)",
            date: "2025-01-01",
            output: "hoogte_zorgtoeslag",
            geboortedatum: "2004-01-01",
            partnerschap_type: "GEEN",
            polis_status: "ACTIEF",
            loon: 15000,
            uitkeringen: 0,
            winst: 0,
            resultaat: 0,
            eigen_woning: 0,
            reguliere_voordelen: 0,
            vervreemdingsvoordelen: 0,
            spaargeld: 0,
            beleggingen: 0,
            onroerend_goed: 0,
            schulden: 0,
            duo_inschrijvingen: Some(("WO",)),
            duo_studiefinanciering: Some((0,)),
        },
        // =====================================================================
        // 2024 scenarios
        // =====================================================================
        Scenario {
            name: "Person over 18 entitled (2024)",
            date: "2024-01-01",
            output: "hoogte_zorgtoeslag",
            geboortedatum: "2005-01-01",
            partnerschap_type: "GEEN",
            polis_status: "ACTIEF",
            loon: 79547,
            uitkeringen: 0,
            winst: 0,
            resultaat: 0,
            eigen_woning: 0,
            reguliere_voordelen: 0,
            vervreemdingsvoordelen: 0,
            spaargeld: 0,
            beleggingen: 0,
            onroerend_goed: 0,
            schulden: 0,
            duo_inschrijvingen: None,
            duo_studiefinanciering: None,
        },
        Scenario {
            name: "Person under 18 not entitled (2024)",
            date: "2024-01-01",
            output: "heeft_recht_op_zorgtoeslag",
            geboortedatum: "2007-01-01",
            partnerschap_type: "GEEN",
            polis_status: "ACTIEF",
            loon: 0,
            uitkeringen: 0,
            winst: 0,
            resultaat: 0,
            eigen_woning: 0,
            reguliere_voordelen: 0,
            vervreemdingsvoordelen: 0,
            spaargeld: 0,
            beleggingen: 0,
            onroerend_goed: 0,
            schulden: 0,
            duo_inschrijvingen: None,
            duo_studiefinanciering: None,
        },
    ];

    for scenario in &scenarios {
        run_scenario(scenario);
    }
}

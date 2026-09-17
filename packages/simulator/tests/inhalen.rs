//! Termijnen die vóór het besluit zouden vervallen, worden **ingehaald**.
//!
//! Een te laat besluit blijft een besluit: het schema dat vóór de besluitdag
//! begint laat het besluit niet omvallen, maar laat de termijnen die al
//! vervallen hadden moeten zijn op de dag van het besluit vervallen. Het scenario
//! `scenarios/inhaaltermijnen.yaml` draagt de bedragen; hier staat wat een
//! scenariobestand niet kan meten — de vervaldata in het gram, de oorspronkelijke
//! dag erbij, en de regel in het journaal.

use regelrecht_simulator::{regulation_root, JournalKind, Scenario, ScenarioRun, Value};
use std::path::Path;

fn run() -> ScenarioRun {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("inhaaltermijnen.yaml");
    let run = Scenario::load(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());
    run
}

/// **Termijn 1 vervalt op de besluitdag, 2-4 op hun eigen dag.** Volgnummers en
/// bedragen blijven gelijk; alleen de ingehaalde termijn draagt een
/// oorspronkelijke vervaldatum.
#[test]
fn de_eerste_termijn_wordt_op_de_besluitdag_ingehaald() {
    let run = run();
    let termijnen = &run.decisions[0].decretogram.obligations;

    let dagen: Vec<(i64, String, Option<String>)> = termijnen
        .iter()
        .map(|due| {
            (
                due.volgnummer,
                due.vervaldatum.to_string(),
                due.oorspronkelijke_vervaldatum.map(|dag| dag.to_string()),
            )
        })
        .collect();
    assert_eq!(
        dagen,
        vec![
            (1, "2024-01-15".to_string(), Some("2024-01-01".to_string())),
            (2, "2024-04-01".to_string(), None),
            (3, "2024-07-01".to_string(), None),
            (4, "2024-10-01".to_string(), None),
        ]
    );
    assert!(
        termijnen.iter().all(|due| due.bedrag == Value::Int(30_000)),
        "inhalen verandert geen bedrag"
    );
}

/// **Het journaal zegt bij de betaling dat ze ingehaald is.**
#[test]
fn het_journaal_noemt_de_inhaalbetaling() {
    let run = run();
    let betalingen: Vec<&str> = run
        .journal
        .iter()
        .filter(|entry| entry.kind == JournalKind::Betaling)
        .map(|entry| entry.description.as_str())
        .collect();

    assert_eq!(betalingen.len(), 4, "vier termijnen, vier betalingen");
    assert!(
        betalingen[0].contains("ingehaald (oorspronkelijk 2024-01-01)"),
        "de eerste betaling hoort ingehaald te heten, kreeg: {}",
        betalingen[0]
    );
    assert!(
        betalingen[1..]
            .iter()
            .all(|regel| !regel.contains("ingehaald")),
        "de andere termijnen vielen op hun eigen dag: {betalingen:?}"
    );
}

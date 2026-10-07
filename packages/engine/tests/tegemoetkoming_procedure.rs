//! De Awir als "Awb van de toeslagen": een eigen procedure met de fasen
//! VOORSCHOT en TOEKENNING, waarop de Awir en de Awb haken.
//!
//! De wetten hieronder zijn een verkleinde versie van het prototype uit de
//! ontwerpnotitie (`packages/cel/ONTWERP-proces-zorgtoeslag.md`, "Uitwerking na
//! de spike"). Ze zijn synthetisch: ze bewijzen wat de engine moet kunnen, niet
//! wat de wet zegt.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{EngineError, LawExecutionService, Value};
use std::collections::BTreeMap;

/// Awr 21: het inkomensgegeven, hier een vast bedrag.
const AWR: &str = r#"
$id: awr_proto
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2020-01-01'
articles:
  - number: '21'
    text: inkomensgegeven
    machine_readable:
      execution:
        parameters: [{name: bsn, type: string, required: true}]
        output: [{name: inkomensgegeven, type: number}]
        actions: [{output: inkomensgegeven, value: 3000000}]
"#;

/// De Awir met art. 8 (het toetsingsinkomen) vóór art. 16 (het geschatte
/// toetsingsinkomen, een haak op VOORSCHOT): beide produceren
/// `toetsingsinkomen`, en art. 16 staat als laatste in het bestand.
const AWIR: &str = r#"
$id: awir_proto
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2020-01-01'
procedure:
  - id: tegemoetkoming
    applies_to: {legal_character: BESCHIKKING}
    stages:
      - name: VOORSCHOT
        requires: [{name: dagtekening_voorschot, type: date}]
      - name: TOEKENNING
        requires: [{name: dagtekening_toekenning, type: date}]
articles:
  - number: '8'
    text: Toetsingsinkomen is het inkomensgegeven over het berekeningsjaar.
    machine_readable:
      execution:
        parameters: [{name: bsn, type: string, required: true}]
        input:
          - name: inkomensgegeven
            type: number
            source: {regulation: awr_proto, output: inkomensgegeven, parameters: {bsn: $bsn}}
        output: [{name: toetsingsinkomen, type: number}]
        actions: [{output: toetsingsinkomen, value: $inkomensgegeven}]
  - number: '16'
    text: voorschot tot het bedrag waarop de tegemoetkoming vermoedelijk zal worden vastgesteld
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {legal_character: BESCHIKKING, stage: VOORSCHOT}
      execution:
        parameters:
          - {name: vermoedelijk_toetsingsinkomen, type: number, required: true}
        output: [{name: toetsingsinkomen, type: number}]
        actions: [{output: toetsingsinkomen, value: $vermoedelijk_toetsingsinkomen}]
"#;

/// Zorgtoeslagwet 2: de hoogte, op het toetsingsinkomen uit de Awir.
const WZT: &str = r#"
$id: wzt_proto
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2020-01-01'
articles:
  - number: '2'
    text: aanspraak op een zorgtoeslag
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          decision_type: TOEKENNING
          procedure_id: tegemoetkoming
        parameters: [{name: bsn, type: string, required: true}]
        input:
          - name: toetsingsinkomen
            type: number
            source: {regulation: awir_proto, output: toetsingsinkomen, parameters: {bsn: $bsn}}
        output:
          - {name: hoogte_zorgtoeslag, type: number}
        actions:
          - output: hoogte_zorgtoeslag
            value: {operation: SUBTRACT, values: [5000000, $toetsingsinkomen]}
"#;

fn service(laws: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for law in laws {
        service.load_law(law).unwrap();
    }
    service
}

fn params(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn bsn() -> (&'static str, Value) {
    ("bsn", Value::String("999993653".to_string()))
}

/// De hoogte bij een inkomen: 5000000 min het inkomen (een rekenregel die
/// alleen laat zien welk inkomen is gebruikt).
fn hoogte(inkomen: i64) -> Value {
    Value::Int(5_000_000 - inkomen)
}

#[test]
fn a_source_to_an_output_a_hook_also_produces_reads_the_ordinary_article() {
    // Awir 16 staat als laatste in het bestand. Voorheen won het laatst
    // geladen artikel, en vroeg Wzt 2 buiten elke procedure om een
    // vermoedelijk_toetsingsinkomen dat er niet is.
    let service = service(&[AWR, AWIR, WZT]);
    let result = service
        .evaluate_law(
            "wzt_proto",
            &["hoogte_zorgtoeslag"],
            params(&[bsn()]),
            "2025-01-01",
        )
        .unwrap();
    assert_eq!(result.outputs["hoogte_zorgtoeslag"], hoogte(3_000_000));
}

#[test]
fn an_output_two_ordinary_articles_produce_is_ambiguous_by_name() {
    let law = r#"
$id: twee_producenten
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2020-01-01'
articles:
  - number: '3'
    text: bevoegd is het CIZ
    machine_readable:
      execution:
        output: [{name: bevoegd_gezag, type: string}]
        actions: [{output: bevoegd_gezag, value: CIZ}]
  - number: '4'
    text: bevoegd is het zorgkantoor
    machine_readable:
      execution:
        output: [{name: bevoegd_gezag, type: string}]
        actions: [{output: bevoegd_gezag, value: Zorgkantoor}]
"#;
    // Laden mag: de Wlz laat elk artikel zijn eigen bevoegd gezag noemen.
    let service = service(&[law]);
    let err = service
        .evaluate_law(
            "twee_producenten",
            &["bevoegd_gezag"],
            BTreeMap::new(),
            "2025-01-01",
        )
        .unwrap_err();
    match &err {
        EngineError::AmbiguousOutput {
            law_id,
            output,
            articles,
        } => {
            assert_eq!(law_id, "twee_producenten");
            assert_eq!(output, "bevoegd_gezag");
            assert_eq!(articles, &["3".to_string(), "4".to_string()]);
        }
        other => panic!("expected AmbiguousOutput, got {other:?}"),
    }
    let message = err.to_string();
    assert!(message.contains("(3, 4)"), "{message}");
}

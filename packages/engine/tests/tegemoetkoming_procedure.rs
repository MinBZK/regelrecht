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

use regelrecht_engine::{
    EngineError, ExecutionOutcome, LawExecutionService, OutputProvenance, StageState, Value,
};
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
        is: BESLUIT
        requires: [{name: dagtekening_voorschot, type: date}]
      - name: TOEKENNING
        is: BESLUIT
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
        output:
          - {name: toetsingsinkomen, type: number}
          - {name: inkomen_geschat, type: boolean}
        actions:
          - {output: toetsingsinkomen, value: $vermoedelijk_toetsingsinkomen}
          - {output: inkomen_geschat, value: true}
"#;

/// De Awb: de standaardprocedure en art. 6:7, een haak op elk besluit.
const AWB: &str = r#"
$id: awb_proto
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2020-01-01'
procedure:
  - id: beschikking
    default: true
    applies_to: {legal_character: BESCHIKKING}
    stages:
      - name: BESLUIT
      - name: BEKENDMAKING
        requires: [{name: bekendmaking_datum, type: date}]
articles:
  - number: '6:7'
    text: De termijn voor het indienen van een bezwaarschrift bedraagt zes weken.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BESLUIT}
      execution:
        output: [{name: bezwaartermijn_weken, type: number}]
        actions: [{output: bezwaartermijn_weken, value: 6}]
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

fn date(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Een verse toestand aan het begin van `stage` van de procedure
/// `tegemoetkoming`, zoals de cel die bouwt.
fn at(stage: &str) -> Option<StageState> {
    Some(StageState {
        procedure_id: "tegemoetkoming".to_string(),
        contextual_law: "wzt_proto".to_string(),
        current_stage: stage.to_string(),
        accumulated_outputs: BTreeMap::new(),
        parameters: BTreeMap::new(),
    })
}

/// De voorschotfase op een verse toestand: het geschatte inkomen is 2000000.
fn voorschot(service: &LawExecutionService) -> ExecutionOutcome {
    service
        .execute_stage(
            "wzt_proto",
            "hoogte_zorgtoeslag",
            at("VOORSCHOT"),
            params(&[
                bsn(),
                ("vermoedelijk_toetsingsinkomen", Value::Int(2_000_000)),
                ("dagtekening_voorschot", date("2024-12-01")),
            ]),
            "2025-01-01",
        )
        .unwrap()
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

#[test]
fn a_hook_on_besluit_fires_on_a_stage_that_is_a_besluit() {
    // VOORSCHOT is een besluitfase (`is: BESLUIT`), dus Awb 6:7 vuurt erop;
    // Awir 16 vuurt op de naam van de fase zelf.
    let service = service(&[AWB, AWR, AWIR, WZT]);
    match voorschot(&service) {
        ExecutionOutcome::Yielded { state, outputs, .. } => {
            assert_eq!(state.current_stage, "TOEKENNING");
            assert_eq!(outputs["bezwaartermijn_weken"], Value::Int(6));
            assert_eq!(outputs["hoogte_zorgtoeslag"], hoogte(2_000_000));
        }
        other => panic!("expected a yield before TOEKENNING, got {other:?}"),
    }
}

#[test]
fn a_hook_on_besluit_does_not_fire_on_a_stage_that_does_not_say_it_is_one() {
    let awir = AWIR.replace("        is: BESLUIT\n", "");
    let service = service(&[AWB, AWR, &awir, WZT]);
    match voorschot(&service) {
        ExecutionOutcome::Yielded { outputs, .. } => {
            assert!(!outputs.contains_key("bezwaartermijn_weken"), "{outputs:?}");
            assert_eq!(outputs["hoogte_zorgtoeslag"], hoogte(2_000_000));
        }
        other => panic!("expected a yield before TOEKENNING, got {other:?}"),
    }
}

#[test]
fn an_input_a_pre_hook_replaced_at_one_stage_is_resolved_again_at_the_next() {
    // Awir 16 vervangt bij VOORSCHOT het toetsingsinkomen door het geschatte.
    // Bij TOEKENNING vuurt die haak niet, en geldt weer Awir 8: het
    // geschatte inkomen mag niet als parameter doorlekken.
    let service = service(&[AWB, AWR, AWIR, WZT]);
    let ExecutionOutcome::Yielded { state, outputs, .. } = voorschot(&service) else {
        panic!("expected a yield before TOEKENNING");
    };
    // Bij het voorschot zelf is het geschatte inkomen gebruikt en te zien.
    assert_eq!(outputs["toetsingsinkomen"], Value::Int(2_000_000));
    assert!(
        !state.accumulated_outputs.contains_key("toetsingsinkomen"),
        "{:?}",
        state.accumulated_outputs
    );
    // Wat de haak daarnaast produceert, en geen invoer vervangt, gaat mee.
    assert_eq!(
        state.accumulated_outputs["inkomen_geschat"],
        Value::Bool(true)
    );
    let outcome = service
        .execute_stage(
            "wzt_proto",
            "hoogte_zorgtoeslag",
            Some(state),
            params(&[bsn(), ("dagtekening_toekenning", date("2026-06-01"))]),
            "2025-01-01",
        )
        .unwrap();
    match outcome {
        ExecutionOutcome::Complete(result) => {
            assert_eq!(result.outputs["hoogte_zorgtoeslag"], hoogte(3_000_000));
            assert_eq!(
                result.resolved_inputs["toetsingsinkomen"],
                Value::Int(3_000_000)
            );
        }
        other => panic!("expected the procedure to complete, got {other:?}"),
    }
}

fn stage_at(
    service: &LawExecutionService,
    stage: &str,
    extra: &[(&str, Value)],
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let mut parameters = params(&[bsn()]);
    parameters.extend(params(extra));
    service.execute_stage_at(
        "wzt_proto",
        "hoogte_zorgtoeslag",
        stage,
        parameters,
        "2025-01-01",
    )
}

#[test]
fn one_stage_runs_on_a_fresh_state_with_the_hooks_of_that_stage() {
    let service = service(&[AWB, AWR, AWIR, WZT]);
    let voorschot = stage_at(
        &service,
        "VOORSCHOT",
        &[
            ("vermoedelijk_toetsingsinkomen", Value::Int(2_000_000)),
            ("dagtekening_voorschot", date("2024-12-01")),
        ],
    )
    .unwrap();
    assert_eq!(voorschot.outputs["hoogte_zorgtoeslag"], hoogte(2_000_000));
    assert_eq!(voorschot.outputs["bezwaartermijn_weken"], Value::Int(6));
    assert_eq!(voorschot.outputs["toetsingsinkomen"], Value::Int(2_000_000));
    assert_eq!(
        voorschot.output_provenance["toetsingsinkomen"],
        OutputProvenance::Reactive {
            law_id: "awir_proto".to_string(),
            article: "16".to_string(),
            hook_point: "pre_actions".to_string(),
        }
    );

    // De toekenning, los van het voorschot: Awir 16 vuurt niet, Awir 8 geldt.
    let toekenning = stage_at(
        &service,
        "TOEKENNING",
        &[("dagtekening_toekenning", date("2026-06-01"))],
    )
    .unwrap();
    assert_eq!(toekenning.outputs["hoogte_zorgtoeslag"], hoogte(3_000_000));
    assert_eq!(toekenning.outputs["bezwaartermijn_weken"], Value::Int(6));
    assert!(!toekenning.outputs.contains_key("inkomen_geschat"));
    assert_eq!(
        toekenning.resolved_inputs["toetsingsinkomen"],
        Value::Int(3_000_000)
    );
}

#[test]
fn one_stage_refuses_a_stage_the_procedure_does_not_have() {
    let service = service(&[AWB, AWR, AWIR, WZT]);
    let err = stage_at(&service, "BEKENDMAKING", &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("BEKENDMAKING"), "{err}");
    assert!(err.contains("tegemoetkoming"), "{err}");
}

#[test]
fn one_stage_refuses_when_a_value_the_stage_requires_is_missing() {
    let service = service(&[AWB, AWR, AWIR, WZT]);
    let err = stage_at(&service, "TOEKENNING", &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("dagtekening_toekenning"), "{err}");
}

#[test]
fn one_stage_refuses_an_article_that_follows_no_procedure() {
    let service = service(&[AWB, AWR, AWIR, WZT]);
    let err = service
        .execute_stage_at(
            "awr_proto",
            "inkomensgegeven",
            "TOEKENNING",
            params(&[bsn()]),
            "2025-01-01",
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("follows no procedure"), "{err}");
}

#[test]
fn one_stage_refuses_a_procedure_that_is_not_loaded() {
    // Zonder de Awir bestaat de procedure `tegemoetkoming` niet: dan mag
    // de fase niet stil zonder procedure worden uitgevoerd.
    let service = service(&[AWB, AWR, WZT]);
    let err = stage_at(&service, "VOORSCHOT", &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("tegemoetkoming"), "{err}");
}

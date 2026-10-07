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
    EngineError, ExecutionOutcome, LawExecutionService, MissingKind, OutputProvenance, StageState,
    Value,
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

/// Een verwijzing naar een uitvoer leest de versie die geldt: dat een latere
/// versie die uitvoer in twee artikelen heeft, maakt de verwijzing op een
/// eerdere datum niet dubbelzinnig.
#[test]
fn a_reference_reads_the_producers_of_the_version_in_force() {
    // De versie van 2026 vraagt een andere parameter dan die van 2020: wie de
    // declaraties van de nieuwste versie leest, geeft de versie die op
    // 2025-06-01 geldt niet wat zij vraagt.
    let doel = |valid_from: &str, parameter: &str, extra: &str| {
        format!(
            r#"
$id: doelwet
regulatory_layer: WET
publication_date: '2020-01-01'
valid_from: '{valid_from}'
articles:
  - number: '1'
    text: bedrag
    machine_readable:
      execution:
        parameters: [{{name: {parameter}, type: string, required: true}}]
        output: [{{name: bedrag, type: number}}]
        actions: [{{output: bedrag, value: 7}}]
{extra}"#
        )
    };
    let tweede = r#"  - number: '2'
    text: ook bedrag
    machine_readable:
      execution:
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: 8}]
"#;
    let vrager = r#"
$id: vrager
regulatory_layer: WET
publication_date: '2020-01-01'
valid_from: '2020-01-01'
articles:
  - number: '1'
    text: vraagt
    machine_readable:
      execution:
        parameters: [{name: bsn, type: string, required: true}]
        input:
          - name: bedrag
            type: number
            source: {regulation: doelwet, output: bedrag, parameters: {bsn: $bsn, kenmerk: $bsn}}
        output: [{name: uitkomst, type: number}]
        actions: [{output: uitkomst, value: $bedrag}]
"#;
    let service = service(&[
        &doel("2020-01-01", "bsn", ""),
        &doel("2026-01-01", "kenmerk", tweede),
        vrager,
    ]);
    let bsn = || BTreeMap::from([("bsn".to_string(), Value::String("1".into()))]);
    let result = service
        .evaluate_law("vrager", &["uitkomst"], bsn(), "2025-06-01")
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(result.outputs["uitkomst"], Value::Int(7));
    // In de versie van 2026 is de uitvoer wel dubbelzinnig: de resolutie
    // zelf weigert haar.
    let err = service
        .evaluate_law("vrager", &["uitkomst"], bsn(), "2026-06-01")
        .unwrap_err();
    assert!(err.to_string().contains("bedrag"), "{err}");
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
    service.execute_stage_at("wzt_proto", "2", stage, parameters, "2025-01-01")
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
            "21",
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

#[test]
fn a_parameter_a_pre_hook_replaced_at_one_stage_is_the_given_one_at_the_next() {
    // Als de vervangen invoer een parameter is: bij de volgende fase geldt
    // weer de opgegeven waarde, niet die van de haak.
    let law = r#"
$id: parameter_stages
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2020-01-01'
procedure:
  - id: twee_fasen
    applies_to: {legal_character: BESCHIKKING}
    stages:
      - name: EERSTE
      - name: TWEEDE
        requires: [{name: datum_tweede, type: date}]
articles:
  - number: '1'
    text: het bedrag op het inkomen
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, procedure_id: twee_fasen}
        parameters: [{name: inkomen, type: number, required: true}]
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: $inkomen}]
  - number: '2'
    text: bij de eerste fase het geschatte inkomen
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {legal_character: BESCHIKKING, stage: EERSTE}
      execution:
        parameters: [{name: geschat, type: number, required: true}]
        output: [{name: inkomen, type: number}]
        actions: [{output: inkomen, value: $geschat}]
"#;
    let service = service(&[law]);
    let first = service
        .execute_stage(
            "parameter_stages",
            "bedrag",
            None,
            params(&[("inkomen", Value::Int(300)), ("geschat", Value::Int(200))]),
            "2025-01-01",
        )
        .unwrap();
    let ExecutionOutcome::Yielded { state, outputs, .. } = first else {
        panic!("expected a yield before TWEEDE");
    };
    assert_eq!(outputs["bedrag"], Value::Int(200));
    let second = service
        .execute_stage(
            "parameter_stages",
            "bedrag",
            Some(state),
            params(&[("datum_tweede", date("2026-01-01"))]),
            "2025-01-01",
        )
        .unwrap();
    let ExecutionOutcome::Complete(result) = second else {
        panic!("expected the procedure to complete");
    };
    assert_eq!(result.outputs["bedrag"], Value::Int(300));
}

#[test]
fn an_open_term_a_pre_hook_replaced_at_one_stage_is_filled_again_at_the_next() {
    let law = r#"
$id: open_term_stages
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2020-01-01'
procedure:
  - id: twee_fasen
    applies_to: {legal_character: BESCHIKKING}
    stages:
      - name: EERSTE
      - name: TWEEDE
        requires: [{name: datum_tweede, type: date}]
articles:
  - number: '1'
    text: het bedrag op het inkomen, bij regeling vast te stellen
    machine_readable:
      open_terms:
        - id: inkomen
          type: number
          required: true
          default:
            actions:
              - output: inkomen
                value: 300
      execution:
        produces: {legal_character: BESCHIKKING, procedure_id: twee_fasen}
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: $inkomen}]
  - number: '2'
    text: bij de eerste fase het geschatte inkomen
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {legal_character: BESCHIKKING, stage: EERSTE}
      execution:
        parameters: [{name: geschat, type: number, required: true}]
        output:
          - {name: inkomen, type: number}
          - {name: inkomen_geschat, type: boolean}
        actions:
          - {output: inkomen, value: $geschat}
          - {output: inkomen_geschat, value: true}
"#;
    let service = service(&[law]);
    let first = service
        .execute_stage(
            "open_term_stages",
            "bedrag",
            None,
            params(&[("geschat", Value::Int(200))]),
            "2025-01-01",
        )
        .unwrap();
    let ExecutionOutcome::Yielded { state, outputs, .. } = first else {
        panic!("expected a yield before TWEEDE");
    };
    assert_eq!(outputs["bedrag"], Value::Int(200));
    // Alleen wat het open begrip vervangt hoort bij de fase; de rest gaat mee.
    assert!(!state.accumulated_outputs.contains_key("inkomen"));
    assert_eq!(
        state.accumulated_outputs["inkomen_geschat"],
        Value::Bool(true)
    );
    let second = service
        .execute_stage(
            "open_term_stages",
            "bedrag",
            Some(state),
            params(&[("datum_tweede", date("2026-01-01"))]),
            "2025-01-01",
        )
        .unwrap();
    let ExecutionOutcome::Complete(result) = second else {
        panic!("expected the procedure to complete");
    };
    assert_eq!(result.outputs["bedrag"], Value::Int(300));
}

/// What `stage_inputs` reports, as (law, article, hook point) per article and
/// (article, name, supplied) per input.
#[allow(clippy::type_complexity)]
fn taking_part(
    stage: &regelrecht_engine::StageInputs,
) -> (
    Vec<(String, String, Option<String>)>,
    Vec<(String, String, bool)>,
) {
    let articles = stage
        .articles
        .iter()
        .map(|a| {
            (
                a.law_id.clone(),
                a.article_number.clone(),
                a.hook.as_ref().map(|h| h.hook_point.as_str().to_string()),
            )
        })
        .collect();
    let inputs = stage
        .inputs
        .iter()
        .map(|i| {
            (
                i.article_number.clone(),
                i.parameter.name.clone(),
                i.supplied,
            )
        })
        .collect();
    (articles, inputs)
}

fn s(x: &str) -> String {
    x.to_string()
}

#[test]
fn a_stage_asks_what_the_article_and_the_hooks_at_that_stage_ask() {
    let service = service(&[AWB, AWR, AWIR, WZT]);
    let voorschot = service
        .stage_inputs(
            "wzt_proto",
            "2",
            "VOORSCHOT",
            &params(&[
                bsn(),
                // An unknown value names nobody: the input is still missing.
                (
                    "vermoedelijk_toetsingsinkomen",
                    Value::unknown(
                        "awir_proto",
                        "vermoedelijk_toetsingsinkomen",
                        MissingKind::NotPassed,
                    ),
                ),
            ]),
            "2025-01-01",
        )
        .unwrap();
    assert_eq!(voorschot.procedure_id, "tegemoetkoming");
    assert_eq!(voorschot.stage, "VOORSCHOT");
    assert_eq!(voorschot.is.as_deref(), Some("BESLUIT"));
    let requires: Vec<&str> = voorschot.requires.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(requires, ["dagtekening_voorschot"]);
    let (articles, inputs) = taking_part(&voorschot);
    assert_eq!(
        articles,
        [
            (s("wzt_proto"), s("2"), None),
            (s("awir_proto"), s("16"), Some(s("pre_actions"))),
            // Awb 6:7 hooks on BESLUIT, and VOORSCHOT is one.
            (s("awb_proto"), s("6:7"), Some(s("post_actions"))),
        ]
    );
    assert_eq!(
        inputs,
        [
            (s("2"), s("bsn"), true),
            (s("16"), s("vermoedelijk_toetsingsinkomen"), false),
        ]
    );

    // At the toekenning Awir 16 does not fire, so its input is not asked.
    let toekenning = service
        .stage_inputs(
            "wzt_proto",
            "2",
            "TOEKENNING",
            &BTreeMap::new(),
            "2025-01-01",
        )
        .unwrap();
    let (articles, inputs) = taking_part(&toekenning);
    assert_eq!(
        articles,
        [
            (s("wzt_proto"), s("2"), None),
            (s("awb_proto"), s("6:7"), Some(s("post_actions"))),
        ]
    );
    assert_eq!(inputs, [(s("2"), s("bsn"), false)]);
    let requires: Vec<&str> = toekenning
        .requires
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(requires, ["dagtekening_toekenning"]);
}

#[test]
fn a_hook_on_besluit_does_not_take_part_in_a_stage_that_is_none() {
    let awir = AWIR.replace("        is: BESLUIT\n", "");
    let service = service(&[AWB, AWR, &awir, WZT]);
    let stage = service
        .stage_inputs(
            "wzt_proto",
            "2",
            "VOORSCHOT",
            &BTreeMap::new(),
            "2025-01-01",
        )
        .unwrap();
    assert_eq!(stage.is, None);
    let (articles, _) = taking_part(&stage);
    assert_eq!(
        articles,
        [
            (s("wzt_proto"), s("2"), None),
            (s("awir_proto"), s("16"), Some(s("pre_actions"))),
        ]
    );
}

#[test]
fn an_article_hooked_before_and_after_a_stage_takes_part_once() {
    // Awir 16 also after the actions at VOORSCHOT: one article, its inputs
    // once.
    let awir = AWIR.replace(
        "          applies_to: {legal_character: BESCHIKKING, stage: VOORSCHOT}\n",
        "          applies_to: {legal_character: BESCHIKKING, stage: VOORSCHOT}\n        - hook_point: post_actions\n          applies_to: {legal_character: BESCHIKKING, stage: VOORSCHOT}\n",
    );
    assert_ne!(awir, AWIR);
    let service = service(&[AWR, &awir, WZT]);
    let stage = service
        .stage_inputs(
            "wzt_proto",
            "2",
            "VOORSCHOT",
            &BTreeMap::new(),
            "2025-01-01",
        )
        .unwrap();
    let (articles, inputs) = taking_part(&stage);
    assert_eq!(
        articles,
        [
            (s("wzt_proto"), s("2"), None),
            (s("awir_proto"), s("16"), Some(s("pre_actions"))),
        ]
    );
    assert_eq!(
        inputs,
        [
            (s("2"), s("bsn"), false),
            (s("16"), s("vermoedelijk_toetsingsinkomen"), false),
        ]
    );
}

#[test]
fn a_stage_is_refused_where_executing_it_would_be() {
    let service = service(&[AWB, AWR, AWIR, WZT]);
    let ask = |law: &str, article: &str, stage: &str| {
        service
            .stage_inputs(law, article, stage, &BTreeMap::new(), "2025-01-01")
            .unwrap_err()
            .to_string()
    };
    let err = ask("wzt_proto", "2", "BEKENDMAKING");
    assert!(err.contains("BEKENDMAKING"), "{err}");
    assert!(err.contains("tegemoetkoming"), "{err}");
    let err = ask("awr_proto", "21", "TOEKENNING");
    assert!(err.contains("follows no procedure"), "{err}");
    let err = ask("wzt_proto", "3", "TOEKENNING");
    assert!(err.contains("no article 3"), "{err}");
    let err = service
        .stage_inputs(
            "wzt_proto",
            "2",
            "TOEKENNING",
            &BTreeMap::new(),
            "1999-01-01",
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("wzt_proto"), "{err}");
}

/// A law with a procedure `voorlopig` whose stage VOORLOPIG says `is: <is>`
/// in the version valid from `valid_from`, an article taking its decision
/// there, and the default procedure with a hook on BESLUIT.
fn dated_procedure(valid_from: &str, is: &str) -> String {
    format!(
        r#"
$id: dated_procedure
regulatory_layer: WET
publication_date: '{valid_from}'
valid_from: '{valid_from}'
procedure:
  - id: standaard
    default: true
    applies_to: {{legal_character: BESCHIKKING}}
    stages:
      - name: BESLUIT
      - name: BEKENDMAKING
  - id: voorlopig
    applies_to: {{legal_character: BESCHIKKING}}
    stages:
      - name: VOORLOPIG
        is: {is}
articles:
  - number: '1'
    text: Er wordt voorlopig beslist.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          procedure_id: voorlopig
        output: [{{name: bedrag, type: number}}]
        actions: [{{output: bedrag, value: 1}}]
  - number: '2'
    text: Op elk besluit volgt een termijn.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {{legal_character: BESCHIKKING, stage: BESLUIT}}
      execution:
        output: [{{name: termijn, type: number}}]
        actions: [{{output: termijn, value: 6}}]
"#
    )
}

#[test]
fn what_a_stage_is_comes_from_the_version_in_force_on_the_date() {
    // In 2024 VOORLOPIG is a BESLUIT, so the hook on BESLUIT fires; in 2025
    // it is a BEKENDMAKING, so it does not. The newest version must not
    // speak for 2024.
    let first = dated_procedure("2024-01-01", "BESLUIT");
    let second = dated_procedure("2025-01-01", "BEKENDMAKING");
    let service = service(&[&first, &second]);
    let termijn = |date: &str| {
        service
            .execute_stage_at("dated_procedure", "1", "VOORLOPIG", BTreeMap::new(), date)
            .unwrap()
            .outputs
            .get("termijn")
            .cloned()
    };
    assert_eq!(termijn("2024-06-01"), Some(Value::Int(6)));
    assert_eq!(termijn("2025-06-01"), None);
    let parts = |date: &str| {
        service
            .stage_inputs("dated_procedure", "1", "VOORLOPIG", &BTreeMap::new(), date)
            .unwrap()
            .articles
            .len()
    };
    assert_eq!(parts("2024-06-01"), 2);
    assert_eq!(parts("2025-06-01"), 1);
}

#[test]
fn one_stage_runs_the_article_named_not_the_one_found_by_output() {
    // Two ordinary articles produce `bedrag`: by output that is ambiguous,
    // by number it is not.
    let law = r#"
$id: twee_producenten
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
procedure:
  - id: standaard
    default: true
    applies_to: {legal_character: BESCHIKKING}
    stages:
      - name: BESLUIT
articles:
  - number: '1'
    text: een
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING}
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: 1}]
  - number: '2'
    text: twee
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING}
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: 2}]
"#;
    let service = service(&[law]);
    let bedrag = |number: &str| {
        service
            .execute_stage_at(
                "twee_producenten",
                number,
                "BESLUIT",
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap()
            .outputs
            .get("bedrag")
            .cloned()
    };
    assert_eq!(bedrag("1"), Some(Value::Int(1)));
    assert_eq!(bedrag("2"), Some(Value::Int(2)));
    let err = service
        .execute_stage_at(
            "twee_producenten",
            "3",
            "BESLUIT",
            BTreeMap::new(),
            "2025-06-01",
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("has no article 3"), "{err}");
}

#[test]
fn the_service_reports_a_stage_alias_that_names_no_stage() {
    let good = dated_procedure("2025-01-01", "BESLUIT");
    assert_eq!(
        service(&[&good]).unknown_stage_aliases(),
        Vec::<String>::new()
    );
    let typo = dated_procedure("2025-01-01", "BESLUT");
    let problems = service(&[&typo]).unknown_stage_aliases();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("`is: BESLUT`"), "{}", problems[0]);
    assert!(problems[0].contains("dated_procedure"), "{}", problems[0]);
}

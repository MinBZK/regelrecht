//! Lazy evaluation (RFC-043): an input is resolved when an operation first
//! reads it, and an article runs only the actions the requested outputs
//! depend on. These tests pin what that buys (a knock-out criterion stops the
//! retrieval behind it) and what it must not change (the outcome does not
//! depend on the order of operands).

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{EngineError, LawExecutionService, Value};
use std::collections::BTreeMap;

const BRP: &str = r#"
$id: lazy_brp
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De basisregistratie bevat de leeftijd.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: leeftijd_register
            type: number
            source: {}
        output:
          - name: leeftijd
            type: number
        actions:
          - output: leeftijd
            value: $leeftijd_register
"#;

const BELASTING: &str = r#"
$id: lazy_belasting
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De inspecteur kent het inkomen.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: inkomen_register
            type: number
            source: {}
        output:
          - name: inkomen
            type: number
        actions:
          - output: inkomen
            value: $inkomen_register
"#;

/// A toeslag whose conditions read the age first or the income first.
fn toeslag(law_id: &str, age_first: bool) -> String {
    let age = "                - operation: GREATER_THAN_OR_EQUAL\n                  subject: $leeftijd\n                  value: 18\n";
    let income = "                - operation: LESS_THAN\n                  subject: $inkomen\n                  value: 100\n";
    let conditions = if age_first {
        format!("{age}{income}")
    } else {
        format!("{income}{age}")
    };
    format!(
        r#"
$id: {law_id}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Aanspraak bestaat voor een meerderjarige met een laag inkomen.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: leeftijd
            type: number
            source:
              regulation: lazy_brp
              output: leeftijd
              parameters:
                bsn: $bsn
          - name: inkomen
            type: number
            source:
              regulation: lazy_belasting
              output: inkomen
              parameters:
                bsn: $bsn
        output:
          - name: voldoet
            type: boolean
        actions:
          - output: voldoet
            value:
              operation: AND
              conditions:
{conditions}"#
    )
}

fn record(bsn: &str, field: &str, value: Option<i64>) -> BTreeMap<String, Value> {
    let mut row = BTreeMap::new();
    row.insert("bsn".to_string(), Value::String(bsn.to_string()));
    row.insert(field.to_string(), value.map_or(Value::Null, Value::Int));
    row
}

/// A service with both registers filled for one person. `inkomen: None`
/// leaves the income register without a row, so the income is unknown.
fn service(leeftijd: i64, inkomen: Option<i64>) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(BRP).unwrap();
    service.load_law(BELASTING).unwrap();
    service.load_law(&toeslag("lazy_toeslag", true)).unwrap();
    service
        .load_law(&toeslag("lazy_toeslag_omgekeerd", false))
        .unwrap();
    service
        .register_dict_source_for_law(
            "lazy_brp",
            "personen",
            "bsn",
            vec![record("1", "leeftijd_register", Some(leeftijd))],
            10,
        )
        .unwrap();
    if let Some(inkomen) = inkomen {
        service
            .register_dict_source_for_law(
                "lazy_belasting",
                "aanslagen",
                "bsn",
                vec![record("1", "inkomen_register", Some(inkomen))],
                10,
            )
            .unwrap();
    }
    service
}

fn bsn() -> BTreeMap<String, Value> {
    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("1".to_string()));
    params
}

fn traced(service: &LawExecutionService, law_id: &str, output: &str) -> (Value, String) {
    let result = service
        .evaluate_law_output_with_trace(law_id, output, bsn(), "2025-01-01")
        .unwrap();
    let rendered = result.trace.as_ref().unwrap().render_box_drawing();
    (result.outputs[output].clone(), rendered)
}

#[test]
fn a_knock_out_criterion_stops_the_retrieval_behind_it() {
    let (voldoet, trace) = traced(&service(17, Some(50)), "lazy_toeslag", "voldoet");
    assert_eq!(voldoet, Value::Bool(false));
    assert!(trace.contains("lazy_brp#leeftijd"), "{trace}");
    assert!(
        !trace.contains("lazy_belasting"),
        "a minor's income must not be consulted:\n{trace}"
    );

    let (voldoet, trace) = traced(&service(30, Some(50)), "lazy_toeslag", "voldoet");
    assert_eq!(voldoet, Value::Bool(true));
    assert!(trace.contains("lazy_belasting#inkomen"), "{trace}");
}

#[test]
fn the_order_of_operands_changes_what_is_fetched_never_the_outcome() {
    for (leeftijd, inkomen) in [
        (17, Some(50)),
        (17, Some(500)),
        (30, Some(50)),
        (30, Some(500)),
        (17, None),
        (30, None),
    ] {
        let service = service(leeftijd, inkomen);
        let (age_first, _) = traced(&service, "lazy_toeslag", "voldoet");
        let (income_first, _) = traced(&service, "lazy_toeslag_omgekeerd", "voldoet");
        assert_eq!(
            age_first, income_first,
            "leeftijd {leeftijd}, inkomen {inkomen:?}"
        );
    }
    // A minor with an unknown income does not qualify, whichever is read first
    // (strong Kleene: false AND unknown is false).
    let service = service(17, None);
    assert_eq!(
        traced(&service, "lazy_toeslag_omgekeerd", "voldoet").0,
        Value::Bool(false)
    );
    // An adult with an unknown income is unknown.
    let service = self::service(30, None);
    assert!(traced(&service, "lazy_toeslag", "voldoet").0.is_unknown());
}

#[test]
fn an_input_nobody_reads_cannot_fail_the_execution() {
    // The income register holds an explicit null for an input that is never
    // absent: reading it is an error at the boundary (RFC-036).
    let mut service = service(17, None);
    service
        .register_dict_source_for_law(
            "lazy_belasting",
            "aanslagen",
            "bsn",
            vec![record("1", "inkomen_register", None)],
            10,
        )
        .unwrap();
    let result = service
        .evaluate_law_output("lazy_toeslag", "voldoet", bsn(), "2025-01-01")
        .unwrap();
    assert_eq!(result.outputs["voldoet"], Value::Bool(false));

    // An adult's income is read, and the same register then fails the call.
    let mut adult = self::service(30, None);
    adult
        .register_dict_source_for_law(
            "lazy_belasting",
            "aanslagen",
            "bsn",
            vec![record("1", "inkomen_register", None)],
            10,
        )
        .unwrap();
    assert!(adult
        .evaluate_law_output("lazy_toeslag", "voldoet", bsn(), "2025-01-01")
        .is_err());
}

const TWICE: &str = r#"
$id: lazy_twice
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De leeftijd telt dubbel.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: leeftijd
            type: number
            source:
              regulation: lazy_brp
              output: leeftijd
              parameters:
                bsn: $bsn
        output:
          - name: dubbel
            type: number
          - name: per_lid
            type: number
        actions:
          - output: dubbel
            value:
              operation: ADD
              values: [$leeftijd, $leeftijd]
          # A FOREACH that binds `bsn`: the input is still looked up for the
          # article's bsn, not for the loop variable.
          - output: per_lid
            value:
              operation: FOREACH
              collection: ['7', '8']
              as: bsn
              body: $leeftijd
              combine: ADD
"#;

#[test]
fn an_input_read_twice_is_resolved_once() {
    let mut service = service(30, Some(50));
    service.load_law(TWICE).unwrap();
    let (dubbel, trace) = traced(&service, "lazy_twice", "dubbel");
    assert_eq!(dubbel, Value::Int(60));
    assert_eq!(
        trace.matches("Reference: lazy_brp#leeftijd").count(),
        1,
        "{trace}"
    );
}

#[test]
fn a_loop_variable_is_not_a_lookup_key() {
    let mut service = service(30, Some(50));
    service.load_law(TWICE).unwrap();
    let (per_lid, _) = traced(&service, "lazy_twice", "per_lid");
    assert_eq!(per_lid, Value::Int(60));
}

#[test]
/// While an input is being resolved, a reference to it is not the article's
/// to answer, as when inputs were resolved one by one in declaration order:
/// two inputs keyed on each other find neither.
fn inputs_that_look_each_other_up_find_neither() {
    let cycle = r#"
$id: lazy_cycle
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Twee gegevens die elkaar nodig hebben.
    machine_readable:
      execution:
        input:
          - name: a
            type: number
            source:
              regulation: lazy_brp
              output: leeftijd
              parameters:
                bsn: $b
          - name: b
            type: number
            source:
              regulation: lazy_brp
              output: leeftijd
              parameters:
                bsn: $a
        output:
          - name: uitkomst
            type: number
        actions:
          - output: uitkomst
            value: $a
"#;
    let mut service = service(30, Some(50));
    service.load_law(cycle).unwrap();
    let error = service
        .evaluate_law_output("lazy_cycle", "uitkomst", BTreeMap::new(), "2025-01-01")
        .unwrap_err();
    assert!(
        matches!(error, EngineError::VariableNotFound(_)),
        "expected the other input to be missing, got {error:?}"
    );
}

#[test]
fn a_voided_output_is_not_computed() {
    // Computing `aanspraak` would read the income, and an explicit null there
    // is an error. The void is checked first, so the answer is the ground.
    let voided = r#"
$id: lazy_void
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Aanspraak naar inkomen.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: inkomen
            type: number
            source:
              regulation: lazy_belasting
              output: inkomen
              parameters:
                bsn: $bsn
        output:
          - name: aanspraak
            type: number
        actions:
          - output: aanspraak
            value: $inkomen
  - number: '2'
    text: In afwijking daarvan bestaat geen aanspraak.
    machine_readable:
      overrides:
        - law: lazy_void
          article: '1'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
"#;
    let mut service = service(30, None);
    service.load_law(voided).unwrap();
    service
        .register_dict_source_for_law(
            "lazy_belasting",
            "aanslagen",
            "bsn",
            vec![record("1", "inkomen_register", None)],
            10,
        )
        .unwrap();
    match service.evaluate_law_output("lazy_void", "aanspraak", bsn(), "2025-01-01") {
        Err(EngineError::OutputVoided { grounds, .. }) => {
            assert_eq!(grounds, "bestaat geen aanspraak");
        }
        other => panic!("expected OutputVoided, got {other:?}"),
    }
}

#[test]
fn a_required_parameter_nothing_reads_is_not_needed() {
    // RFC-043: what the requested output does not read cannot fail it, a
    // missing required parameter included. The parameter is still required
    // for every output that reads it.
    let law = r#"
$id: lazy_required
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Twee uitkomsten.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
          - name: bedrag
            type: number
            required: true
        output:
          - name: vast
            type: number
          - name: verdubbeld
            type: number
        actions:
          - output: vast
            value: 1
          - output: verdubbeld
            value:
              operation: MULTIPLY
              values: [$bedrag, 2]
"#;
    let mut service = LawExecutionService::new();
    service.load_law(law).unwrap();
    let result = service
        .evaluate_law_output("lazy_required", "vast", bsn(), "2025-01-01")
        .unwrap();
    assert_eq!(result.outputs["vast"], Value::Int(1));
    assert!(service
        .evaluate_law_output("lazy_required", "verdubbeld", bsn(), "2025-01-01")
        .is_err());
}

#[test]
fn an_open_term_nobody_reads_is_not_resolved() {
    let law = r#"
$id: lazy_open_term
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief wordt bij regeling vastgesteld.
    machine_readable:
      open_terms:
        - id: tarief
          type: number
          required: false
          delegation_type: MINISTERIELE_REGELING
          default:
            actions:
              - output: tarief
                value: 3
      execution:
        output:
          - name: vast
            type: number
          - name: met_tarief
            type: number
        actions:
          - output: vast
            value: 1
          - output: met_tarief
            value:
              operation: MULTIPLY
              values: [$tarief, 2]
"#;
    let mut service = LawExecutionService::new();
    service.load_law(law).unwrap();
    let run = |output: &str| {
        let result = service
            .evaluate_law_output_with_trace("lazy_open_term", output, BTreeMap::new(), "2025-01-01")
            .unwrap();
        let trace = result.trace.as_ref().unwrap().render_box_drawing();
        (result.outputs[output].clone(), trace)
    };
    let (vast, trace) = run("vast");
    assert_eq!(vast, Value::Int(1));
    assert!(!trace.contains("tarief"), "{trace}");
    let (met_tarief, trace) = run("met_tarief");
    assert_eq!(met_tarief, Value::Int(6));
    assert!(
        trace.contains("Open term 'tarief' using default value"),
        "{trace}"
    );
}

#[test]
fn a_pre_actions_hook_receives_the_input_it_declares() {
    let triggering = r#"
$id: lazy_hook_trigger
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: leeftijd
            type: number
            source:
              regulation: lazy_brp
              output: leeftijd
              parameters:
                bsn: $bsn
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
    let hook = r#"
$id: lazy_hook
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Voor de beschikking wordt de leeftijd vastgelegd.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        parameters:
          - name: leeftijd
            type: number
            required: true
        output:
          - name: vastgelegde_leeftijd
            type: number
        actions:
          - output: vastgelegde_leeftijd
            value: $leeftijd
"#;
    let mut service = service(30, Some(50));
    service.load_law(triggering).unwrap();
    service.load_law(hook).unwrap();
    let result = service
        .evaluate_law_output("lazy_hook_trigger", "bedrag", bsn(), "2025-01-01")
        .unwrap();
    assert_eq!(result.outputs["bedrag"], Value::Int(100));
    assert_eq!(result.outputs["vastgelegde_leeftijd"], Value::Int(30));
}

// ---------------------------------------------------------------------------
// Precedence between names, as it was when inputs were resolved up front
// ---------------------------------------------------------------------------

const ECHO: &str = r#"
$id: lazy_echo
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Geeft terug wat het krijgt.
    machine_readable:
      execution:
        parameters:
          - name: v
            type: number
            required: true
        output:
          - name: echo
            type: number
        actions:
          - output: echo
            value: $v
"#;

fn constant(law_id: &str, output: &str, value: i64) -> String {
    format!(
        r#"
$id: {law_id}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een vaste waarde.
    machine_readable:
      execution:
        output:
          - name: {output}
            type: number
        actions:
          - output: {output}
            value: {value}
"#
    )
}

fn value_of(
    service: &LawExecutionService,
    law_id: &str,
    output: &str,
    params: BTreeMap<String, Value>,
) -> Value {
    service
        .evaluate_law_output(law_id, output, params, "2025-01-01")
        .unwrap()
        .outputs[output]
        .clone()
}

#[test]
fn a_pre_hook_output_shadows_an_input_for_the_actions_and_the_post_hook_alike() {
    let pre_hook = r#"
$id: lazy_pre
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Vooraf vastgesteld.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: x
            type: number
        actions:
          - output: x
            value: 99
"#;
    let post_hook = r#"
$id: lazy_post
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Achteraf vermeld.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        parameters:
          - name: x
            type: number
            required: true
        output:
          - name: vermeld
            type: number
        actions:
          - output: vermeld
            value: $x
"#;
    let trigger = r#"
$id: lazy_shadow
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het besluit.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: x
            type: number
            source: {}
        output:
          - name: y
            type: number
        actions:
          - output: y
            value: $x
"#;
    let mut service = LawExecutionService::new();
    service.load_law(pre_hook).unwrap();
    service.load_law(post_hook).unwrap();
    service.load_law(trigger).unwrap();
    service
        .register_dict_source_for_law(
            "lazy_shadow",
            "register",
            "bsn",
            vec![record("1", "x", Some(5))],
            10,
        )
        .unwrap();
    let result = service
        .evaluate_law_output("lazy_shadow", "y", bsn(), "2025-01-01")
        .unwrap();
    assert_eq!(result.outputs["y"], Value::Int(99));
    assert_eq!(result.outputs["vermeld"], Value::Int(99));
}

#[test]
fn an_open_term_comes_before_an_input_of_the_same_name() {
    let law = r#"
$id: lazy_term_input
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Twee bronnen voor een naam.
    machine_readable:
      open_terms:
        - id: x
          type: number
          required: false
          delegation_type: MINISTERIELE_REGELING
          default:
            actions:
              - output: x
                value: 7
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: x
            type: number
            source: {}
        output:
          - name: y
            type: number
        actions:
          - output: y
            value: $x
"#;
    let mut service = LawExecutionService::new();
    service.load_law(law).unwrap();
    service
        .register_dict_source_for_law(
            "lazy_term_input",
            "register",
            "bsn",
            vec![record("1", "x", Some(5))],
            10,
        )
        .unwrap();
    assert_eq!(
        value_of(&service, "lazy_term_input", "y", bsn()),
        Value::Int(7)
    );
}

#[test]
fn of_two_inputs_with_one_name_the_last_declared_counts() {
    let law = r#"
$id: lazy_duplicate
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Twee keer dezelfde input.
    machine_readable:
      execution:
        input:
          - name: x
            type: number
            source:
              regulation: lazy_een
              output: een
          - name: x
            type: number
            source:
              regulation: lazy_twee
              output: twee
        output:
          - name: y
            type: number
        actions:
          - output: y
            value: $x
"#;
    let mut service = LawExecutionService::new();
    service.load_law(&constant("lazy_een", "een", 1)).unwrap();
    service.load_law(&constant("lazy_twee", "twee", 2)).unwrap();
    service.load_law(law).unwrap();
    assert_eq!(
        value_of(&service, "lazy_duplicate", "y", BTreeMap::new()),
        Value::Int(2)
    );
}

#[test]
fn an_input_keyed_on_its_own_name_reads_the_parameter() {
    // `v: $v` names the parameter `v`, which the caller left out: the key is
    // unknown, the target is not run, and the input is unknown (RFC-036).
    let law = r#"
$id: lazy_self_keyed
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een input op zijn eigen naam.
    machine_readable:
      execution:
        parameters:
          - name: v
            type: number
            required: false
        input:
          - name: v
            type: number
            source:
              regulation: lazy_echo
              output: echo
              parameters:
                v: $v
        output:
          - name: y
            type: number
        actions:
          - output: y
            value: $v
"#;
    let mut service = LawExecutionService::new();
    service.load_law(ECHO).unwrap();
    service.load_law(law).unwrap();
    assert!(value_of(&service, "lazy_self_keyed", "y", BTreeMap::new()).is_unknown());
}

// ---------------------------------------------------------------------------
// What a hook or an override declares, and voids before computing
// ---------------------------------------------------------------------------

/// The income register with an explicit null: reading `inkomen` then fails.
fn with_null_income(mut service: LawExecutionService) -> LawExecutionService {
    service
        .register_dict_source_for_law(
            "lazy_belasting",
            "aanslagen",
            "bsn",
            vec![record("1", "inkomen_register", None)],
            10,
        )
        .unwrap();
    service
}

#[test]
fn a_passed_null_is_refused_only_where_the_input_is_read() {
    let law = r#"
$id: lazy_passed
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een input die de aanroeper kan meegeven.
    machine_readable:
      execution:
        input:
          - name: bedrag
            type: number
            source: {}
        output:
          - name: vast
            type: number
          - name: dubbel
            type: number
        actions:
          - output: vast
            value: 1
          - output: dubbel
            value:
              operation: MULTIPLY
              values: [$bedrag, 2]
"#;
    let mut service = LawExecutionService::new();
    service.load_law(law).unwrap();
    let mut params = BTreeMap::new();
    params.insert("bedrag".to_string(), Value::Null);
    assert_eq!(
        value_of(&service, "lazy_passed", "vast", params.clone()),
        Value::Int(1)
    );
    let error = service
        .evaluate_law_output("lazy_passed", "dubbel", params, "2025-01-01")
        .unwrap_err();
    assert!(error.to_string().contains("bedrag"), "{error}");
}

#[test]
fn a_post_hook_gets_the_inputs_it_declares_and_nothing_else_runs() {
    let trigger = r#"
$id: lazy_post_trigger
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het besluit.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: leeftijd
            type: number
            source:
              regulation: lazy_brp
              output: leeftijd
              parameters:
                bsn: $bsn
          - name: inkomen
            type: number
            source:
              regulation: lazy_belasting
              output: inkomen
              parameters:
                bsn: $bsn
        output:
          - name: bedrag
            type: number
          - name: los
            type: number
        actions:
          - output: bedrag
            value: 100
          - output: los
            value: $inkomen
"#;
    // The hook declares the lookup key and an input: neither is an output of
    // the article, so neither makes it run in full.
    let hook = r#"
$id: lazy_post_reader
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bij het besluit wordt de leeftijd vermeld.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
          - name: leeftijd
            type: number
            required: true
        output:
          - name: vermelde_leeftijd
            type: number
        actions:
          - output: vermelde_leeftijd
            value: $leeftijd
"#;
    let mut service = with_null_income(service(30, None));
    service.load_law(trigger).unwrap();
    service.load_law(hook).unwrap();
    let result = service
        .evaluate_law_output("lazy_post_trigger", "bedrag", bsn(), "2025-01-01")
        .expect("the income nobody asked for is not read");
    assert_eq!(result.outputs["bedrag"], Value::Int(100));
    assert_eq!(result.outputs["vermelde_leeftijd"], Value::Int(30));
    assert!(!result.outputs.contains_key("los"));
}

#[test]
fn a_replacing_override_gets_the_input_it_declares() {
    // The general rule never reads the age; the special rule, applying within
    // the execution its law starts (RFC-007), does. The age is resolved for it.
    let general = r#"
$id: lazy_replace
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bedrag.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: leeftijd
            type: number
            source:
              regulation: lazy_brp
              output: leeftijd
              parameters:
                bsn: $bsn
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
    let special = r#"
$id: lazy_replace_special
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan is het bedrag twee keer de leeftijd.
    machine_readable:
      overrides:
        - law: lazy_replace
          article: '1'
          output: bedrag
      execution:
        parameters:
          - name: leeftijd
            type: number
            required: true
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value:
              operation: MULTIPLY
              values: [$leeftijd, 2]
  - number: '2'
    text: Het gevolg is het bedrag.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: bedrag
            type: number
            source:
              regulation: lazy_replace
              output: bedrag
              parameters:
                bsn: $bsn
        output:
          - name: gevolg
            type: number
        actions:
          - output: gevolg
            value: $bedrag
"#;
    let mut service = service(30, Some(50));
    service.load_law(general).unwrap();
    service.load_law(special).unwrap();
    assert_eq!(
        value_of(&service, "lazy_replace", "bedrag", bsn()),
        Value::Int(100)
    );
    assert_eq!(
        value_of(&service, "lazy_replace_special", "gevolg", bsn()),
        Value::Int(60)
    );
}

const VOIDED: &str = r#"
$id: lazy_void_two
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Aanspraak naar inkomen, en een vaste toelichting.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: inkomen
            type: number
            source:
              regulation: lazy_belasting
              output: inkomen
              parameters:
                bsn: $bsn
        output:
          - name: aanspraak
            type: number
          - name: toelichting
            type: number
        actions:
          - output: aanspraak
            value: $inkomen
          - output: toelichting
            value: 1
"#;

#[test]
fn another_laws_void_does_not_apply_standalone() {
    let voiding = r#"
$id: lazy_void_two_rule
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Er bestaat geen aanspraak.
    machine_readable:
      overrides:
        - law: lazy_void_two
          article: '1'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
"#;
    let mut service = with_null_income(service(30, None));
    service.load_law(VOIDED).unwrap();
    // An override from another law applies within the execution it starts
    // (RFC-007), so this one only applies when its law is the contextual law;
    // standalone, the entitlement stands and reading it fails on the null.
    service.load_law(voiding).unwrap();
    for requested in [["toelichting", "aanspraak"], ["aanspraak", "toelichting"]] {
        let error = service
            .evaluate_law("lazy_void_two", &requested, bsn(), "2025-01-01")
            .unwrap_err();
        assert!(
            !matches!(error, EngineError::OutputVoided { .. }),
            "another law's void does not apply standalone: {error:?}"
        );
    }
}

#[test]
fn a_void_of_the_same_law_among_several_requested_outputs_is_checked_first() {
    let law = VOIDED.replace(
        "          - output: toelichting\n            value: 1\n",
        "          - output: toelichting\n            value: 1\n  - number: '2'\n    text: Er bestaat geen aanspraak.\n    machine_readable:\n      overrides:\n        - law: lazy_void_two\n          article: '1'\n          output: aanspraak\n          voids: true\n          legal_text_excerpt: bestaat geen aanspraak\n",
    );
    let mut service = with_null_income(service(30, None));
    service.load_law(&law).unwrap();
    for requested in [["toelichting", "aanspraak"], ["aanspraak", "toelichting"]] {
        match service.evaluate_law("lazy_void_two", &requested, bsn(), "2025-01-01") {
            Err(EngineError::OutputVoided { grounds, .. }) => {
                assert_eq!(grounds, "bestaat geen aanspraak");
            }
            other => panic!("expected OutputVoided for {requested:?}, got {other:?}"),
        }
    }
}

#[test]
fn a_void_from_the_contextual_law_is_checked_first() {
    // The voiding law starts the execution and reads the entitlement across
    // laws: its void applies, so the entitlement is not computed (reading the
    // income would fail) and the read gets the ground.
    let contextual = r#"
$id: lazy_void_context
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Voor deze regeling bestaat geen aanspraak.
    machine_readable:
      overrides:
        - law: lazy_void_two
          article: '1'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: aanspraak
            type: number
            source:
              regulation: lazy_void_two
              output: aanspraak
              parameters:
                bsn: $bsn
        output:
          - name: gevolg
            type: number
        actions:
          - output: gevolg
            value: $aanspraak
"#;
    let mut service = with_null_income(service(30, None));
    service.load_law(VOIDED).unwrap();
    service.load_law(contextual).unwrap();
    match service.evaluate_law_output("lazy_void_context", "gevolg", bsn(), "2025-01-01") {
        Err(EngineError::OutputVoided { grounds, .. }) => {
            assert_eq!(grounds, "bestaat geen aanspraak");
        }
        other => panic!("expected OutputVoided, got {other:?}"),
    }
}

//! Taking a decision on a synthetic law: what the cell fills in itself and
//! what it refuses to make up, and which version of the law it applies. The
//! law is made up; it proves what the cell does, not what any law says.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use regelrecht_cel::config::CellConfig;
use regelrecht_cel::{Cell, Error, Input};
use regelrecht_engine::LawExecutionService;
use serde_json::json;

/// A version of `testbesluit` valid from `valid_from`, whose decision is
/// `amount`, dated by `dated_by`.
fn version(valid_from: &str, amount: i64, dated_by: &str) -> String {
    format!(
        r#"
$id: testbesluit
regulatory_layer: WET
publication_date: '2023-01-01'
valid_from: '{valid_from}'
procedure:
  - id: eigen
    applies_to: {{legal_character: BESCHIKKING}}
    stages:
      - name: BESLUIT
        requires:
          - {{name: besluitdatum, type: date}}
          - {{name: kenmerk, type: string}}
articles:
  - number: '1'
    text: Test.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          decision_type: TOEKENNING
          procedure_id: eigen
          extensions:
            chronolex:
              establishes:
                - event: besloten
                  type: decretogram
                  stage: BESLUIT
                  fields: outputs
                  period: {{parameter: jaar, unit: year}}
                  dated_by: {dated_by}
        parameters:
          - {{name: jaar, type: number, required: true}}
        output: [{{name: bedrag, type: number}}]
        actions: [{{output: bedrag, value: {amount}}}]
"#
    )
}

fn service(dated_by: &str) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for v in [
        version("2024-01-01", 100, dated_by),
        version("2025-01-01", 200, dated_by),
    ] {
        service.load_law(&v).unwrap_or_else(|e| panic!("{e}"));
    }
    service
}

fn at(moment: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(moment).unwrap()
}

fn cell(service: &LawExecutionService) -> Result<Cell, Error> {
    let config = CellConfig::from_yaml(
        "id: test\nrecording_actor: actor\nstreams: [s.yaml]\n",
        &["$id: s\nrecording_actor: actor\nchronicle: c\nevents:\n  - name: besloten\n    establishes: testbesluit#1\n"],
        None,
    )?;
    Cell::in_memory(config, Vec::new(), service, "2025-05-01".parse().unwrap())
}

fn given(entries: &[(&str, serde_json::Value)]) -> BTreeMap<String, Input> {
    entries
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                Input {
                    value: v.clone(),
                    provenance: json!({"source": "test"}),
                },
            )
        })
        .collect()
}

/// The law names the date the decision bears; the cell fills that one with
/// the day it decides, and refuses to make up anything else its stage
/// requires.
#[test]
fn the_cell_fills_only_the_date_the_law_names() {
    let service = service("besluitdatum");
    let mut cell = cell(&service).unwrap_or_else(|e| panic!("{e}"));
    let now = at("2025-05-01T10:00:00+02:00");

    let e = cell
        .decide(
            &service,
            "besloten",
            BTreeMap::new(),
            given(&[("jaar", json!(2025))]),
            now,
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    assert!(e.to_string().contains("'kenmerk'"), "{e}");

    let gram = cell
        .decide(
            &service,
            "besloten",
            BTreeMap::new(),
            given(&[("jaar", json!(2025)), ("kenmerk", json!("K-1"))]),
            now,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        gram.inputs["besluitdatum"],
        json!({"value": "2025-05-01", "provenance": {"source": "decision", "stage": "BESLUIT"}})
    );
    assert_eq!(gram.inputs["kenmerk"]["value"], "K-1");
}

/// The period comes from the inputs as given, once: a period given as an
/// extra input selects the law of that period for the decision too.
#[test]
fn a_period_given_by_the_caller_selects_the_law_of_that_period() {
    let service = service("besluitdatum");
    let mut cell = cell(&service).unwrap_or_else(|e| panic!("{e}"));
    let gram = cell
        .decide(
            &service,
            "besloten",
            BTreeMap::new(),
            given(&[("jaar", json!(2024)), ("kenmerk", json!("K-1"))]),
            at("2025-05-01T10:00:00+02:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(gram.regulation_valid_from.as_deref(), Some("2024-01-01"));
    assert_eq!(gram.fields["bedrag"], 100);
    assert_eq!(gram.period.map(|p| p.value), Some(2024));
}

/// `dated_by` names a date the stage requires; anything else is an error in
/// the law, found when the cell starts.
#[test]
fn dated_by_must_name_a_date_the_stage_requires() {
    let e = cell(&service("kenmerk")).err().expect("not a date");
    assert!(e.to_string().contains("not a date"), "{e}");
    let e = cell(&service("onbekend")).err().expect("not required");
    assert!(e.to_string().contains("requires no 'onbekend'"), "{e}");
}

/// A stage that `is` a stage no procedure has is an error in the law: the
/// hooks on it would silently not fire, so the cell does not start.
#[test]
fn a_misspelled_stage_alias_stops_the_cell() {
    let mut service = service("besluitdatum");
    service
        .load_law(
            r#"
$id: algemeen
regulatory_layer: WET
publication_date: '2023-01-01'
valid_from: '2023-01-01'
procedure:
  - id: standaard
    default: true
    applies_to: {legal_character: BESCHIKKING}
    stages:
      - name: BESLUIT
  - id: ander
    applies_to: {legal_character: BESCHIKKING}
    stages:
      - name: STAP
        is: BESLUT
articles:
  - number: '1'
    text: Test.
"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let e = cell(&service).err().expect("refused");
    assert!(matches!(e, Error::Setup(_)), "{e}");
    assert!(e.to_string().contains("BESLUT"), "{e}");
}

/// What a lexostatus gives is read as the field it reads; whether a field
/// is filled in is a boolean, whatever the field holds.
#[test]
fn a_lexostatus_reads_a_filled_field_as_a_boolean() {
    let service = service("besluitdatum");
    let config = CellConfig::from_yaml(
        "id: test\nrecording_actor: actor\nstreams: [s.yaml]\nlexostatuses: l.yaml\n",
        &["$id: s\nrecording_actor: actor\nchronicle: c\nevents:\n  - name: besloten\n    establishes: testbesluit#1\n"],
        Some(
            "cell: test\nlexostatus_definitions:\n  - name: besluit\n    reduction:\n      chronicle: c\n      filter: {event: besloten}\n      pick: latest\n      derivations:\n        bedrag: {field: bedrag}\n        heeft_bedrag: {filled: bedrag, legal_basis: ['testbesluit#1']}\n",
        ),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let cell = Cell::in_memory(config, Vec::new(), &service, "2025-05-01".parse().unwrap())
        .unwrap_or_else(|e| panic!("{e}"));
    let fields = cell
        .lexostatus_fields(&service, "besluit", "2025-05-01".parse().unwrap())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        fields["bedrag"].type_,
        Some(regelrecht_law_model::ParameterType::Number)
    );
    let filled = &fields["heeft_bedrag"];
    assert_eq!(
        filled.type_,
        Some(regelrecht_law_model::ParameterType::Boolean)
    );
    assert_eq!(filled.legal_basis, ["testbesluit#1"]);
    assert_eq!(filled.declared_by, "testbesluit#1");
}

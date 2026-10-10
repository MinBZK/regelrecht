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

/// The period a stream names for a decision on no submission, when the law
/// already names one: an error in the configuration, found when the cell
/// starts.
#[test]
fn a_stream_does_not_name_the_period_the_law_names() {
    let service = service("besluitdatum");
    let config = CellConfig::from_yaml(
        "id: test\nrecording_actor: actor\nstreams: [s.yaml]\n",
        &["$id: s\nrecording_actor: actor\nchronicle: c\nevents:\n  - name: besloten\n    establishes: testbesluit#1\n    period: {parameter: jaar, unit: year}\n    subject: [kenmerk]\n"],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let e = Cell::in_memory(config, Vec::new(), &service, "2025-05-01".parse().unwrap())
        .err()
        .expect("refused");
    assert!(matches!(e, Error::Setup(_)), "{e}");
    assert!(e.to_string().contains("the law names the period"), "{e}");
}

/// A made-up decision on no submission, about a person (`bsn`) over a year
/// (`jaar`, which the stream names), with an optional `opmerking` that says
/// nothing about whom it concerns. Article 2 is the holder's planning: the
/// day it is taken, 15 April of the year after, but no day for 2025 for
/// person `B`. Article 3 gives `dates` outputs of type date, for a
/// `decided_on` that names it.
fn ex_officio_law(dates: usize) -> String {
    let outputs: String = (0..dates)
        .map(|i| format!("          - {{name: dag{i}, type: date}}\n"))
        .collect();
    let actions: String = (0..dates)
        .map(|i| format!("          - {{output: dag{i}, value: '2025-01-01'}}\n"))
        .collect();
    let (outputs, actions) = if dates == 0 {
        (
            "          - {name: getal, type: number}\n".to_string(),
            "          - {output: getal, value: 1}\n".to_string(),
        )
    } else {
        (outputs, actions)
    };
    format!(
        r#"
$id: testaanslag
regulatory_layer: WET
publication_date: '2023-01-01'
valid_from: '2023-01-01'
procedure:
  - id: aanslag
    applies_to: {{legal_character: BESCHIKKING}}
    stages:
      - name: BESLUIT
        requires:
          - {{name: besluitdatum, type: date}}
articles:
  - number: '1'
    text: Test.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          decision_type: TOEKENNING
          procedure_id: aanslag
          extensions:
            chronolex:
              establishes:
                - event: aanslag
                  type: decretogram
                  stage: BESLUIT
                  fields: outputs
                  dated_by: besluitdatum
        parameters:
          - {{name: bsn, type: string, required: true}}
          - {{name: jaar, type: number, required: true}}
          - {{name: opmerking, type: string, required: false}}
        output: [{{name: bedrag, type: number}}]
        actions: [{{output: bedrag, value: 1}}]
  - number: '2'
    text: Test.
    machine_readable:
      execution:
        parameters:
          - {{name: bsn, type: string, required: true}}
          - {{name: jaar, type: number, required: true}}
        output:
          - {{name: dag, type: date, nullable: true}}
        actions:
          - output: dag
            value:
              operation: IF
              cases:
                - when:
                    operation: AND
                    conditions:
                      - {{operation: EQUALS, subject: $bsn, value: B}}
                      - {{operation: EQUALS, subject: $jaar, value: 2025}}
                  then: null
              default:
                operation: DATE
                year:
                  operation: ADD
                  values: [$jaar, 1]
                month: 4
                day: 15
  - number: '3'
    text: Test.
    machine_readable:
      execution:
        parameters:
          - {{name: bsn, type: string, required: true}}
          - {{name: jaar, type: number, required: true}}
        output:
{outputs}        actions:
{actions}"#
    )
}

/// A cell that takes `aanslag` ex officio on the day `decided_on` gives,
/// with `extra` added to its event in the stream.
fn ex_officio_cell(
    service: &LawExecutionService,
    decided_on: &str,
    extra: &str,
) -> Result<Cell, Error> {
    let stream = format!(
        "$id: s\nrecording_actor: actor\nchronicle: c\nevents:\n  - name: aanslag\n    establishes: testaanslag#1\n    decided_on: {decided_on}\n    period: {{parameter: jaar, unit: year}}\n{extra}"
    );
    let config = CellConfig::from_yaml(
        "id: test\nrecording_actor: actor\nstreams: [s.yaml]\n",
        &[stream.as_str()],
    )?;
    Cell::in_memory(config, Vec::new(), service, "2025-01-01".parse().unwrap())
}

fn ex_officio_service(dates: usize) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service
        .load_law(&ex_officio_law(dates))
        .unwrap_or_else(|e| panic!("{e}"));
    service
}

fn person(bsn: &str) -> BTreeMap<String, Input> {
    given(&[("bsn", json!(bsn))])
}

/// A decision on no submission names whom it concerns (`subject`): without
/// it the cell cannot tell a second decision about one person from a first
/// about another, and does not start.
#[test]
fn an_ex_officio_decision_names_its_subject() {
    let service = ex_officio_service(1);
    let e = ex_officio_cell(&service, "testaanslag#2", "")
        .err()
        .expect("refused");
    assert!(e.to_string().contains("`subject`"), "{e}");
    let e = ex_officio_cell(&service, "testaanslag#2", "    subject: [onbekend]\n")
        .err()
        .expect("refused");
    assert!(e.to_string().contains("'onbekend'"), "{e}");
    let cell = ex_officio_cell(&service, "testaanslag#2", "    subject: [bsn]\n")
        .unwrap_or_else(|e| panic!("{e}"));
    // Asked about something else than its subject: refused.
    let e = cell
        .due_ex_officio(
            &service,
            "aanslag",
            &given(&[("bsn", json!("A")), ("opmerking", json!("x"))]),
            at("2026-05-01T10:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
}

/// Once per year per subject: what else is given does not make a second
/// decision about the same person over the same year another one, and a
/// decision about another person is not the same one.
#[test]
fn an_ex_officio_decision_is_taken_once_per_period_per_subject() {
    let service = ex_officio_service(1);
    let mut cell = ex_officio_cell(&service, "testaanslag#2", "    subject: [bsn]\n")
        .unwrap_or_else(|e| panic!("{e}"));
    let now = at("2026-05-01T10:00:00+02:00");
    let with = |bsn: &str, extra: &[(&str, serde_json::Value)]| {
        let mut inputs = given(extra);
        inputs.extend(person(bsn));
        inputs.extend(given(&[("jaar", json!(2024))]));
        inputs
    };
    let first = cell
        .decide(&service, "aanslag", BTreeMap::new(), with("A", &[]), now)
        .unwrap_or_else(|e| panic!("{e}"));
    let e = cell
        .decide(
            &service,
            "aanslag",
            BTreeMap::new(),
            with("A", &[("opmerking", json!("nogmaals"))]),
            now,
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    assert!(e.to_string().contains(&first.id), "{e}");
    cell.decide(&service, "aanslag", BTreeMap::new(), with("B", &[]), now)
        .unwrap_or_else(|e| panic!("another person: {e}"));
}

/// The holder's planning gives no day: the decision is not due, and the
/// cell does not take it; before its day neither.
#[test]
fn an_ex_officio_decision_without_its_day_is_refused() {
    let service = ex_officio_service(1);
    let mut cell = ex_officio_cell(&service, "testaanslag#2", "    subject: [bsn]\n")
        .unwrap_or_else(|e| panic!("{e}"));
    let mut inputs = person("B");
    inputs.extend(given(&[("jaar", json!(2025))]));
    let e = cell
        .decide(
            &service,
            "aanslag",
            BTreeMap::new(),
            inputs,
            at("2026-05-01T10:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    assert!(e.to_string().contains("not due"), "{e}");
    let mut inputs = person("A");
    inputs.extend(given(&[("jaar", json!(2025))]));
    let e = cell
        .decide(
            &service,
            "aanslag",
            BTreeMap::new(),
            inputs,
            at("2026-04-14T10:00:00+02:00"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("2026-04-15"), "{e}");
}

/// A year without a decision is asked again after a later year was decided:
/// every undecided year from the first decision on, through the year of
/// now. With `first_period`, from that year on, also before the first
/// decision.
#[test]
fn a_skipped_year_is_asked_again() {
    let service = ex_officio_service(1);
    let mut cell = ex_officio_cell(&service, "testaanslag#2", "    subject: [bsn]\n")
        .unwrap_or_else(|e| panic!("{e}"));
    let decide = |cell: &mut Cell, year: i32, now: &str| {
        let mut inputs = person("A");
        inputs.extend(given(&[("jaar", json!(year))]));
        cell.decide(&service, "aanslag", BTreeMap::new(), inputs, at(now))
            .unwrap_or_else(|e| panic!("{year}: {e}"))
    };
    decide(&mut cell, 2024, "2025-05-01T10:00:00+02:00");
    // 2025 is passed over; 2026 is decided on its day.
    decide(&mut cell, 2026, "2027-04-15T10:00:00+02:00");
    let due = cell
        .due_ex_officio(
            &service,
            "aanslag",
            &person("A"),
            at("2027-04-16T10:00:00+02:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(due.period.map(|p| p.value), Some(2025));
    assert_eq!(due.day, Some("2026-04-15".parse().unwrap()));

    // Before the first decision, from the year before the one of now; with
    // `first_period`, from that year.
    let fresh = ex_officio_cell(&service, "testaanslag#2", "    subject: [bsn]\n")
        .unwrap_or_else(|e| panic!("{e}"));
    let due = fresh
        .due_ex_officio(
            &service,
            "aanslag",
            &person("A"),
            at("2027-04-16T10:00:00+02:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(due.period.map(|p| p.value), Some(2026));
    let from = ex_officio_cell(
        &service,
        "testaanslag#2",
        "    subject: [bsn]\n    first_period: 2024\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let due = from
        .due_ex_officio(
            &service,
            "aanslag",
            &person("A"),
            at("2027-04-16T10:00:00+02:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(due.period.map(|p| p.value), Some(2024));
}

/// A `first_period` after the period of now asks no period at all: an
/// error, not an empty answer. So is a first period so far back that the
/// cell would ask more than `MAX_EX_OFFICIO_PERIODS` periods.
#[test]
fn a_first_period_out_of_range_is_an_error() {
    let service = ex_officio_service(1);
    let now = at("2027-04-16T10:00:00+02:00");
    let cell = ex_officio_cell(
        &service,
        "testaanslag#2",
        "    subject: [bsn]\n    first_period: 1900\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let e = cell
        .due_ex_officio(&service, "aanslag", &person("A"), now)
        .unwrap_err();
    assert!(matches!(e, Error::Setup(_)), "{e}");
    assert!(e.to_string().contains("at most"), "{e}");
    // A first period after now is not yet due: nothing to ask, no error.
    let cell = ex_officio_cell(
        &service,
        "testaanslag#2",
        "    subject: [bsn]\n    first_period: 2028\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let due = cell
        .due_ex_officio(&service, "aanslag", &person("A"), now)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((due.period, due.day), (None, None));
    // The period of now itself is the one period asked.
    let cell = ex_officio_cell(
        &service,
        "testaanslag#2",
        "    subject: [bsn]\n    first_period: 2027\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let due = cell
        .due_ex_officio(&service, "aanslag", &person("A"), now)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(due.period.map(|p| p.value), Some(2027));
}

/// The article `decided_on` names gives one day: its one output of type
/// date. None or two is an error in the configuration.
#[test]
fn decided_on_gives_one_date() {
    for dates in [0, 2] {
        let service = ex_officio_service(dates);
        let mut cell = ex_officio_cell(&service, "testaanslag#3", "    subject: [bsn]\n")
            .unwrap_or_else(|e| panic!("{e}"));
        let mut inputs = person("A");
        inputs.extend(given(&[("jaar", json!(2024))]));
        let e = cell
            .decide(
                &service,
                "aanslag",
                BTreeMap::new(),
                inputs,
                at("2025-05-01T10:00:00+02:00"),
            )
            .unwrap_err();
        assert!(matches!(e, Error::Setup(_)), "{dates}: {e}");
        assert!(
            e.to_string()
                .contains(&format!("{dates} outputs of type date")),
            "{dates}: {e}"
        );
        let e = cell
            .due_ex_officio(
                &service,
                "aanslag",
                &person("A"),
                at("2025-05-01T10:00:00+02:00"),
            )
            .unwrap_err();
        assert!(matches!(e, Error::Setup(_)), "{dates}: {e}");
    }
}

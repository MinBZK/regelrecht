//! Someone applies for zorgtoeslag (Awir art. 15), the Belastingdienst/Toeslagen
//! grants it (Zorgtoeslagwet art. 2), and the cell of Toeslagen records both
//! as grams in its chronicle. The shape of each gram comes from the law: the
//! application from executing Awir 15 (Awb 4:2 and 4:13 hook onto it), the
//! decision from the outputs of Zorgtoeslagwet art. 2.
//!
//! The regulation is the corpus itself; the cell configuration is a fixture.
//! The citizen is the fictional test person of the engine's zorgtoeslag trace.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use regelrecht_cel::cell::load_regulations;
use regelrecht_cel::config::{CellConfig, Read};
use regelrecht_cel::extension::PeriodUnit;
use regelrecht_cel::register;
use regelrecht_cel::{Cell, Error, Gram, Input, Period};
use regelrecht_engine::{LawExecutionService, Value};
use serde_json::{json, Map};

const BSN: &str = "999993653";
const AWIR: &str = "algemene_wet_inkomensafhankelijke_regelingen";
const ZORGTOESLAG: &str = "wet_op_de_zorgtoeslag";
/// The toetsingsinkomen the citizen expects (Awir 16): 30.000 euro, far
/// above what the registers will know of the year (795,47 euro).
const ESTIMATE: i64 = 3_000_000;
/// The account the citizen gives in the application (fictitious format).
const ACCOUNT: &str = "NL00TEST0123456789";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cell_yaml() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/toeslagen/cell.yaml")
}

/// The corpus, with the registers of the fixture cell bound: the policy of
/// Toeslagen that reads its chronicle back.
fn regulations() -> LawExecutionService {
    let mut service =
        load_regulations(&root().join("corpus/regulation")).unwrap_or_else(|e| panic!("{e}"));
    let config = CellConfig::load(&cell_yaml()).unwrap_or_else(|e| panic!("{e}"));
    register::bind(&mut service, &config).unwrap_or_else(|e| panic!("{e}"));
    service
}

/// The values the lexostatus `name` of `cell` gives at `as_of`.
fn read(
    cell: &Cell,
    service: &LawExecutionService,
    name: &str,
    inputs: &serde_json::Map<String, serde_json::Value>,
    as_of: DateTime<FixedOffset>,
) -> serde_json::Map<String, serde_json::Value> {
    let reading = cell.read_lexostatus(service, name, inputs, as_of);
    let reading = reading.unwrap_or_else(|e| panic!("{e}"));
    reading
        .values
        .into_iter()
        .map(|(k, i)| (k, i.value))
        .collect()
}

fn at(moment: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(moment).unwrap()
}

/// The cell of Toeslagen over its chronicles in `data`, on the day of `now`.
fn cell(service: &LawExecutionService, data: &Path, now: DateTime<FixedOffset>) -> Cell {
    Cell::open(&cell_yaml(), service, data, now.date_naive()).unwrap_or_else(|e| panic!("{e}"))
}

fn year(value: i32) -> Period {
    Period {
        unit: PeriodUnit::Year,
        value,
    }
}

fn object(v: serde_json::Value) -> Map<String, serde_json::Value> {
    v.as_object().unwrap().clone()
}

/// The application as the citizen fills it in: what Awir 15 and Awb 4:2 lid
/// 1 ask of them.
fn application() -> Map<String, serde_json::Value> {
    application_for(2025)
}

/// The same application, for the berekeningsjaar `year`.
fn application_for(year: i32) -> Map<String, serde_json::Value> {
    object(json!({
        "bsn": BSN,
        "aangevraagd_berekeningsjaar": year,
        "naam_aanvrager": "J. Voorbeeld",
        "adres_aanvrager": "Voorbeeldstraat 1, 2511 AA Den Haag",
        "dagtekening": "2025-03-03",
        "ondertekening": "J. Voorbeeld",
        "vermoedelijk_toetsingsinkomen": ESTIMATE,
        "rekeningnummer": ACCOUNT,
    }))
}

fn record(entries: Vec<(&str, Value)>) -> BTreeMap<String, Value> {
    entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}

/// What the registers know of the citizen, as the engine's zorgtoeslag trace
/// test registers it.
fn register_sources(service: &mut LawExecutionService) {
    register_sources_with_income(service, 79547);
}

/// The same registers, with `income` as the citizen's wages over the year.
fn register_sources_with_income(service: &mut LawExecutionService, income: i64) {
    let bsn = || ("bsn", Value::String(BSN.to_string()));
    let sources = [
        (
            "personal_data",
            record(vec![
                bsn(),
                ("geboortedatum", Value::String("2005-01-01".into())),
            ]),
        ),
        (
            "relationship_data",
            record(vec![
                bsn(),
                ("partnerschap_type", Value::String("GEEN".into())),
                ("partner_bsn", Value::Null),
            ]),
        ),
        (
            "insurance",
            record(vec![
                bsn(),
                ("polis_status", Value::String("ACTIEF".into())),
                ("verdragsinschrijving", Value::Bool(false)),
            ]),
        ),
        (
            "box1",
            record(vec![
                bsn(),
                ("loon_uit_dienstbetrekking", Value::Int(income)),
                ("uitkeringen_en_pensioenen", Value::Int(0)),
                ("winst_uit_onderneming", Value::Int(0)),
                ("resultaat_overige_werkzaamheden", Value::Int(0)),
                ("eigen_woning", Value::Int(0)),
            ]),
        ),
        (
            "inkomensgegevens",
            record(vec![
                bsn(),
                (
                    "aanslag_of_navorderingsaanslag_vastgesteld",
                    Value::Bool(true),
                ),
                ("belastbaar_loon", Value::Int(income)),
                ("niet_in_nederland_belastbaar_inkomen", Value::Int(0)),
            ]),
        ),
        (
            "box2",
            record(vec![
                bsn(),
                ("reguliere_voordelen", Value::Int(0)),
                ("vervreemdingsvoordelen", Value::Int(0)),
            ]),
        ),
        (
            "box3",
            record(vec![
                bsn(),
                ("spaargeld", Value::Int(0)),
                ("beleggingen", Value::Int(0)),
                ("onroerend_goed", Value::Int(0)),
                ("schulden", Value::Int(0)),
            ]),
        ),
        (
            "detenties",
            record(vec![
                bsn(),
                ("detentiestatus", Value::Null),
                ("inrichting_type", Value::Null),
                ("zorgtype", Value::Null),
                ("juridische_grondslag", Value::Null),
            ]),
        ),
    ];
    for (name, row) in sources {
        service
            .register_dict_source(name, "bsn", vec![row])
            .unwrap();
    }
}

#[test]
fn a_citizen_applies_and_toeslagen_records_the_application() {
    let data = tempfile::tempdir().unwrap();
    let service = regulations();
    let received = at("2025-03-04T10:15:00+01:00");
    let mut cell = cell(&service, data.path(), received);

    let gram = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(gram.type_, "submission");
    assert_eq!(gram.subtype.as_deref(), Some("aanvraag"));
    // The first stage of the procedure of the beschikking taken on it.
    assert_eq!(gram.stage.as_deref(), Some("AANVRAAG"));
    assert_eq!(gram.recording_actor, "belastingdienst_toeslagen");
    // Awb 4:13 lid 1: the receipt is the moment that counts.
    assert_eq!(gram.effective_at, "2025-03-04T10:15:00+01:00");
    assert_eq!(
        gram.effective_at_legal_basis,
        ["algemene_wet_bestuursrecht#4:13 lid 1"]
    );
    assert_eq!(gram.establishes, format!("{AWIR}#15"));
    assert_eq!(gram.legal_basis[0], format!("{AWIR}#15"));
    // The fields are what the executed law asks of the applicant: Awir 15
    // and Awb 4:2 lid 1. The decision requested (4:2 lid 1 onder c) is the
    // one Zorgtoeslagwet art. 2 takes on it: the cell fills it in.
    let mut expected = application();
    expected.insert(
        "gevraagde_beschikking".into(),
        json!(format!("{ZORGTOESLAG}#2")),
    );
    assert_eq!(gram.fields, expected);

    let chronicle = data.path().join("toeslagen/toeslagen.jsonl");
    assert_eq!(
        std::fs::read_to_string(&chronicle).unwrap().lines().count(),
        1
    );

    // What the law does not ask is refused, and so is what the cell fills
    // in itself.
    for (field, value) in [
        ("schoenmaat", json!(44)),
        ("gevraagde_beschikking", json!("iets anders")),
        ("datum_ontvangst", json!("2025-01-01")),
        ("aangevraagd_berekeningsjaar", json!("vorig jaar")),
    ] {
        let mut wrong = application();
        wrong.insert(field.into(), value);
        let e = cell
            .record_submission(&service, "aanvraag_ontvangen", &wrong, received)
            .unwrap_err();
        assert!(matches!(e, Error::Refused(_)), "{field}: {e}");
    }
    // A field left out is still an application (Awb 4:5 asks to complete it).
    let mut partial = application();
    partial.remove("adres_aanvrager");
    let gram = cell
        .record_submission(&service, "aanvraag_ontvangen", &partial, received)
        .unwrap();
    assert!(!gram.fields.contains_key("adres_aanvrager"));
}

#[test]
fn toeslagen_decides_on_the_application_and_records_the_decision() {
    let data = tempfile::tempdir().unwrap();
    let service = regulations();
    let received = at("2025-03-04T10:15:00+01:00");
    let mut cell = cell(&service, data.path(), received);
    let application = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();

    // 1. The cell reads the application back from its chronicle.
    let root = object(json!({"root": application.id}));
    let read = read(&cell, &service, "aanvraag", &root, received);
    assert_eq!(
        serde_json::Value::Object(read.clone()),
        json!({
            "bsn": BSN,
            "aangevraagd_berekeningsjaar": 2025,
            "datum_ontvangst": "2025-03-04",
        })
    );

    // 2. Awir 15 lid 1 on what the cell read back: in time.
    let timely = service
        .evaluate_law_output(
            AWIR,
            "aanvraag_binnen_termijn",
            read.iter()
                .map(|(k, v)| (k.clone(), Value::from(v)))
                .collect(),
            "2025-03-04",
        )
        .unwrap();
    assert_eq!(
        timely.outputs.get("aanvraag_binnen_termijn"),
        Some(&Value::Bool(true))
    );

    // 3. Six weeks later Toeslagen decides: Zorgtoeslagwet art. 2 on the
    //    application, with what the registers know of the citizen.
    let decided = at("2025-04-15T09:00:00+02:00");
    let mut service = service;
    register_sources(&mut service);
    // The decision reads its parameters from the application it is taken
    // on and the policy article its event `reads`, kept to what its stage asks (the bsn; not the day of receipt,
    // and at the toekenning not the estimate, which only Awir 16 asks at the
    // voorschot). The berekeningsjaar it concerns is the cell's to give: the
    // first decision of the event concerns the year the application asks
    // for. `decision_inputs` shows what `decide` will read.
    let inputs = cell
        .decision_inputs(&service, "zorgtoeslag_toegekend", &application.id, decided)
        .unwrap_or_else(|e| panic!("{e}"));
    let from_case = |name: &str| {
        (
            name.to_string(),
            Input {
                value: read[name].clone(),
                provenance: json!({
                    "source": "lexostatus",
                    "lexostatus": "aanvraag",
                    "article": format!("{AWIR}#15"),
                    "gram": application.id,
                }),
            },
        )
    };
    // Awir 24 asks what was paid on the voorschot: nothing yet.
    let paid = (
        "uitbetaalde_voorschotten".to_string(),
        Input {
            value: json!(0),
            provenance: json!({
                "source": "lexostatus",
                "lexostatus": "uitbetaald",
                "register": "fictief_beleid_kroniek_toeslagen#kroniek",
                "article": "fictief_beleid_kroniek_toeslagen#3a",
            }),
        },
    );
    let period = (
        "berekeningsjaar".to_string(),
        Input {
            value: json!(2025),
            provenance: json!({"source": "period"}),
        },
    );
    assert_eq!(inputs, BTreeMap::from([from_case("bsn"), period, paid]));
    let refers_to = BTreeMap::from([("on_application".to_string(), application.id.clone())]);

    // What the cell reads from the case cannot be given as well.
    let e = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            refers_to.clone(),
            BTreeMap::from([(
                "bsn".to_string(),
                Input {
                    value: json!("999990019"),
                    provenance: json!({"source": "caller"}),
                },
            )]),
            decided,
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");

    // The cell reads the application `refers_to` names itself.
    let decision = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            refers_to,
            BTreeMap::new(),
            decided,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(decision.type_, "decretogram");
    // The stage of the procedure of the Awir it is taken at (Zorgtoeslagwet
    // art. 2: `procedure_id: tegemoetkoming`).
    assert_eq!(decision.stage.as_deref(), Some("TOEKENNING"));
    assert_eq!(decision.legal_character.as_deref(), Some("BESCHIKKING"));
    assert_eq!(decision.decision_type.as_deref(), Some("TOEKENNING"));
    assert_eq!(decision.regulation.as_deref(), Some(ZORGTOESLAG));
    assert_eq!(
        decision.regulation_valid_from.as_deref(),
        Some("2025-01-01")
    );
    assert_eq!(decision.refers_to["on_application"], application.id);
    assert_eq!(decision.establishes, format!("{ZORGTOESLAG}#2"));
    // What the case gave, and the dagtekening the stage requires: the day
    // the decision is taken.
    let mut expected_inputs: BTreeMap<String, serde_json::Value> = inputs
        .into_iter()
        .map(|(k, i)| (k, serde_json::to_value(i).unwrap()))
        .collect();
    expected_inputs.insert(
        "dagtekening_toekenning".into(),
        json!({
            "value": "2025-04-15",
            "provenance": {"source": "decision", "stage": "TOEKENNING"},
        }),
    );
    assert_eq!(decision.inputs, expected_inputs);
    assert_eq!(decision.inputs["bsn"]["provenance"]["source"], "lexostatus");
    assert_eq!(decision.period, Some(year(2025)));

    // The fields are the outputs of art. 2, and they are what the engine
    // computes for this citizen without any cell: the existing calculation.
    let mut direct = regulations();
    register_sources(&mut direct);
    let expected = direct
        .evaluate_law_output(
            ZORGTOESLAG,
            "hoogte_zorgtoeslag",
            BTreeMap::from([("bsn".to_string(), Value::String(BSN.into()))]),
            "2025-04-15",
        )
        .unwrap();
    assert_eq!(
        decision.fields["hoogte_zorgtoeslag"],
        serde_json::to_value(&expected.outputs["hoogte_zorgtoeslag"]).unwrap()
    );
    assert_eq!(decision.fields["hoogte_zorgtoeslag"], json!(157731));
    assert_eq!(decision.fields["tegemoetkoming"], json!(157731));
    // The Awb hooks on every besluit, and TOEKENNING is one (`is: BESLUIT`).
    assert_eq!(decision.fields["bezwaartermijn_weken"], json!(6));
    assert_eq!(decision.fields["motivering_vereist"], json!(true));
    // Awir 16 does not fire at the toekenning: no voorschot, no estimate.
    assert!(!decision.fields.contains_key("voorschotbedrag"));
    assert!(!decision.fields.contains_key("toetsingsinkomen"));

    let chronicle = data.path().join("toeslagen/toeslagen.jsonl");
    assert_eq!(
        std::fs::read_to_string(&chronicle).unwrap().lines().count(),
        2
    );

    // A decision without its application, or on another decision, is refused.
    let e = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            BTreeMap::new(),
            BTreeMap::new(),
            decided,
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    let on_a_decision = BTreeMap::from([("on_application".to_string(), decision.id.clone())]);
    let e = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            on_a_decision,
            BTreeMap::new(),
            decided,
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");

    // The chronicle survives the cell: a new cell reads it back.
    drop(cell);
    let cell = self::cell(&service, data.path(), decided);
    let read_again = self::read(&cell, &service, "aanvraag", &root, decided);
    assert_eq!(read_again, read);
}

/// An application for 2025 decided in 2026 (Awir 15 lid 1 allows it until
/// 1 September 2026) is computed with the law of 2025: the decision concerns
/// the berekeningsjaar, not the day it is taken.
#[test]
fn a_decision_applies_the_law_of_the_berekeningsjaar() {
    // The corpus has no standaardpremie for 2026 yet; a fictional one makes
    // the law of 2026 differ from that of 2025.
    let regulations = || {
        let mut service = regulations();
        let premium_2025 = std::fs::read_to_string(root().join(
            "corpus/regulation/nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml",
        ))
        .unwrap();
        assert!(premium_2025.contains("value: 211200"));
        let premium_2026 = premium_2025
            .replace("valid_from: '2025-01-01'", "valid_from: '2026-01-01'")
            .replace("value: 211200", "value: 231200");
        service.load_law(&premium_2026).unwrap();
        service
    };
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let received = at("2026-03-02T10:15:00+01:00");
    let mut cell = cell(&service, data.path(), received);
    let application = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();
    assert_eq!(application.fields["aangevraagd_berekeningsjaar"], 2025);

    register_sources(&mut service);
    let decided = at("2026-04-13T09:00:00+02:00");
    let decision = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            BTreeMap::from([("on_application".to_string(), application.id.clone())]),
            BTreeMap::new(),
            decided,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    // Taken and recorded in 2026, on the law of 2025.
    assert_eq!(decision.effective_at, "2026-04-13T09:00:00+02:00");
    assert_eq!(decision.recorded_at, "2026-04-13T09:00:00+02:00");
    assert_eq!(decision.period, Some(year(2025)));
    assert_eq!(
        decision.regulation_valid_from.as_deref(),
        Some("2025-01-01")
    );
    // The year took part in the decision: the year the application asks
    // for, given by the cell.
    assert_eq!(
        decision.inputs["berekeningsjaar"],
        json!({"value": 2025, "provenance": {"source": "period"}})
    );

    let direct = |day: &str| {
        let mut direct = regulations();
        register_sources(&mut direct);
        let result = direct
            .evaluate_law_output(
                ZORGTOESLAG,
                "hoogte_zorgtoeslag",
                BTreeMap::from([("bsn".to_string(), Value::String(BSN.into()))]),
                day,
            )
            .unwrap();
        serde_json::to_value(&result.outputs["hoogte_zorgtoeslag"]).unwrap()
    };
    assert_eq!(decision.fields["hoogte_zorgtoeslag"], direct("2025-01-01"));
    assert_ne!(decision.fields["hoogte_zorgtoeslag"], direct("2026-04-13"));
}

/// A decision on a period the case does not give is refused, not computed
/// with the law of the day it is taken.
#[test]
fn a_decision_without_its_period_is_refused() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let received = at("2025-03-04T10:15:00+01:00");
    let mut cell = cell(&service, data.path(), received);
    let mut submitted = application();
    submitted.remove("aangevraagd_berekeningsjaar");
    let application = cell
        .record_submission(&service, "aanvraag_ontvangen", &submitted, received)
        .unwrap();
    register_sources(&mut service);
    let e = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            BTreeMap::from([("on_application".to_string(), application.id)]),
            BTreeMap::new(),
            at("2025-04-15T09:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    assert!(e.to_string().contains("aangevraagd_berekeningsjaar"), "{e}");
}

/// A `reads` entry names a policy of the holder; a bare name (the
/// lexostatus configuration the cell no longer has) is refused when the
/// configuration is read.
#[test]
fn an_event_reading_a_bare_name_is_refused() {
    let fixture = cell_yaml().parent().unwrap().to_path_buf();
    let read = |f: &str| std::fs::read_to_string(fixture.join(f)).unwrap();
    let decisions = read("streams/zorgtoeslag_besluiten.yaml").replacen(
        "    reads:\n",
        "    reads:\n      - aanvraag\n",
        1,
    );
    assert_ne!(decisions, read("streams/zorgtoeslag_besluiten.yaml"));
    let e = CellConfig::from_yaml(
        &read("cell.yaml"),
        &[&read("streams/zorgtoeslag_aanvragen.yaml"), &decisions],
    )
    .unwrap_err();
    assert!(matches!(e, Error::Setup(_)), "{e}");
}

/// Awir 15 lid 1: an application for 2025 can be made until 1 September
/// 2026.
#[test]
fn an_application_is_in_time_until_the_first_of_september() {
    let service = regulations();
    for (received, in_time) in [("2026-08-31", true), ("2026-09-01", false)] {
        let result = service
            .evaluate_law_output(
                AWIR,
                "aanvraag_binnen_termijn",
                BTreeMap::from([
                    ("bsn".to_string(), Value::String(BSN.into())),
                    ("aangevraagd_berekeningsjaar".to_string(), Value::Int(2025)),
                    (
                        "datum_ontvangst".to_string(),
                        Value::String(received.into()),
                    ),
                ]),
                "2025-03-04",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("aanvraag_binnen_termijn"),
            Some(&Value::Bool(in_time)),
            "received {received}"
        );
    }
}

/// In the browser the chronicle lives in memory: what the page kept of an
/// earlier session comes back in, and the cell reads it as its own.
#[test]
fn a_chronicle_in_memory_reads_back_what_it_was_given() {
    let service = regulations();
    let config = CellConfig::load(&cell_yaml()).unwrap();
    let received = at("2025-03-04T10:15:00+01:00");
    let mut cell =
        Cell::in_memory(config.clone(), Vec::new(), &service, received.date_naive()).unwrap();
    let gram = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();
    let kept: Vec<Gram> = cell.grams().cloned().collect();
    assert_eq!(kept, std::slice::from_ref(&gram));

    let again = Cell::in_memory(
        config.clone(),
        kept.clone(),
        &service,
        received.date_naive(),
    )
    .unwrap();
    let root = object(json!({"root": gram.id}));
    let read = read(&again, &service, "aanvraag", &root, received);
    assert_eq!(read["bsn"], BSN);

    // A gram given twice is refused, not silently kept twice.
    let twice = [kept.clone(), kept].concat();
    assert!(Cell::in_memory(config, twice, &service, received.date_naive()).is_err());
}

/// The hoogte Zorgtoeslagwet art. 2 gives on `day` for the citizen whose
/// registered income is `income`, without any cell.
fn hoogte_on(income: i64, day: &str) -> serde_json::Value {
    let mut direct = regulations();
    register_sources_with_income(&mut direct, income);
    let result = direct
        .evaluate_law_output(
            ZORGTOESLAG,
            "hoogte_zorgtoeslag",
            BTreeMap::from([("bsn".to_string(), Value::String(BSN.into()))]),
            day,
        )
        .unwrap();
    serde_json::to_value(&result.outputs["hoogte_zorgtoeslag"]).unwrap()
}

/// Awir 16: the voorschot is computed on the income the citizen expects, at
/// the stage VOORSCHOT; the toekenning on the income the registers know, at
/// the stage TOEKENNING, and not on the estimate.
#[test]
fn the_voorschot_rests_on_the_estimate_and_the_toekenning_on_the_income() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let received = at("2025-03-04T10:15:00+01:00");
    let mut cell = cell(&service, data.path(), received);
    let application = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();
    // Awir 16 hooks on the application of Awir 15: it asks the estimate.
    assert_eq!(
        application.fields["vermoedelijk_toetsingsinkomen"],
        ESTIMATE
    );
    register_sources(&mut service);
    let refers_to = || BTreeMap::from([("on_application".to_string(), application.id.clone())]);

    // The voorschot asks the estimate; the toekenning does not.
    let decided = at("2025-04-15T09:00:00+02:00");
    let voorschot_inputs = cell
        .decision_inputs(&service, "voorschot_verleend", &application.id, decided)
        .unwrap();
    assert_eq!(
        voorschot_inputs.keys().collect::<Vec<_>>(),
        [
            // Awir 15 lid 5 via Awir 16: the application holds for the
            // berekeningsjaar of the voorschot, which the cell gives.
            "aangevraagd_berekeningsjaar",
            "berekeningsjaar",
            "bsn",
            // Awir 16 lid 1: a voorschot only on an application received
            // before 1 April of the year after the berekeningsjaar.
            "datum_ontvangst",
            "vermoedelijk_toetsingsinkomen"
        ]
    );
    let toekenning_inputs = cell
        .decision_inputs(&service, "zorgtoeslag_toegekend", &application.id, decided)
        .unwrap();
    assert!(!toekenning_inputs.contains_key("vermoedelijk_toetsingsinkomen"));

    let voorschot = cell
        .decide(
            &service,
            "voorschot_verleend",
            refers_to(),
            BTreeMap::new(),
            decided,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(voorschot.stage.as_deref(), Some("VOORSCHOT"));
    assert_eq!(voorschot.period, Some(year(2025)));
    assert_eq!(
        voorschot.regulation_valid_from.as_deref(),
        Some("2025-01-01")
    );
    assert_eq!(
        voorschot.inputs["dagtekening_voorschot"],
        json!({
            "value": "2025-04-15",
            "provenance": {"source": "decision", "stage": "VOORSCHOT"},
        })
    );
    assert_eq!(
        voorschot.inputs["vermoedelijk_toetsingsinkomen"]["value"],
        ESTIMATE
    );
    // Computed on the estimate: what the law gives a citizen whose income is
    // the estimate.
    assert_eq!(voorschot.fields["toetsingsinkomen"], ESTIMATE);
    let on_estimate = hoogte_on(ESTIMATE, "2025-01-01");
    assert_eq!(voorschot.fields["hoogte_zorgtoeslag"], on_estimate);
    // Awir 14 lid 4: the voorschot in whole euros, rounded half up.
    let hoogte = on_estimate.as_i64().unwrap();
    assert_ne!(hoogte % 100, 0, "pick an estimate whose hoogte has cents");
    assert_eq!(
        voorschot.fields["voorschotbedrag"],
        json!((hoogte + 50) / 100 * 100)
    );
    // The Awb on a besluit: VOORSCHOT is one.
    assert_eq!(voorschot.fields["bezwaartermijn_weken"], json!(6));
    assert_eq!(voorschot.fields["motivering_vereist"], json!(true));
    for basis in [
        "algemene_wet_inkomensafhankelijke_regelingen#16",
        "algemene_wet_bestuursrecht#6:7",
    ] {
        assert!(voorschot.legal_basis.iter().any(|b| b == basis), "{basis}");
    }

    // A year later the toekenning, still on the law of 2025, on the income
    // the registers know: not the estimate.
    let toegekend = at("2026-06-01T09:00:00+02:00");
    let toekenning = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            refers_to(),
            BTreeMap::new(),
            toegekend,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(toekenning.stage.as_deref(), Some("TOEKENNING"));
    assert_eq!(
        toekenning.regulation_valid_from.as_deref(),
        Some("2025-01-01")
    );
    assert_eq!(toekenning.fields["hoogte_zorgtoeslag"], json!(157731));
    assert_ne!(toekenning.fields["hoogte_zorgtoeslag"], on_estimate);
    assert!(!toekenning
        .inputs
        .contains_key("vermoedelijk_toetsingsinkomen"));
    assert_eq!(toekenning.fields["bezwaartermijn_weken"], json!(6));
    assert_eq!(toekenning.fields["motivering_vereist"], json!(true));
}

/// A decision reads what it asks of the application it is taken on from
/// that application, as the law describes it, and the rest from the articles
/// of the policy of the holder it reads; what a policy gives, it reads from
/// the policy. A parameter two reads give is ambiguous, and refused.
#[test]
fn a_decision_reads_the_application_and_policy_articles() {
    let fixture = cell_yaml().parent().unwrap().to_path_buf();
    let read = |f: &str| std::fs::read_to_string(fixture.join(f)).unwrap();
    let decisions = read("streams/zorgtoeslag_besluiten.yaml");
    // One read is a list of one.
    let policy = "{regulation: fictief_beleid_kroniek_toeslagen}";
    let payments = read("streams/zorgtoeslag_betalingen.yaml")
        .replace(&format!("reads: [{policy}]"), &format!("reads: {policy}"));
    assert_ne!(payments, read("streams/zorgtoeslag_betalingen.yaml"));
    let config = |decisions: &str| {
        CellConfig::from_yaml(
            &read("cell.yaml"),
            &[
                &read("streams/zorgtoeslag_aanvragen.yaml"),
                decisions,
                &payments,
            ],
        )
        .unwrap_or_else(|e| panic!("{e}"))
    };
    let article = |number: Option<&str>| Read {
        regulation: "fictief_beleid_kroniek_toeslagen".into(),
        article: number.map(Into::into),
    };
    let service = regulations();
    let received = at("2025-03-04T10:15:00+01:00");
    let day = received.date_naive();

    let split = config(&decisions);
    let (_, voorschot) = split.event("voorschot_verleend").unwrap();
    assert_eq!(voorschot.reads, [article(Some("4"))]);
    let (_, toekenning) = split.event("zorgtoeslag_toegekend").unwrap();
    assert_eq!(toekenning.reads, [article(Some("3a"))]);
    let (_, termijn) = split.event("betaalopdracht_gegeven").unwrap();
    assert_eq!(termijn.reads, [article(None)]);
    let mut cell = Cell::in_memory(split, Vec::new(), &service, day).unwrap();
    let aanvraag = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();
    let inputs = cell
        .decision_inputs(&service, "voorschot_verleend", &aanvraag.id, received)
        .unwrap();
    assert_eq!(
        inputs["bsn"].provenance,
        json!({
            "source": "lexostatus",
            "lexostatus": "aanvraag",
            "article": format!("{AWIR}#15"),
            "gram": aanvraag.id,
        })
    );
    assert_eq!(inputs["datum_ontvangst"].value, "2025-03-04");
    // The estimate is in the application too, but the policy of Toeslagen
    // (art. 4, an aanname) says how it is read: only that article is read.
    assert_eq!(
        inputs["vermoedelijk_toetsingsinkomen"].provenance,
        json!({
            "source": "lexostatus",
            "lexostatus": "schatting_inkomen",
            "register": "fictief_beleid_kroniek_toeslagen#kroniek",
            "article": "fictief_beleid_kroniek_toeslagen#4",
        })
    );
    assert_eq!(inputs["vermoedelijk_toetsingsinkomen"].value, ESTIMATE);

    let twice = config(&decisions.replacen(
        "      - {regulation: fictief_beleid_kroniek_toeslagen, article: '4'}\n",
        "      - {regulation: fictief_beleid_kroniek_toeslagen, article: '4'}\n      - {regulation: fictief_beleid_kroniek_toeslagen, article: '4'}\n",
        1,
    ));
    let mut cell = Cell::in_memory(twice, Vec::new(), &service, day).unwrap();
    let aanvraag = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();
    let e = cell
        .decision_inputs(&service, "voorschot_verleend", &aanvraag.id, received)
        .unwrap_err();
    assert!(matches!(e, Error::Setup(_)), "{e}");
    assert!(e.to_string().contains("from both"), "{e}");
}

/// An application received at `received`, the citizen's registers, and the
/// voorschot on it decided at `decided`: the cell with both grams.
fn with_voorschot(
    service: &mut LawExecutionService,
    data: &Path,
    received: &str,
    decided: &str,
) -> (Cell, Gram, Gram) {
    with_voorschot_on(service, data, received, decided, ESTIMATE, 79547)
}

/// The same, with the citizen expecting `estimate` and the registers knowing
/// `income` as the wages over the year.
fn with_voorschot_on(
    service: &mut LawExecutionService,
    data: &Path,
    received: &str,
    decided: &str,
    estimate: i64,
    income: i64,
) -> (Cell, Gram, Gram) {
    let received = at(received);
    let mut cell = cell(service, data, received);
    let mut submitted = application();
    submitted.insert("vermoedelijk_toetsingsinkomen".into(), json!(estimate));
    let application = cell
        .record_submission(service, "aanvraag_ontvangen", &submitted, received)
        .unwrap_or_else(|e| panic!("{e}"));
    register_sources_with_income(service, income);
    let voorschot = cell
        .decide(
            service,
            "voorschot_verleend",
            BTreeMap::from([("on_application".to_string(), application.id.clone())]),
            BTreeMap::new(),
            at(decided),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    (cell, application, voorschot)
}

/// Pay the voorschottermijn of the month of `day`, on that day at ten: the
/// betaalopdracht Toeslagen gives, and the bank's answer that it credited
/// the amount. Returns the order (`None` if the law gives none that month).
fn pay(cell: &mut Cell, service: &LawExecutionService, root: &str, day: &str) -> Option<Gram> {
    let order = order(cell, service, root, day)?;
    answer(cell, service, &order, true, day);
    Some(order)
}

/// Only the betaalopdracht of the month of `day`, on that day at ten.
fn order(cell: &mut Cell, service: &LawExecutionService, root: &str, day: &str) -> Option<Gram> {
    let now = at(&format!("{day}T10:00:00+01:00"));
    cell.execute(
        service,
        "betaalopdracht_gegeven",
        root,
        now.date_naive(),
        now,
    )
    .unwrap_or_else(|e| panic!("{day}: {e}"))
}

/// The bank's answer to `order`, as the channel delivers it to Toeslagen on
/// `day` at eleven: credited, or refused because the account is blocked.
fn answer(
    cell: &mut Cell,
    service: &LawExecutionService,
    order: &Gram,
    credited: bool,
    day: &str,
) -> Gram {
    let bank = json!({"source": "kanaal", "from": "bank"});
    let bedrag = order.fields["bedrag"].clone();
    let input = |v: serde_json::Value| Input {
        value: v,
        provenance: bank.clone(),
    };
    let inputs = BTreeMap::from([
        ("bijgeschreven".to_string(), input(json!(credited))),
        (
            "bijgeschreven_bedrag".to_string(),
            input(if credited { bedrag.clone() } else { json!(0) }),
        ),
        ("bedrag_opdracht".to_string(), input(bedrag)),
        (
            "reden_weigering".to_string(),
            input(if credited {
                serde_json::Value::Null
            } else {
                json!("rekening geblokkeerd")
            }),
        ),
    ]);
    let mut grams = cell
        .receive(
            service,
            "fictief_beleid_termijnbedrag_voorschot#2",
            BTreeMap::from([("betaalopdracht".to_string(), order.id.clone())]),
            inputs,
            at(&format!("{day}T11:00:00+01:00")),
            at(&format!("{day}T11:00:00+01:00")),
        )
        .unwrap_or_else(|e| panic!("{day}: {e}"));
    assert_eq!(grams.len(), 1, "one answer: {grams:?}");
    grams.remove(0)
}

/// The first day of each month from `year`-`month`, `count` of them.
fn months(year: i32, month: u32, count: u32) -> Vec<String> {
    (0..count)
        .map(|i| {
            let m = month - 1 + i;
            format!("{}-{:02}-01", year + (m / 12) as i32, m % 12 + 1)
        })
        .collect()
}

fn amount(gram: &Gram, field: &str) -> i64 {
    gram.fields[field]
        .as_i64()
        .unwrap_or_else(|| panic!("{field}: {}", gram.fields[field]))
}

/// Awir 22 lid 1: a voorschot granted before the berekeningsjaar begins is
/// paid in 12 termijnen, the first in December before it. The cell pays the
/// law per month; a termijn becomes a gram only when it is paid.
#[test]
fn a_voorschot_before_the_year_is_paid_in_twelve_termijnen() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    // A doorlopende aanvraag for 2025, received and granted in November
    // 2024, under the law of 2025 (Awir 16 lid 2).
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
    );
    assert_eq!(voorschot.period, Some(year(2025)));
    let voorschotbedrag = amount(&voorschot, "voorschotbedrag");
    assert!(voorschotbedrag > 0);

    // November: no termijn yet, no gram.
    assert_eq!(
        pay(&mut cell, &service, &application.id, "2024-11-25"),
        None
    );

    let mut paid = Vec::new();
    for day in months(2024, 12, 12) {
        let gram = pay(&mut cell, &service, &application.id, &day)
            .unwrap_or_else(|| panic!("{day}: no termijn"));
        paid.push(gram);
    }
    let first = &paid[0];
    assert_eq!(first.type_, "executogram");
    assert_eq!(first.name, "betaalopdracht_gegeven");
    assert_eq!(first.stage, None);
    assert_eq!(
        first.establishes,
        "fictief_beleid_termijnbedrag_voorschot#1"
    );
    // It refers to the voorschot by its stage, and belongs to the case of
    // the application.
    assert_eq!(first.refers_to["voorschot"], voorschot.id);
    assert_eq!(first.period, Some(year(2025)));
    assert_eq!(first.regulation_valid_from.as_deref(), Some("2025-01-01"));
    assert_eq!(first.effective_at, "2024-12-01T10:00:00+01:00");
    assert_eq!(
        first.fields.keys().collect::<Vec<_>>(),
        [
            "bedrag",
            "meegenomen_achterstand",
            "rekeningnummer_begunstigde",
            "termijnbedrag",
            "uitvoerdatum"
        ],
        "only what the law names"
    );
    // What it was paid on: the voorschot read back, and the month.
    assert_eq!(first.inputs["voorschotbedrag"]["value"], voorschotbedrag);
    assert_eq!(first.inputs["dagtekening_voorschot"]["value"], "2024-11-20");
    assert_eq!(first.inputs["berekeningsjaar"]["value"], 2025);
    assert_eq!(
        first.inputs["maand"],
        json!({"value": "2024-12-01", "provenance": {"source": "execution"}})
    );
    // Twelve termijnen, December to November, together the voorschot.
    let total: i64 = paid.iter().map(|g| amount(g, "termijnbedrag")).sum();
    assert_eq!(total, voorschotbedrag);
    assert_eq!(amount(&paid[0], "termijnbedrag"), voorschotbedrag / 12);

    // December after the year: no more termijnen.
    assert_eq!(
        pay(&mut cell, &service, &application.id, "2025-12-01"),
        None
    );
    let termijnen = cell
        .grams()
        .filter(|g| g.name == "betaalopdracht_gegeven")
        .count();
    assert_eq!(termijnen, 12);
}

/// Awir 22 lid 2 and 4: granted in March, the months that passed are paid
/// at once with the first termijn, the rest in termijnen until November.
#[test]
fn a_voorschot_in_march_pays_the_passed_months_at_once() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2025-03-04T10:15:00+01:00",
        "2025-03-10T09:00:00+01:00",
    );
    let voorschotbedrag = amount(&voorschot, "voorschotbedrag");
    let paid: Vec<Gram> = ["2025-03-20"]
        .into_iter()
        .map(String::from)
        .chain(months(2025, 4, 8))
        .map(|day| {
            pay(&mut cell, &service, &application.id, &day)
                .unwrap_or_else(|| panic!("{day}: no termijn"))
        })
        .collect();
    assert_eq!(paid.len(), 9);
    // The first termijn carries January to March at once.
    let at_once = (voorschotbedrag * 3 + 6) / 12;
    let per_termijn = (voorschotbedrag - at_once) / 9;
    assert_eq!(amount(&paid[0], "termijnbedrag"), at_once + per_termijn);
    assert_eq!(amount(&paid[1], "termijnbedrag"), per_termijn);
    let total: i64 = paid.iter().map(|g| amount(g, "termijnbedrag")).sum();
    assert_eq!(total, voorschotbedrag);
}

/// A termijn is paid once a month, never before the voorschot it executes,
/// and never ahead of time: a termijn that has yet to come is not a fact.
#[test]
fn a_termijn_is_paid_once_and_not_ahead_of_time() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2025-03-04T10:15:00+01:00",
        "2025-03-10T09:00:00+01:00",
    );
    let refused = |cell: &mut Cell, on: &str, now: &str| {
        let e = cell
            .execute(
                &service,
                "betaalopdracht_gegeven",
                &application.id,
                on.parse().unwrap(),
                at(now),
            )
            .unwrap_err();
        assert!(matches!(e, Error::Refused(_)), "{on}: {e}");
        e.to_string()
    };
    // Before the voorschot it executes.
    refused(&mut cell, "2025-03-01", "2025-03-20T10:00:00+01:00");
    // Ahead of time.
    let e = refused(&mut cell, "2025-04-01", "2025-03-20T10:00:00+01:00");
    assert!(e.contains("not a fact"), "{e}");
    // Once a month.
    assert!(pay(&mut cell, &service, &application.id, "2025-04-01").is_some());
    let e = refused(&mut cell, "2025-04-15", "2025-04-15T10:00:00+02:00");
    assert!(e.contains("already"), "{e}");
    // A decision is not executed on a day, and a case without a voorschot
    // has nothing to execute.
    let e = cell
        .execute(
            &service,
            "voorschot_verleend",
            &application.id,
            "2025-04-15".parse().unwrap(),
            at("2025-04-15T10:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    let other = cell
        .record_submission(
            &service,
            "aanvraag_ontvangen",
            &application_for(2025),
            at("2025-04-15T10:00:00+02:00"),
        )
        .unwrap();
    let e = cell
        .execute(
            &service,
            "betaalopdracht_gegeven",
            &other.id,
            "2025-04-15".parse().unwrap(),
            at("2025-04-15T10:00:00+02:00"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("has none"), "{e}");
}

/// A reading is the state at a moment: a termijn paid later does not count
/// at an earlier moment, and the sum of nothing is zero.
#[test]
fn a_reading_counts_only_what_holds_at_its_moment() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
    );
    let root = object(json!({"root": application.id, "berekeningsjaar": 2025}));
    let paid = |cell: &Cell, moment: &str| {
        read(cell, &service, "uitbetaald", &root, at(moment))["uitbetaalde_voorschotten"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(paid(&cell, "2025-06-30T12:00:00+02:00"), 0);
    let termijnen: Vec<Gram> = months(2024, 12, 6)
        .iter()
        .map(|day| pay(&mut cell, &service, &application.id, day).unwrap())
        .collect();
    let sum = |n: usize| -> i64 {
        termijnen[..n]
            .iter()
            .map(|g| amount(g, "termijnbedrag"))
            .sum()
    };
    assert_eq!(paid(&cell, "2025-06-30T12:00:00+02:00"), sum(6));
    assert_eq!(paid(&cell, "2025-03-15T12:00:00+01:00"), sum(4));
    // Before the first termijn: nothing paid.
    assert_eq!(paid(&cell, "2024-11-30T12:00:00+01:00"), 0);

    // The voorschot, as the policy of Toeslagen reads it back: before it
    // was granted it is not there (the policy leaves it empty).
    let read = |moment: &str| {
        cell.read_case(
            &service,
            "betaalopdracht_gegeven",
            &application.id,
            Some(("berekeningsjaar", 2025)),
            at(moment),
        )
        .unwrap()
    };
    let before = read("2024-11-19T12:00:00+01:00");
    assert!(!before.contains_key("voorschotbedrag"), "{before:?}");
    assert!(!before.contains_key("dagtekening_voorschot"), "{before:?}");
    let after = read("2024-11-20T12:00:00+01:00");
    assert_eq!(
        after["voorschotbedrag"].value,
        voorschot.fields["voorschotbedrag"]
    );
    assert_eq!(after["dagtekening_voorschot"].value, "2024-11-20");
    // The berekeningsjaar is the cell's to give: the case is read for it.
    assert!(!after.contains_key("berekeningsjaar"), "{after:?}");
    // Where it came from: the register, and the article of the policy that
    // read it (the engine says which).
    assert_eq!(
        after["voorschotbedrag"].provenance,
        json!({
            "source": "lexostatus",
            "lexostatus": "voorschot",
            "register": "fictief_beleid_kroniek_toeslagen#kroniek",
            "article": "fictief_beleid_kroniek_toeslagen#1",
        })
    );
}

/// The register a policy reads must be bound with the engine: a cell over a
/// service without it does not start, rather than read nothing.
#[test]
fn a_cell_whose_register_is_not_bound_does_not_start() {
    let data = tempfile::tempdir().unwrap();
    let service =
        load_regulations(&root().join("corpus/regulation")).unwrap_or_else(|e| panic!("{e}"));
    let e = Cell::open(
        &cell_yaml(),
        &service,
        data.path(),
        "2025-03-04".parse().unwrap(),
    )
    .err()
    .expect("a cell with an unbound register must not start");
    assert!(matches!(e, Error::Setup(_)), "{e}");
    assert!(e.to_string().contains("not bound"), "{e}");
}

/// An event can only read a policy that reads a register of the cell.
#[test]
fn an_event_reading_a_policy_without_register_is_refused() {
    let fixture = cell_yaml().parent().unwrap().to_path_buf();
    let read = |f: &str| std::fs::read_to_string(fixture.join(f)).unwrap();
    let cell = read("cell.yaml");
    let without = &cell[..cell.find("registers:").unwrap()];
    let e = CellConfig::from_yaml(
        without,
        &[
            &read("streams/zorgtoeslag_aanvragen.yaml"),
            &read("streams/zorgtoeslag_besluiten.yaml"),
            &read("streams/zorgtoeslag_betalingen.yaml"),
        ],
    )
    .unwrap_err();
    assert!(e.to_string().contains("reads no register"), "{e}");
}

/// The application, the voorschot in March on `estimate`, every termijn of
/// it paid, and a year later the toekenning on `income`: the toekenning
/// gram, with what was paid.
fn toekenning_after_termijnen(estimate: i64, income: i64) -> (Gram, Gram, i64) {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, voorschot) = with_voorschot_on(
        &mut service,
        data.path(),
        "2025-03-04T10:15:00+01:00",
        "2025-03-10T09:00:00+01:00",
        estimate,
        income,
    );
    let paid: i64 = ["2025-03-20".to_string()]
        .into_iter()
        .chain(months(2025, 4, 8))
        .map(|day| {
            amount(
                &pay(&mut cell, &service, &application.id, &day).unwrap(),
                "termijnbedrag",
            )
        })
        .sum();
    assert_eq!(paid, amount(&voorschot, "voorschotbedrag"));
    let toekenning = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            BTreeMap::from([("on_application".to_string(), application.id.clone())]),
            // Awir 19 lid 1: the aanslag over 2025, from the dossier.
            BTreeMap::from([(
                "datum_vaststelling_aanslag".to_string(),
                Input {
                    value: json!("2026-03-15"),
                    provenance: json!({"source": "dossier"}),
                },
            )]),
            at("2026-06-01T09:00:00+02:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    (voorschot, toekenning, paid)
}

/// Awir 24 lid 2 sets off the voorschotten paid out: only what the bank
/// credited (`voorschottermijn_betaald`), not an order without an answer and
/// not an order the bank refused.
#[test]
fn the_verrekening_counts_only_what_the_bank_credited() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2025-03-04T10:15:00+01:00",
        "2025-03-10T09:00:00+01:00",
    );
    let credited = pay(&mut cell, &service, &application.id, "2025-03-20").unwrap();
    let refused = order(&mut cell, &service, &application.id, "2025-04-01").unwrap();
    let failed = answer(&mut cell, &service, &refused, false, "2025-04-01");
    assert_eq!(failed.name, "betaling_mislukt");
    assert_eq!(failed.fields["mislukt_bedrag"], refused.fields["bedrag"]);
    assert_eq!(failed.fields["reden"], "rekening geblokkeerd");
    // May: the order carries April's failed amount, and has no answer yet.
    let pending = order(&mut cell, &service, &application.id, "2025-05-01").unwrap();
    assert_eq!(
        pending.fields["meegenomen_achterstand"],
        refused.fields["bedrag"]
    );
    let toekenning = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            BTreeMap::from([("on_application".to_string(), application.id.clone())]),
            BTreeMap::from([(
                "datum_vaststelling_aanslag".to_string(),
                Input {
                    value: json!("2026-03-15"),
                    provenance: json!({"source": "dossier"}),
                },
            )]),
            at("2026-06-01T09:00:00+02:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        toekenning.inputs["uitbetaalde_voorschotten"]["value"],
        credited.fields["bedrag"]
    );
}

/// The rounded tegemoetkoming (Awir 14 lid 4) the law gives on `income`.
fn awarded(income: i64) -> i64 {
    let hoogte = hoogte_on(income, "2025-01-01").as_i64().unwrap();
    (hoogte + 50) / 100 * 100
}

/// Awir 19, 24 lid 1 and 2: the toekenning on a definitive income higher
/// than the estimate sets off what was paid on the voorschot and pays out
/// the rest within four weeks. Nothing is recovered.
#[test]
fn a_toekenning_above_the_voorschot_pays_out_the_rest() {
    let (_, toekenning, paid) = toekenning_after_termijnen(ESTIMATE, 79547);
    assert_eq!(toekenning.stage.as_deref(), Some("TOEKENNING"));
    // What was paid, read from the chronicle at the moment of the decision.
    assert_eq!(
        toekenning.inputs["uitbetaalde_voorschotten"],
        json!({
            "value": paid,
            "provenance": {
                    "source": "lexostatus",
                    "lexostatus": "uitbetaald",
                    "register": "fictief_beleid_kroniek_toeslagen#kroniek",
                    "article": "fictief_beleid_kroniek_toeslagen#3a",
                },
        })
    );
    let toegekend = awarded(79547);
    assert_eq!(toekenning.fields["toegekende_tegemoetkoming"], toegekend);
    assert!(toegekend > paid);
    assert_eq!(toekenning.fields["nog_uit_te_betalen"], toegekend - paid);
    assert_eq!(toekenning.fields["terug_te_vorderen_na_verrekening"], 0);
    assert_eq!(toekenning.fields["terug_te_vorderen"], 0);
    assert_eq!(toekenning.fields["uiterste_uitbetaaldatum"], "2026-06-29");
    assert_eq!(toekenning.fields["uiterste_toekenningsdatum"], "2026-09-15");
    for basis in ["19", "24", "26a"] {
        let basis = format!("{AWIR}#{basis}");
        assert!(toekenning.legal_basis.contains(&basis), "{basis}");
    }
}

/// Awir 24 lid 3 and 26a: on a definitive income higher than the estimate
/// the voorschot was too high, and the set-off leaves an amount to recover;
/// above € 118 it is recovered.
#[test]
fn a_toekenning_below_the_voorschot_recovers_the_difference() {
    let income = 3_000_000;
    let (_, toekenning, paid) = toekenning_after_termijnen(79547, income);
    let toegekend = awarded(income);
    assert!(paid - toegekend > 11_800, "{paid} - {toegekend}");
    assert_eq!(toekenning.fields["nog_uit_te_betalen"], 0);
    assert_eq!(
        toekenning.fields["terug_te_vorderen_na_verrekening"],
        paid - toegekend
    );
    assert_eq!(toekenning.fields["terug_te_vorderen"], paid - toegekend);
}

/// Awir 26a lid 1 (text of 2025): an amount to recover of at most € 118 is
/// not recovered; the terugvordering is nihil.
#[test]
fn a_small_amount_to_recover_is_not_recovered() {
    // Both on the slope of the zorgtoeslag: € 500 more income is about € 69
    // less zorgtoeslag.
    let (estimate, income) = (3_000_000, 3_050_000);
    let (_, toekenning, paid) = toekenning_after_termijnen(estimate, income);
    let difference = paid - awarded(income);
    assert!(
        difference > 0 && difference <= 11_800,
        "pick incomes whose difference is at most € 118, not {difference}"
    );
    assert_eq!(
        toekenning.fields["terug_te_vorderen_na_verrekening"],
        difference
    );
    assert_eq!(toekenning.fields["terug_te_vorderen"], 0);
    assert_eq!(toekenning.fields["nog_uit_te_betalen"], 0);
}

/// After the toekenning the voorschot is set off (Awir 24 lid 2): a termijn
/// not paid by then is not paid any more, and that follows from the
/// toekenning, not from a gram of its own. What the toekenning reads is what
/// was paid at its moment.
#[test]
fn after_the_toekenning_no_termijn_is_paid() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2025-03-04T10:15:00+01:00",
        "2025-03-10T09:00:00+01:00",
    );
    let paid: i64 = ["2025-03-20", "2025-04-01"]
        .iter()
        .map(|day| {
            amount(
                &pay(&mut cell, &service, &application.id, day).unwrap(),
                "termijnbedrag",
            )
        })
        .sum();
    let toekenning = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            BTreeMap::from([("on_application".to_string(), application.id.clone())]),
            BTreeMap::new(),
            at("2025-04-15T09:00:00+02:00"),
        )
        .unwrap();
    assert_eq!(toekenning.inputs["uitbetaalde_voorschotten"]["value"], paid);
    let before = cell.grams().count();
    let e = cell
        .execute(
            &service,
            "betaalopdracht_gegeven",
            &application.id,
            "2025-05-01".parse().unwrap(),
            at("2025-05-01T10:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Ended(_)), "{e}");
    assert!(e.to_string().contains("TOEKENNING"), "{e}");
    assert_eq!(cell.grams().count(), before);
}

fn days(list: &[&str]) -> Vec<chrono::NaiveDate> {
    list.iter().map(|d| d.parse().unwrap()).collect()
}

/// The cell says on which days a termijn is due: per month the day the
/// policy executing art. 22 gives (the first), or the day the voorschot
/// holds if that is later, never twice in a month and never after the
/// toekenning. Whether a termijn falls on such a day is the law's to say.
#[test]
fn the_cell_says_which_days_a_termijn_is_due() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let due = |service: &LawExecutionService,
               cell: &Cell,
               after: Option<&str>,
               through: &str,
               now: &str| {
        cell.due_executions(
            service,
            "betaalopdracht_gegeven",
            &cell
                .grams()
                .find(|g| g.name == "aanvraag_ontvangen")
                .unwrap()
                .id
                .clone(),
            after.map(|a| a.parse().unwrap()),
            through.parse().unwrap(),
            at(now),
        )
        .unwrap_or_else(|e| panic!("{e}"))
        .into_iter()
        .map(|d| d.day)
        .collect::<Vec<_>>()
    };
    // Before the voorschot there is nothing to execute.
    let received = at("2025-03-04T10:15:00+01:00");
    let mut early = cell(&service, data.path(), received);
    early
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();
    assert!(due(
        &service,
        &early,
        None,
        "2025-06-30",
        "2025-03-05T10:00:00+01:00"
    )
    .is_empty());

    let data = tempfile::tempdir().unwrap();
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2025-03-04T10:15:00+01:00",
        "2025-03-10T09:00:00+01:00",
    );
    // At the moment of the voorschot: that day itself.
    assert_eq!(
        due(
            &service,
            &cell,
            None,
            "2025-03-10",
            "2025-03-10T09:00:00+01:00"
        ),
        days(&["2025-03-10"])
    );
    // Later: the same day, whenever the cell is asked; then the first of
    // every month, also ahead of time.
    assert_eq!(
        due(
            &service,
            &cell,
            None,
            "2025-06-15",
            "2025-04-20T10:00:00+02:00"
        ),
        days(&["2025-03-10", "2025-04-01", "2025-05-01", "2025-06-01"])
    );
    // A month that has a termijn is done; what was asked already is too.
    pay(&mut cell, &service, &application.id, "2025-04-01").unwrap();
    // Executed later on the day of the voorschot, the termijn holds from
    // the voorschot's moment, not from the start of that day.
    let late = cell
        .preview_execution(
            &service,
            "betaalopdracht_gegeven",
            &application.id,
            "2025-03-10".parse().unwrap(),
            at("2025-04-20T10:00:00+02:00"),
        )
        .unwrap()
        .unwrap();
    assert_eq!(late.effective_at, "2025-03-10T09:00:00+01:00");
    assert_eq!(
        due(
            &service,
            &cell,
            None,
            "2025-06-15",
            "2025-04-20T10:00:00+02:00"
        ),
        days(&["2025-03-10", "2025-05-01", "2025-06-01"])
    );
    assert_eq!(
        due(
            &service,
            &cell,
            Some("2025-05-01"),
            "2025-06-15",
            "2025-04-20T10:00:00+02:00"
        ),
        days(&["2025-06-01"])
    );
    // After the toekenning nothing is due any more.
    cell.decide(
        &service,
        "zorgtoeslag_toegekend",
        BTreeMap::from([("on_application".to_string(), application.id.clone())]),
        BTreeMap::new(),
        at("2025-04-21T09:00:00+02:00"),
    )
    .unwrap();
    assert!(due(
        &service,
        &cell,
        None,
        "2025-06-15",
        "2025-04-21T10:00:00+02:00"
    )
    .is_empty());
}

/// Once a month is once a month for the case, whichever voorschot a termijn
/// refers to: a second voorschot on the same application (a herziening) does
/// not make a second termijn in the same month.
#[test]
fn a_herziening_does_not_pay_twice_in_a_month() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, first) = with_voorschot(
        &mut service,
        data.path(),
        "2025-03-04T10:15:00+01:00",
        "2025-03-10T09:00:00+01:00",
    );
    let paid = pay(&mut cell, &service, &application.id, "2025-04-01").unwrap();
    assert_eq!(paid.refers_to["voorschot"], first.id);
    // A herziening is a voorschot for the same berekeningsjaar: the caller
    // says which. Without it, the next voorschot is that of the next year,
    // which Toeslagen grants only on 1 November before it.
    let on_application = BTreeMap::from([("on_application".to_string(), application.id.clone())]);
    let e = cell
        .decide(
            &service,
            "voorschot_verleend",
            on_application.clone(),
            BTreeMap::new(),
            at("2025-04-10T09:00:00+02:00"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("2025-11-01"), "{e}");
    let second = cell
        .decide(
            &service,
            "voorschot_verleend",
            on_application,
            BTreeMap::from([(
                "berekeningsjaar".to_string(),
                Input {
                    value: json!(2025),
                    provenance: json!({"source": "caller"}),
                },
            )]),
            at("2025-04-10T09:00:00+02:00"),
        )
        .unwrap();
    assert_eq!(second.period, Some(year(2025)));
    let e = cell
        .execute(
            &service,
            "betaalopdracht_gegeven",
            &application.id,
            "2025-04-15".parse().unwrap(),
            at("2025-04-15T10:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    assert!(e.to_string().contains("already"), "{e}");
    // The next month, the termijn executes the latest voorschot.
    let next = pay(&mut cell, &service, &application.id, "2025-05-01").unwrap();
    assert_eq!(next.refers_to["voorschot"], second.id);
}

/// Time only moves forward: the cell does not record a gram at a moment
/// before the last one it recorded.
#[test]
fn a_gram_is_not_recorded_before_the_last_one() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2025-03-04T10:15:00+01:00",
        "2025-03-10T09:00:00+01:00",
    );
    let e = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            BTreeMap::from([("on_application".to_string(), application.id.clone())]),
            BTreeMap::new(),
            at("2025-03-09T09:00:00+01:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    assert!(e.to_string().contains("time does not go back"), "{e}");
}

/// What a lexostatus gives is laid down in the law or the policy: the
/// application gives what the decisions ask of it, as the law declares the
/// field, and the day of receipt as a date; the article of the policy gives
/// the sum of the termijnen paid as an amount in eurocent.
#[test]
fn a_lexostatus_takes_its_shape_from_the_law_or_the_policy() {
    let data = tempfile::tempdir().unwrap();
    let service = regulations();
    let cell = cell(&service, data.path(), at("2025-03-04T10:15:00+01:00"));
    let all = cell
        .lexostatuses(&service, "2025-06-01".parse().unwrap())
        .unwrap();
    let by_name =
        |name: &str| serde_json::to_value(all.iter().find(|l| l.name() == name).unwrap()).unwrap();
    let aanvraag = by_name("aanvraag");
    assert_eq!(aanvraag["kind"], "submission");
    assert_eq!(aanvraag["provision"], format!("{AWIR}#15"));
    let fields: Vec<(&str, &str)> = aanvraag["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["name"].as_str().unwrap(), f["type"].as_str().unwrap()))
        .collect();
    assert!(fields.contains(&("bsn", "string")), "{fields:?}");
    assert!(fields.contains(&("datum_ontvangst", "date")), "{fields:?}");
    let uitbetaald = by_name("uitbetaald");
    assert_eq!(uitbetaald["kind"], "policy");
    assert_eq!(
        uitbetaald["provision"],
        "fictief_beleid_kroniek_toeslagen#3a"
    );
    assert_eq!(uitbetaald["period"], "berekeningsjaar");
    assert_eq!(
        uitbetaald["fields"],
        json!([{
            "name": "uitbetaalde_voorschotten",
            "type": "amount",
            "unit": "eurocent",
            "legal_basis": ["fictief_beleid_kroniek_toeslagen#3a"],
            "declared_by": "fictief_beleid_kroniek_toeslagen#3a",
            "description": "Wat de bank op het voorschot voor het berekeningsjaar bijschreef.",
        }])
    );
    let readers: Vec<&str> = uitbetaald["read_by"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["event"].as_str().unwrap())
        .collect();
    assert_eq!(readers, ["zorgtoeslag_toegekend"]);
}

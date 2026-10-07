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
use regelrecht_cel::config::CellConfig;
use regelrecht_cel::extension::PeriodUnit;
use regelrecht_cel::{Cell, Error, Gram, Input, Period};
use regelrecht_engine::{LawExecutionService, Value};
use serde_json::{json, Map};

const BSN: &str = "999993653";
const AWIR: &str = "algemene_wet_inkomensafhankelijke_regelingen";
const ZORGTOESLAG: &str = "wet_op_de_zorgtoeslag";
/// The toetsingsinkomen the citizen expects (Awir 16): 30.000 euro, far
/// above what the registers will know of the year (795,47 euro).
const ESTIMATE: i64 = 3_000_000;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cell_yaml() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/toeslagen/cell.yaml")
}

fn regulations() -> LawExecutionService {
    load_regulations(&root().join("corpus/regulation")).unwrap_or_else(|e| panic!("{e}"))
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
    let read = cell
        .read(
            "aanvraag",
            &object(json!({"root": application.id})),
            received,
        )
        .unwrap();
    assert_eq!(
        serde_json::Value::Object(read.clone()),
        json!({
            "bsn": BSN,
            "aangevraagd_berekeningsjaar": 2025,
            "vermoedelijk_toetsingsinkomen": ESTIMATE,
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
    // The decision reads its parameters from the lexostatus its event
    // `reads`, kept to what its stage asks (the bsn; not the day of receipt,
    // and at the toekenning not the estimate, which only Awir 16 asks at the
    // voorschot) and the berekeningsjaar it concerns. `decision_inputs` shows
    // what `decide` will read.
    let inputs = cell
        .decision_inputs(&service, "zorgtoeslag_toegekend", &application.id, decided)
        .unwrap_or_else(|e| panic!("{e}"));
    let from_case = |name: &str| {
        (
            name.to_string(),
            Input {
                value: read[name].clone(),
                provenance: json!({"source": "lexostatus", "lexostatus": "aanvraag"}),
            },
        )
    };
    // Awir 24 asks what was paid on the voorschot: nothing yet.
    let paid = (
        "uitbetaalde_voorschotten".to_string(),
        Input {
            value: json!(0),
            provenance: json!({"source": "lexostatus", "lexostatus": "uitbetaald"}),
        },
    );
    assert_eq!(
        inputs,
        BTreeMap::from([
            from_case("bsn"),
            from_case("aangevraagd_berekeningsjaar"),
            paid
        ])
    );
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
    let read_again = cell
        .read(
            "aanvraag",
            &object(json!({"root": application.id})),
            decided,
        )
        .unwrap();
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
    // The year took part in the decision, from the case.
    assert_eq!(
        decision.inputs["aangevraagd_berekeningsjaar"],
        json!({
            "value": 2025,
            "provenance": {"source": "lexostatus", "lexostatus": "aanvraag"},
        })
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

/// A lexostatus that reads a field no gram has is refused when the cell
/// starts: a typo would otherwise read as a fact nobody has.
#[test]
fn a_lexostatus_reading_an_unknown_field_is_refused() {
    let config = tempfile::tempdir().unwrap();
    let fixture = cell_yaml().parent().unwrap().to_path_buf();
    std::fs::create_dir_all(config.path().join("streams")).unwrap();
    for f in [
        "cell.yaml",
        "streams/zorgtoeslag_aanvragen.yaml",
        "streams/zorgtoeslag_besluiten.yaml",
        "streams/zorgtoeslag_betalingen.yaml",
    ] {
        std::fs::copy(fixture.join(f), config.path().join(f)).unwrap();
    }
    let lexostatuses = std::fs::read_to_string(fixture.join("lexostatuses.yaml"))
        .unwrap()
        .replace("field: bsn", "field: bsnn");
    std::fs::write(config.path().join("lexostatuses.yaml"), lexostatuses).unwrap();

    let data = tempfile::tempdir().unwrap();
    let Err(e) = Cell::open(
        &config.path().join("cell.yaml"),
        &regulations(),
        data.path(),
        at("2025-03-04T10:15:00+01:00").date_naive(),
    ) else {
        panic!("a lexostatus reading 'bsnn' must be refused");
    };
    assert!(matches!(e, Error::Setup(_)), "{e}");
    assert!(e.to_string().contains("'bsnn'"), "{e}");
}

/// An event that reads a lexostatus the cell does not define is refused
/// when the configuration is read.
#[test]
fn an_event_reading_an_unknown_lexostatus_is_refused() {
    let fixture = cell_yaml().parent().unwrap().to_path_buf();
    let read = |f: &str| std::fs::read_to_string(fixture.join(f)).unwrap();
    // Every lexostatus of the list is checked, not only the first.
    let decisions = read("streams/zorgtoeslag_besluiten.yaml").replacen(
        "reads: [aanvraag]",
        "reads: [aanvraag, aanvragen]",
        1,
    );
    assert_ne!(decisions, read("streams/zorgtoeslag_besluiten.yaml"));
    let e = CellConfig::from_yaml(
        &read("cell.yaml"),
        &[&read("streams/zorgtoeslag_aanvragen.yaml"), &decisions],
        Some(&read("lexostatuses.yaml")),
    )
    .unwrap_err();
    assert!(matches!(e, Error::Setup(_)), "{e}");
    assert!(e.to_string().contains("'aanvragen'"), "{e}");
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

/// The lexostatus a decision reads takes the case's root and nothing else:
/// the cell passes only that.
#[test]
fn an_event_reading_a_lexostatus_without_root_is_refused() {
    let fixture = cell_yaml().parent().unwrap().to_path_buf();
    let read = |f: &str| std::fs::read_to_string(fixture.join(f)).unwrap();
    let streams = [
        read("streams/zorgtoeslag_aanvragen.yaml"),
        read("streams/zorgtoeslag_besluiten.yaml"),
    ];
    let streams = [streams[0].as_str(), streams[1].as_str()];
    for (from, to) in [
        ("inputs: [root]", "inputs: [root, jaar]"),
        ("root: $root}", "root: $jaar}"),
    ] {
        let lexostatuses = read("lexostatuses.yaml");
        assert!(lexostatuses.contains(from), "{from}");
        let e = CellConfig::from_yaml(
            &read("cell.yaml"),
            &streams,
            Some(&lexostatuses.replace(from, to)),
        )
        .unwrap_err();
        assert!(matches!(e, Error::Setup(_)), "{to}: {e}");
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
    let read = again
        .read("aanvraag", &object(json!({"root": gram.id})), received)
        .unwrap();
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
            "aangevraagd_berekeningsjaar",
            "bsn",
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

/// A decision may read several lexostatuses; a parameter two of them give is
/// ambiguous, and refused.
#[test]
fn a_decision_reads_several_lexostatuses() {
    let fixture = cell_yaml().parent().unwrap().to_path_buf();
    let read = |f: &str| std::fs::read_to_string(fixture.join(f)).unwrap();
    let estimate = "        vermoedelijk_toetsingsinkomen:\n          field: vermoedelijk_toetsingsinkomen\n          legal_basis: [algemene_wet_inkomensafhankelijke_regelingen#16 lid 1]\n";
    let lexostatuses = read("lexostatuses.yaml");
    assert!(lexostatuses.contains(estimate));
    let schatting = |derivations: &str| {
        format!(
            "{}\n  - name: schatting\n    inputs: [root]\n    reduction:\n      chronicle: toeslagen\n      filter: {{type: submission, subtype: aanvraag, root: $root}}\n      pick: latest\n      derivations:\n{derivations}",
            lexostatuses.replace(estimate, "").trim_end()
        )
    };
    let decisions = read("streams/zorgtoeslag_besluiten.yaml").replacen(
        "reads: [aanvraag]",
        "reads: [aanvraag, schatting]",
        1,
    );
    // One name is a list of one.
    let payments = read("streams/zorgtoeslag_betalingen.yaml")
        .replace("reads: [voorschot]", "reads: voorschot");
    let config = |lexostatuses: &str| {
        CellConfig::from_yaml(
            &read("cell.yaml"),
            &[
                &read("streams/zorgtoeslag_aanvragen.yaml"),
                &decisions,
                &payments,
            ],
            Some(lexostatuses),
        )
        .unwrap_or_else(|e| panic!("{e}"))
    };
    let service = regulations();
    let received = at("2025-03-04T10:15:00+01:00");
    let day = received.date_naive();

    let split = config(&schatting(estimate));
    let (_, toekenning) = split.event("zorgtoeslag_toegekend").unwrap();
    assert_eq!(toekenning.reads, ["aanvraag", "uitbetaald"]);
    let (_, termijn) = split.event("voorschottermijn_betaald").unwrap();
    assert_eq!(termijn.reads, ["voorschot"]);
    let mut cell = Cell::in_memory(split, Vec::new(), &service, day).unwrap();
    let aanvraag = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();
    let inputs = cell
        .decision_inputs(&service, "voorschot_verleend", &aanvraag.id, received)
        .unwrap();
    assert_eq!(inputs["bsn"].provenance["lexostatus"], "aanvraag");
    assert_eq!(
        inputs["vermoedelijk_toetsingsinkomen"].provenance["lexostatus"],
        "schatting"
    );
    assert_eq!(inputs["vermoedelijk_toetsingsinkomen"].value, ESTIMATE);

    let twice = config(&schatting(&format!(
        "{estimate}        bsn:\n          field: bsn\n"
    )));
    let mut cell = Cell::in_memory(twice, Vec::new(), &service, day).unwrap();
    let aanvraag = cell
        .record_submission(&service, "aanvraag_ontvangen", &application(), received)
        .unwrap();
    let e = cell
        .decision_inputs(&service, "voorschot_verleend", &aanvraag.id, received)
        .unwrap_err();
    assert!(matches!(e, Error::Setup(_)), "{e}");
    assert!(e.to_string().contains("'bsn'"), "{e}");
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

/// Pay the voorschottermijn of the month of `day`, on that day at ten.
fn pay(cell: &mut Cell, service: &LawExecutionService, root: &str, day: &str) -> Option<Gram> {
    let now = at(&format!("{day}T10:00:00+01:00"));
    cell.execute(
        service,
        "voorschottermijn_betaald",
        root,
        now.date_naive(),
        now,
    )
    .unwrap_or_else(|e| panic!("{day}: {e}"))
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
    assert_eq!(first.name, "voorschottermijn_betaald");
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
        ["termijnbedrag"],
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
        .filter(|g| g.name == "voorschottermijn_betaald")
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
                "voorschottermijn_betaald",
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
            "voorschottermijn_betaald",
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
    let root = object(json!({"root": application.id}));
    let paid = |cell: &Cell, moment: &str| {
        cell.read("uitbetaald", &root, at(moment)).unwrap()["uitbetaalde_voorschotten"]
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

    // The voorschot read before it was granted is not there.
    let e = cell
        .read("voorschot", &root, at("2024-11-19T12:00:00+01:00"))
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    let read = cell
        .read("voorschot", &root, at("2024-11-20T12:00:00+01:00"))
        .unwrap();
    assert_eq!(read["voorschotbedrag"], voorschot.fields["voorschotbedrag"]);
    assert_eq!(read["dagtekening_voorschot"], "2024-11-20");
    assert_eq!(read["berekeningsjaar"], 2025);
}

/// A sum reads many grams, so it goes with `pick: all`; `pick: all` reads
/// nothing but sums.
#[test]
fn a_sum_goes_with_pick_all() {
    let fixture = cell_yaml().parent().unwrap().to_path_buf();
    let read = |f: &str| std::fs::read_to_string(fixture.join(f)).unwrap();
    let streams = [
        read("streams/zorgtoeslag_aanvragen.yaml"),
        read("streams/zorgtoeslag_besluiten.yaml"),
        read("streams/zorgtoeslag_betalingen.yaml"),
    ];
    let streams: Vec<&str> = streams.iter().map(String::as_str).collect();
    let lexostatuses = read("lexostatuses.yaml");
    for (from, to) in [
        ("pick: all", "pick: latest"),
        (
            "          sum: termijnbedrag",
            "          field: termijnbedrag",
        ),
    ] {
        assert!(lexostatuses.contains(from), "{from}");
        let e = CellConfig::from_yaml(
            &read("cell.yaml"),
            &streams,
            Some(&lexostatuses.replace(from, to)),
        )
        .unwrap_err();
        assert!(matches!(e, Error::Setup(_)), "{to}: {e}");
        assert!(e.to_string().contains("uitbetaalde_voorschotten"), "{e}");
    }
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
            "provenance": {"source": "lexostatus", "lexostatus": "uitbetaald"},
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
            "voorschottermijn_betaald",
            &application.id,
            "2025-05-01".parse().unwrap(),
            at("2025-05-01T10:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    assert!(e.to_string().contains("TOEKENNING"), "{e}");
    assert_eq!(cell.grams().count(), before);
}

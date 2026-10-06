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
use regelrecht_cel::{Cell, Error, Gram, Input};
use regelrecht_engine::{LawExecutionService, Value};
use serde_json::{json, Map};

const BSN: &str = "999993653";
const AWIR: &str = "algemene_wet_inkomensafhankelijke_regelingen";
const ZORGTOESLAG: &str = "wet_op_de_zorgtoeslag";

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

fn object(v: serde_json::Value) -> Map<String, serde_json::Value> {
    v.as_object().unwrap().clone()
}

/// The application as the citizen fills it in: what Awir 15 and Awb 4:2 lid
/// 1 ask of them.
fn application() -> Map<String, serde_json::Value> {
    object(json!({
        "bsn": BSN,
        "aangevraagd_berekeningsjaar": 2025,
        "naam_aanvrager": "J. Voorbeeld",
        "adres_aanvrager": "Voorbeeldstraat 1, 2511 AA Den Haag",
        "dagtekening": "2025-03-03",
        "ondertekening": "J. Voorbeeld",
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
                ("loon_uit_dienstbetrekking", Value::Int(79547)),
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
                ("belastbaar_loon", Value::Int(79547)),
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
        .read("aanvraag", &object(json!({"root": application.id})))
        .unwrap();
    assert_eq!(
        serde_json::Value::Object(read.clone()),
        json!({
            "bsn": BSN,
            "aangevraagd_berekeningsjaar": 2025,
            "datum_ontvangst": "2025-03-04",
            "mede_ondertekend_door_partner": false,
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
    let inputs = BTreeMap::from([(
        "bsn".to_string(),
        Input {
            value: read["bsn"].clone(),
            provenance: json!({"source": "own", "lexostatus": "aanvraag"}),
        },
    )]);
    let refers_to = BTreeMap::from([("on_application".to_string(), application.id.clone())]);
    let decision = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            refers_to,
            inputs,
            decided,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(decision.type_, "decretogram");
    assert_eq!(decision.stage.as_deref(), Some("BESLUIT"));
    assert_eq!(decision.legal_character.as_deref(), Some("BESCHIKKING"));
    assert_eq!(decision.decision_type.as_deref(), Some("TOEKENNING"));
    assert_eq!(decision.regulation.as_deref(), Some(ZORGTOESLAG));
    assert_eq!(
        decision.regulation_valid_from.as_deref(),
        Some("2025-01-01")
    );
    assert_eq!(decision.refers_to["on_application"], application.id);
    assert_eq!(decision.inputs["bsn"]["provenance"]["source"], "own");

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
        .read("aanvraag", &object(json!({"root": application.id})))
        .unwrap();
    assert_eq!(read_again, read);
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
        .read("aanvraag", &object(json!({"root": gram.id})))
        .unwrap();
    assert_eq!(read["bsn"], BSN);

    // A gram given twice is refused, not silently kept twice.
    let twice = [kept.clone(), kept].concat();
    assert!(Cell::in_memory(config, twice, &service, received.date_naive()).is_err());
}

//! A political party submits an application for a subsidy (Wpp art. 102), the
//! Autoriteit decides on it (art. 107), and the cell of the Autoriteit records
//! both as grams in its chronicle. The shape of each gram comes from the law:
//! the application from executing art. 102 (Awb 4:2 and 4:13 hook onto it),
//! the decision from the outputs of art. 107. For the decision, the engine
//! executes art. 107 with what the cell reads back from its chronicle (the
//! lexostatus `aanvraag`) and with what other authorities and the Autoriteit
//! itself supply.
//!
//! Only cells, regulation and a data directory: no channels, forms, processes,
//! synthesis or registers. That corpus is not in this repository, so the test
//! runs only when `CEL_WPP_CORPUS` points at its root (with `cells/` and
//! `regulation/`):
//!
//! ```text
//! CEL_WPP_CORPUS=<root> cargo test -p regelrecht-cel --test wpp_kroniek
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::DateTime;
use http_body_util::BodyExt;
use regelrecht_cel::assessment;
use regelrecht_cel::config::{Config, DEFAULT_PORT};
use regelrecht_cel::regulations;
use regelrecht_cel::runtime::Runtime;
use regelrecht_cel::schema::{self, Kind};
use regelrecht_cel::transport::RUNTIME_TOKEN_HEADER;
use serde_json::{json, Map, Value};
use tower::ServiceExt;

const CELL: &str = "/cells/autoriteit_politieke_partijen";
const AUTHORITY: &str = "nederlandse_autoriteit_politieke_partijen";
const WPP: &str = "wet_op_de_politieke_partijen";

fn corpus() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("CEL_WPP_CORPUS")?);
    assert!(
        root.join("cells").is_dir() && root.join("regulation").is_dir(),
        "CEL_WPP_CORPUS={} has no cells/ and regulation/",
        root.display()
    );
    Some(root)
}

/// A runtime over the corpus whose clock the test sets.
fn runtime(root: &Path, data: &Path, now: &Arc<Mutex<String>>) -> Runtime {
    let config = Config {
        cells_path: root.join("cells"),
        regulation_path: root.join("regulation"),
        data_dir: data.to_path_buf(),
        port: DEFAULT_PORT,
        read_token: None,
        read_token_sources: Vec::new(),
        reduction: Default::default(),
        registers: None,
        channels: None,
        synthesis: None,
        examples: None,
    };
    let now = now.clone();
    let clock = Arc::new(move || DateTime::parse_from_rfc3339(&now.lock().unwrap()).unwrap());
    Runtime::load(&config, clock).unwrap_or_else(|e| panic!("{e:#?}"))
}

async fn as_runtime(
    rt: &Runtime,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header(RUNTIME_TOKEN_HEADER, rt.runtime_token.as_str());
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = rt.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The application of the case: a party with three seats in one municipal
/// council, under its own designation (Wpp 102 lid 3 to 5, Awb 4:2 lid 1).
fn application() -> Value {
    json!({
        "actor": AUTHORITY,
        "stream": "napp_aanvragen",
        "event": "aanvraag_ontvangen",
        "intake": {"channel": "aanvrager"},
        "external": {
            "subsidiejaar": 2027,
            "statutaire_naam": "Vereniging Voorbeeld",
            "geregistreerde_aanduiding": "VOORBEELD",
            "adres_aanvrager": "Voorbeeldstraat 1, 2511 AA Den Haag",
            "dagtekening": "2027-03-04",
            "ondertekening": "A. Voorzitter",
            "zeteltabel": [
                {"orgaan": "gemeenteraad", "gemeentecode": "GM0001", "zetels": 3,
                 "samenstellende_aanduidingen": null}
            ],
        },
    })
}

async fn record_application(rt: &Runtime) -> Value {
    let (status, body) = as_runtime(
        rt,
        "POST",
        &format!("{CELL}/api/grams"),
        Some(application()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body:#}");
    body["gram"].clone()
}

#[tokio::test]
async fn a_party_submits_an_application_and_the_authority_records_it() {
    let Some(root) = corpus() else {
        eprintln!("CEL_WPP_CORPUS not set: skipped");
        return;
    };
    let data = tempfile::tempdir().unwrap();
    let now = Arc::new(Mutex::new("2027-03-05T09:30:00+01:00".to_string()));
    let rt = runtime(&root, data.path(), &now);

    let gram = record_application(&rt).await;
    schema::validate(Kind::Gram, &gram).unwrap();
    assert_eq!(gram["recording_actor"], AUTHORITY);
    assert_eq!(gram["subtype"], "aanvraag");
    assert_eq!(gram["stage"], "AANVRAAG");
    // Awb 4:13 lid 1: the receipt is the moment that counts.
    assert_eq!(gram["effective_at"], "2027-03-05T09:30:00+01:00");
    // The fields are what the executed law asks: Wpp 102 lid 1, 3, 4 and 5
    // and Awb 4:2 lid 1. The name of the applicant (4:2 lid 1 onder a) is the
    // statutory name (102 lid 3 onder a), and the decision asked for (4:2
    // lid 1 onder c) is the one Wpp 107 takes on it: neither is a field of
    // its own for the party.
    assert_eq!(
        gram["fields"],
        json!({
            "subsidiejaar": 2027,
            "statutaire_naam": "Vereniging Voorbeeld",
            "geregistreerde_aanduiding": "VOORBEELD",
            "adres_aanvrager": "Voorbeeldstraat 1, 2511 AA Den Haag",
            "dagtekening": "2027-03-04",
            "gevraagde_beschikking": "wet_op_de_politieke_partijen#107",
            "ondertekening": "A. Voorzitter",
            "zeteltabel": [
                {"orgaan": "gemeenteraad", "gemeentecode": "GM0001", "zetels": 3,
                 "samenstellende_aanduidingen": null}
            ],
        })
    );

    let chronicle = data.path().join("autoriteit_politieke_partijen/napp.jsonl");
    assert_eq!(
        std::fs::read_to_string(chronicle).unwrap().lines().count(),
        1
    );

    // A field the law does not ask is refused.
    let unknown = json!({
        "actor": AUTHORITY, "stream": "napp_aanvragen", "event": "aanvraag_ontvangen",
        "intake": {"channel": "aanvrager"},
        "external": {"subsidiejaar": 2027, "schoenmaat": 44},
    });
    let (status, body) = as_runtime(&rt, "POST", &format!("{CELL}/api/grams"), Some(unknown)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:#}");
}

/// What the Kiesraad knows of the party (Kieswet G 1 to G 3) and of the last
/// election. A stand-in for its cell: fixed values of the case.
fn kiesraad() -> Map<String, Value> {
    let mut m = Map::new();
    for orgaan in [
        "tweedekamer",
        "eerstekamer",
        "europeesparlement",
        "provinciestaten",
        "algemeenbestuur",
    ] {
        m.insert(format!("is_ingeschreven_{orgaan}"), json!(false));
        m.insert(format!("is_geschrapt_{orgaan}"), json!(false));
    }
    m.insert("is_ingeschreven_gemeenteraad".into(), json!(true));
    m.insert("is_geschrapt_gemeenteraad".into(), json!(false));
    for orgaan in ["gemeenteraad", "provinciale_staten", "algemeen_bestuur"] {
        m.insert(format!("doorwerking_geblokkeerd_{orgaan}"), json!(false));
    }
    for (k, v) in [
        ("is_samengevoegde_aanduiding", json!(false)),
        ("zetels_op_lijst_met_eigen_aanduiding", json!(3)),
        ("registratie_grondslag_g1", json!(false)),
        ("registratie_grondslag_g2", json!(false)),
        ("mededeling_gedaan", json!(false)),
        ("dagtekening_mededeling", json!("2026-07-01")),
        ("datum_mededeling_registraties", json!("2026-07-01")),
        ("datum_kandidaatstelling", json!("2026-07-01")),
        ("verkiezingsjaar", json!(2026)),
    ] {
        m.insert(k.into(), v);
    }
    m
}

/// Per row of the seat table, what the Kiesraad (the registration with that
/// body) and the CBS (the population on the reference date of art. 106 lid 1)
/// add to what the party stated. A stand-in for their cells.
fn complete_row(row: &mut Value) {
    row["registratie_bij_dit_orgaan"] = json!(true);
    row["binnen_provincie_van_registratie"] = json!(true);
    row["inwonertal"] = json!(100_001);
}

/// The judgement and the dossier of the Autoriteit at the decision: the
/// decision date, the care of Awb 3:2 and the reasons of 3:46, no request to
/// supplement (4:5), no suspension (4:15), no merger (art. 110).
fn autoriteit(decision_date: &str) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in [
        ("besluitdatum", json!(decision_date)),
        ("relevante_feiten_vergaard", json!(true)),
        ("af_te_wegen_belangen_in_beeld", json!(true)),
        ("betrokken_belangen_afgewogen", json!(true)),
        ("afwegingsruimte_beperkt", json!(true)),
        ("nadelige_gevolgen_onevenredig", json!(false)),
        ("eigen_motivering_gegeven", json!(true)),
        ("verwijst_naar_vaste_gedragslijn", json!(false)),
        ("gedragslijn_is_vastgestelde_beleidsregel", json!(false)),
        ("indiener_in_verzuim", json!(false)),
        ("is_verdwijnende_rechtspersoon", json!(false)),
        ("notarisverklaring_aanwezig", json!(false)),
        ("datum_akte_verleden", Value::Null),
        ("fusiejaar", json!(0)),
        ("fusie_door_oprichting_nieuwe_vereniging", json!(false)),
        ("fusie_door_verkrijging_vermogen", json!(false)),
        ("datum_uitnodiging_aanvulling", Value::Null),
        ("datum_aanvraag_aangevuld", Value::Null),
        ("einddag_hersteltermijn", Value::Null),
        ("uitstel_ingestemd_dagen", Value::Null),
        ("vertraging_toerekenbaar_dagen", Value::Null),
        ("overmacht_dagen", Value::Null),
        ("datum_mededeling_buitenlandse_informatie", Value::Null),
        ("datum_buitenlandse_informatie_ontvangen", Value::Null),
        ("datum_verder_uitstel_niet_redelijk", Value::Null),
    ] {
        m.insert(k.into(), v);
    }
    m
}

/// What art. 107 asks about the announcement (Awb 3:40, 3:41, through 2:8
/// and 2:19): at the decision it has not happened yet, and the law does not
/// require it then (`required: false`).
fn not_yet_announced() -> Map<String, Value> {
    [
        "datum_bekendmaking_beschikking",
        "elektronisch_bekendgemaakt",
        "toegezonden_of_uitgereikt_aan_belanghebbenden",
        "toezending_of_uitreiking_niet_mogelijk",
        "op_andere_geschikte_wijze_bekendgemaakt",
        "geadresseerde_uitdrukkelijk_bereikbaar_verklaard",
        "systeem_met_toegang_voor_geadresseerde",
        "bericht_toegankelijk_voor_geadresseerde",
        "bericht_heeft_extern_systeem_bereikt",
    ]
    .into_iter()
    .map(|k| (k.to_string(), Value::Null))
    .collect()
}

/// A copy of the regulation without `uitvoeringsbeleid/`.
fn law_only(regulation: &Path) -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    // Not a name with a leading dot (tempfile's default): the loader skips
    // those.
    let dir = tempfile::Builder::new().prefix("wet").tempdir().unwrap();
    for entry in std::fs::read_dir(regulation.join("nl")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != "uitvoeringsbeleid" {
            copy(
                &entry.path(),
                &dir.path().join("nl").join(entry.file_name()),
            );
        }
    }
    dir
}

#[tokio::test]
async fn the_authority_decides_on_the_application_and_records_the_decision() {
    let Some(root) = corpus() else {
        eprintln!("CEL_WPP_CORPUS not set: skipped");
        return;
    };
    let data = tempfile::tempdir().unwrap();
    let now = Arc::new(Mutex::new("2027-03-05T09:30:00+01:00".to_string()));
    let rt = runtime(&root, data.path(), &now);
    let application = record_application(&rt).await;
    let id = application["id"].as_str().unwrap().to_string();

    // The decision is taken on 29 June 2027 (before 1 July: art. 107 lid 1).
    *now.lock().unwrap() = "2027-06-29T14:00:00+02:00".to_string();

    // 1. The cell reads the application back from its chronicle, as the
    //    parameters of Wpp 102 that art. 107 reads.
    let (status, read) = as_runtime(
        &rt,
        "GET",
        &format!("{CELL}/api/lexostatus/aanvraag?root={id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{read:#}");
    let from_chronicle = read["parameters"].as_object().unwrap().clone();
    assert_eq!(from_chronicle["subsidiejaar"], 2027);
    assert_eq!(from_chronicle["aanvraagdatum"], "2027-03-05");
    assert_eq!(from_chronicle["bevat_statutaire_naam"], true);
    assert_eq!(from_chronicle["bevat_aantal_zetels"], true);

    // 2. With what other authorities and the Autoriteit itself supply, each
    //    with where it came from.
    let mut parameters: BTreeMap<String, Value> = BTreeMap::new();
    let mut inputs = Map::new();
    let mut put = |source: &Map<String, Value>, provenance: Value| {
        for (k, v) in source {
            parameters.insert(k.clone(), v.clone());
            inputs.insert(k.clone(), json!({"value": v, "provenance": provenance}));
        }
    };
    let mut own = from_chronicle.clone();
    let mut table = own.remove("zeteltabel").unwrap();
    put(&own, json!({"source": "own", "lexostatus": "aanvraag"}));
    for row in table.as_array_mut().unwrap() {
        complete_row(row);
    }
    put(
        &Map::from_iter([("zeteltabel".to_string(), table)]),
        json!({"source": "per_row", "lexostatus": "aanvraag", "field": "zeteltabel"}),
    );
    put(
        &kiesraad(),
        json!({"source": "cell", "cell": "kiesraad", "lexostatus": "registratie", "transport": "vaste waarden in de test"}),
    );
    put(&autoriteit("2027-06-29"), json!({"source": "handler"}));
    // Not given to the engine: at the decision it has not happened, and the
    // law does not require it then (required: false, RFC-036). It is in the
    // inputs, as null with its stage.
    for (k, v) in not_yet_announced() {
        inputs.insert(
            k,
            json!({"value": v, "provenance": {"source": "state_at_decision", "stage": "BEKENDMAKING"}}),
        );
    }

    // 3. The engine executes art. 107 on the law: every output, as the
    //    decision's fields. Without the implementing policy of the
    //    Autoriteit, which hooks onto the decision with how it works (portal,
    //    second reviewer, leniency) and overrides nothing of the law.
    let law = law_only(&root.join("regulation"));
    let corpus = regulations::load(law.path()).unwrap();
    let outputs = [
        "besluitdeadline_art107",
        "directe_vaststelling_art107",
        "besluit_tot_subsidievaststelling_genomen_art107",
        "besluit_tijdig_art107",
        "vastgesteld_subsidiebedrag_art107",
        "beschikking_zorgvuldig_art107",
        "aanvrager_bestaat_op_besluitdatum_art107",
    ];
    let evaluation =
        assessment::evaluate(&corpus.service, WPP, &outputs, &parameters, "2027-06-29");
    assert!(evaluation.error.is_none(), "{evaluation:#?}");
    // Only the timeliness waits, and only for the announcement.
    assert_eq!(
        evaluation.missing_per.keys().collect::<Vec<_>>(),
        ["besluit_tijdig_art107"],
        "{evaluation:#?}"
    );
    let mut waits = evaluation.missing_per["besluit_tijdig_art107"].clone();
    waits.sort();
    let mut announcement: Vec<String> = not_yet_announced().keys().cloned().collect();
    announcement.sort();
    assert_eq!(waits, announcement, "{evaluation:#?}");
    // Art. 106 lid 2 onder d: 100.001 inhabitants is class d, EUR 1.137 per
    // seat; three seats give EUR 3.411 (in cents).
    assert_eq!(
        evaluation.values["vastgesteld_subsidiebedrag_art107"], 341_100,
        "{evaluation:#?}"
    );

    // 4. The cell records the decision, referring to the application.
    let mut external = Map::new();
    external.insert("besluitdatum".into(), json!("2027-06-29"));
    for o in outputs {
        external.insert(
            o.into(),
            evaluation.values.get(o).cloned().unwrap_or(Value::Null),
        );
    }
    let inputs_count = inputs.len();
    let wpp = corpus.regulations.iter().find(|r| r.id == WPP).unwrap();
    let decision = json!({
        "actor": AUTHORITY,
        "stream": "napp_besluiten",
        "event": "subsidie_vastgesteld",
        "intake": {"channel": "autoriteit"},
        "external": external,
        "refers_to": {"on_application": id},
        "decision": {
            "legal_character": "BESCHIKKING",
            "decision_type": "TOEKENNING",
            "regulation": WPP,
            "regulation_valid_from": wpp.valid_from,
            // As Wpp 107 names it (competent_authority).
            "competent_authority": "Nederlandse autoriteit politieke partijen",
            "inputs": inputs,
        },
    });
    let (status, body) = as_runtime(
        &rt,
        "POST",
        &format!("{CELL}/api/grams"),
        Some(decision.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body:#}");
    let gram = &body["gram"];
    schema::validate(Kind::Gram, gram).unwrap();

    assert_eq!(gram["type"], "decretogram");
    assert_eq!(gram["stage"], "BESLUIT");
    assert_eq!(gram["legal_character"], "BESCHIKKING");
    assert_eq!(gram["refers_to"]["on_application"], json!(id));
    // Art. 107 lid 1: the decision date is the moment that counts.
    assert!(
        gram["effective_at"]
            .as_str()
            .unwrap()
            .starts_with("2027-06-29"),
        "{gram:#}"
    );
    assert_eq!(gram["fields"]["vastgesteld_subsidiebedrag_art107"], 341_100);
    assert_eq!(
        gram["fields"]["besluit_tot_subsidievaststelling_genomen_art107"],
        true
    );
    // Whether the decision is in time depends on its announcement (Awb
    // 3:40), which comes after it: unknown at the decision.
    assert_eq!(gram["fields"]["besluit_tijdig_art107"], Value::Null);
    assert_eq!(
        gram["competent_authority"],
        "Nederlandse autoriteit politieke partijen"
    );
    assert_eq!(
        gram["inputs"]["zeteltabel"]["provenance"]["source"],
        "per_row"
    );
    let mut fields: Vec<&str> = gram["fields"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    let mut expected = outputs.to_vec();
    expected.sort_unstable();
    assert_eq!(fields, expected, "the fields are the outputs of art. 107");
    // Every input with where it came from.
    for (input, source) in [
        ("aanvraagdatum", "own"),
        ("zeteltabel", "per_row"),
        ("is_ingeschreven_gemeenteraad", "cell"),
        ("besluitdatum", "handler"),
        ("datum_bekendmaking_beschikking", "state_at_decision"),
    ] {
        assert_eq!(
            gram["inputs"][input]["provenance"]["source"], source,
            "{input}"
        );
    }
    assert_eq!(
        gram["inputs"].as_object().unwrap().len(),
        inputs_count,
        "every input of the run, no more"
    );

    let chronicle = data.path().join("autoriteit_politieke_partijen/napp.jsonl");
    assert_eq!(
        std::fs::read_to_string(chronicle).unwrap().lines().count(),
        2
    );

    // One decision per application.
    let (status, body) =
        as_runtime(&rt, "POST", &format!("{CELL}/api/grams"), Some(decision)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:#}");
}

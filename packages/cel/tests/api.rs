//! The routes, through the real router, on the generic fixtures.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use chrono::DateTime;
use http_body_util::BodyExt;
use regelrecht_cel::api::Clock;
use regelrecht_cel::config::{Config, ReductionMode, DEFAULT_PORT};
use regelrecht_cel::runtime::Runtime;
use regelrecht_cel::schema::{self, Kind};
use regelrecht_cel::transport::{READ_TOKEN_HEADER, RUNTIME_TOKEN_HEADER};
use serde_json::{json, Value};
use tower::ServiceExt;

/// The process with a portal, and the cell it records in.
const AGENCY: &str = "/processes/test_instantie_proces";
const AGENCY_CELL: &str = "/cells/test_instantie";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn clock() -> Clock {
    Arc::new(|| DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap())
}

/// A runtime over `<setup>/cells` and `<setup>/processes`.
fn runtime_at(setup: &Path, data: &Path) -> Result<Runtime, Vec<String>> {
    let config = Config {
        cells_path: setup.join("cells"),
        processes_path: Some(setup.join("processes")),
        regulation_path: fixtures().join("regulation"),
        data_dir: data.to_path_buf(),
        port: DEFAULT_PORT,
        read_token: None,
        read_token_sources: Vec::new(),
        reduction: Default::default(),
        registers: None,
    };
    Runtime::load(&config, clock())
}

/// A runtime like [`runtime_at`], with a read token.
fn runtime_with_read_token(setup: &Path, data: &Path, token: &str, sources: &[&str]) -> Runtime {
    let config = Config {
        cells_path: setup.join("cells"),
        processes_path: Some(setup.join("processes")),
        regulation_path: fixtures().join("regulation"),
        data_dir: data.to_path_buf(),
        port: DEFAULT_PORT,
        read_token: Some(token.to_string()),
        read_token_sources: sources.iter().map(|b| b.to_string()).collect(),
        reduction: Default::default(),
        registers: None,
    };
    Runtime::load(&config, clock()).unwrap()
}

fn app(data: &Path) -> Router {
    as_reader(&runtime_at(&fixtures(), data).unwrap())
}

/// The router of a runtime whose tests read the read routes of a cell the
/// way a process of that runtime does: a `GET` under `/cells/` carries the
/// runtime token. The read routes are not open (see
/// `only_the_runtime_records_and_reads`).
fn as_reader(rt: &Runtime) -> Router {
    let token = rt.runtime_token.as_str().to_string();
    rt.router.clone().layer(axum::middleware::map_request(
        move |mut req: Request<Body>| {
            let token = token.clone();
            async move {
                if req.method() == axum::http::Method::GET
                    && req.uri().path().starts_with("/cells/")
                {
                    req.headers_mut()
                        .insert(RUNTIME_TOKEN_HEADER, token.parse().unwrap());
                }
                req
            }
        },
    ))
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let headers: Vec<(&str, &str)> = cookie.map(|c| ("cookie", c)).into_iter().collect();
    call_with(app, method, uri, &headers, body).await
}

/// A request to a cell the way a process of the runtime makes it: with the
/// runtime token.
async fn as_runtime(rt: &Runtime, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let headers = [(RUNTIME_TOKEN_HEADER, rt.runtime_token.as_str())];
    let (status, body, _) = call_with(&rt.router, method, uri, &headers, Some(body)).await;
    (status, body)
}

async fn call_with(
    app: &Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value, cookie)
}

async fn logins(app: &Router, kvk: &str) -> String {
    let (status, _, cookie) = call(
        app,
        "POST",
        &format!("{AGENCY}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": kvk, "persoon": "A. Tester"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    cookie.unwrap()
}

fn complete() -> Value {
    json!({"external": {
        "naam": "Vereniging Voorbeeld",
        "adres": "Voorbeeldstraat 1, 1234 AB Voorbeeld",
        "dagtekening": "2025-03-12",
        "aanvraagjaar": 2025,
        "aanduiding": "VOORBEELD",
        "registratie": "a",
        "organen": [
            {"orgaan": "raad", "zetels": 10, "samengevoegd": false},
            {"orgaan": "raad", "zetels": 3, "samengevoegd": false}
        ],
        "rekeningnummer": "NL00TEST0123456789"
    }})
}

#[tokio::test]
async fn login_rejects_an_invalid_kvk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "123", "persoon": "A"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("acht cijfers"));
}

#[tokio::test]
async fn form_gives_fields_with_labels() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = call(&app, "GET", &format!("{AGENCY}/api/form"), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cell"], "test_instantie");
    assert_eq!(body["event"], "aanvraag_ontvangen");
    assert_eq!(body["fields"][0]["label"], "Naam van de aanvrager");
    schema::validate(Kind::Stream, &body["stream"]).unwrap();
}

#[tokio::test]
async fn the_cell_gives_its_streams_with_their_hash() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = call(&app, "GET", "/cells/test_afnemer/api/stream", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<&str> = body["streams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["test_afnemer_aanvragen", "test_afnemer_zaakverloop"]);
    for s in body["streams"].as_array().unwrap() {
        assert_eq!(s["sha256"].as_str().unwrap().len(), 64);
        schema::validate(Kind::Stream, &s["stream"]).unwrap();
    }
}

#[tokio::test]
async fn assessment_without_login_is_not_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application/assessment"),
        None,
        Some(complete()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn assessment_complete_and_incomplete_without_recording() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = logins(&app, "12345678").await;

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application/assessment"),
        Some(&c),
        Some(complete()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["result"]["value"], json!(true), "{body}");
    assert_eq!(
        body["lexostatus"]["parameters"]["aanvraagdatum"],
        "2025-03-12"
    );

    let mut incomplete = complete();
    incomplete["external"]
        .as_object_mut()
        .unwrap()
        .remove("aanduiding");
    let (_, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application/assessment"),
        Some(&c),
        Some(incomplete),
    )
    .await;
    assert_eq!(body["result"]["value"], json!(false), "{body}");
    assert_eq!(body["result"]["absent"], json!(["bevat_aanduiding"]));

    let mut without_year = complete();
    without_year["external"]
        .as_object_mut()
        .unwrap()
        .remove("aanvraagjaar");
    let (_, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application/assessment"),
        Some(&c),
        Some(without_year),
    )
    .await;
    assert_eq!(body["result"]["to_assess"], json!(false));
    assert_eq!(
        body["result"]["reason"],
        "cannot be judged: missing aanvraagjaar"
    );

    // An assessment records nothing.
    let (_, chronicle, _) = call(
        &app,
        "GET",
        &format!("{AGENCY_CELL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(chronicle, json!([]));
}

#[tokio::test]
async fn unknown_field_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = logins(&app, "12345678").await;
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application"),
        Some(&c),
        Some(json!({"external": {"schoenmaat": 44}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("schoenmaat"));
}

#[tokio::test]
async fn unknown_table_column_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = logins(&app, "12345678").await;
    let mut body = complete();
    body["external"]["organen"][1]["kleur"] = json!("rood");
    let (status, response, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application"),
        Some(&c),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        response["error"].as_str().unwrap().contains(
            "unknown field 'organen[1].kleur': the event 'aanvraag_ontvangen' does not record it"
        ),
        "{response}"
    );
    // Nothing was recorded.
    let (_, chronicle, _) = call(
        &app,
        "GET",
        &format!("{AGENCY_CELL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(chronicle, json!([]));
}

#[tokio::test]
async fn submitting_records_a_gram_per_kvk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = logins(&app, "12345678").await;

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application"),
        Some(&c),
        Some(complete()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gram = &body["gram"];
    schema::validate(Kind::Gram, gram).unwrap();
    assert_eq!(gram["type"], "submission");
    assert_eq!(gram["subtype"], "aanvraag");
    assert_eq!(
        gram["fields"]["core"]["signed_via"]["kvk_nummer"],
        "12345678"
    );
    let yaml = body["yaml"].as_str().unwrap();
    assert!(
        yaml.starts_with("kind: chronolexogram\nid: ")
            && yaml.contains("\ntype: submission\nsubtype: aanvraag\n"),
        "{yaml}"
    );
    // Fields in the order of the stream: core before content.
    assert!(yaml.find("core:").unwrap() < yaml.find("content:").unwrap());
    let case = gram["id"].as_str().unwrap().to_string();

    // An incomplete application is recorded too, in a new case.
    let mut incomplete = complete();
    incomplete["external"]
        .as_object_mut()
        .unwrap()
        .remove("adres");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application"),
        Some(&c),
        Some(incomplete),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        body["gram"]["fields"]["core"]["aanvrager"]["adres"],
        Value::Null
    );
    assert_ne!(body["gram"]["id"], json!(case));

    // The chronicle and the lexostatuses belong to the cell, without login:
    // between a consumer and the cell there is no security context.
    let (_, chronicle, _) = call(
        &app,
        "GET",
        &format!("{AGENCY_CELL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(chronicle.as_array().unwrap().len(), 2);
    for item in chronicle.as_array().unwrap() {
        schema::validate(Kind::Gram, &item["gram"]).unwrap();
    }
    let rows =
        std::fs::read_to_string(dir.path().join("test_instantie/test_kroniek.jsonl")).unwrap();
    assert_eq!(rows.lines().count(), 2);

    let (status, lexo, _) = call(
        &app,
        "GET",
        &format!("{AGENCY_CELL}/api/lexostatus/aanvraag_inhoud?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lexo}");
    assert_eq!(lexo["parameters"]["bevat_naam"], json!(true));

    // The process has no chronicle and no lexostatus.
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{AGENCY}/api/chronicle"),
        Some(&c),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_lexostatus() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = logins(&app, "12345678").await;
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{AGENCY_CELL}/api/lexostatus/bestaat_niet?root=x"),
        Some(&c),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test]
fn fixtures_validate_against_the_schemas() {
    let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (file, kind) in [
        ("chronicles/test_aanvragen.yaml", Kind::Stream),
        ("chronicles/test_registers.yaml", Kind::Stream),
        ("chronicles/test_afnemer_aanvragen.yaml", Kind::Stream),
        ("cells/instantie/lexostatuses.yaml", Kind::Lexostatus),
        ("cells/register/lexostatuses.yaml", Kind::Lexostatus),
        ("cells/afnemer/lexostatuses.yaml", Kind::Lexostatus),
        ("cells/instantie/cell.yaml", Kind::Cell),
        ("cells/register/cell.yaml", Kind::Cell),
        ("cells/afnemer/cell.yaml", Kind::Cell),
        ("cells/gebieden/cell.yaml", Kind::Cell),
        ("processes/instantie/process.yaml", Kind::Process),
        ("processes/afnemer/process.yaml", Kind::Process),
    ] {
        let doc: Value =
            serde_yaml_ng::from_str(&std::fs::read_to_string(f.join(file)).unwrap()).unwrap();
        let result = schema::validate(kind, &doc);
        assert!(result.is_ok(), "{file}: {result:?}");
    }
}

// --- The runtime: several cells, each with its own chronicle ---

#[tokio::test]
async fn cells_are_listed_with_their_possibilities() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = call(&app, "GET", "/api/cells", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "test_afnemer",
            "test_gebieden",
            "test_instantie",
            "test_register",
            "test_toeslag"
        ]
    );
    let register = &body[3];
    assert_eq!(register["recording_actor"], "test_register");
    assert_eq!(register["lexostatuses"][0]["name"], "register");
    assert_eq!(
        register["lexostatuses"][0]["inputs"][0]["name"],
        "aanduiding"
    );
    assert_eq!(register["lexostatuses"][0]["inputs"][1]["name"], "orgaan");
    assert_eq!(
        body[0]["lexostatuses"][0]["extra_fields"],
        json!(["aanduiding", "gebieden"])
    );
    // A cell says nothing about who acts: that is in the process.
    for key in ["portal", "rollen", "behandeling", "synthese"] {
        assert!(body[0].get(key).is_none(), "{key}");
    }
}

#[tokio::test]
async fn processes_are_listed_with_their_cell() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = call(&app, "GET", "/api/processes", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let consumer = &body[0];
    assert_eq!(consumer["id"], "test_afnemer_proces");
    assert_eq!(consumer["actor"], "test_afnemer");
    assert_eq!(consumer["cell"], "test_afnemer");
    assert_eq!(consumer["portal"], json!(true));
    // The own cell is a source like the others, for the case.
    assert_eq!(
        consumer["synthesis"][0],
        json!({"cell": "test_afnemer", "lexostatus": "aanvraag_inhoud", "case": true, "transport": "internal", "parameters": []})
    );
    assert_eq!(consumer["synthesis"][3]["cell"], "test_register");
    assert_eq!(consumer["synthesis"][3]["transport"], "internal");
    let agency = &body[1];
    assert_eq!(agency["id"], "test_instantie_proces");
    assert_eq!(agency["handling"], Value::Null);
    assert_eq!(
        agency["roles"],
        json!({
            "aanvrager": {"channel": "eherkenning", "routes": ["portal"], "label": "aanvrager"},
            "burger": {"channel": "burger", "routes": ["portal"], "label": "burger"},
            "loket": {"channel": "medewerker", "routes": ["counter"], "label": "Loketmedewerker"},
        })
    );
    assert_eq!(agency["counter"], json!(true));
    assert_eq!(agency["authority"], Value::Null);
    // The channels with their fields: the frontend builds the login screen from them.
    assert_eq!(
        agency["channels"]["burger"]["fields"][0]["check"],
        "elfproef"
    );
    assert_eq!(agency["channels"]["burger"]["owner"], "nummer");
    assert_eq!(consumer["authority"], "Test afnemer");
    assert_eq!(consumer["counter"], json!(false));
}

#[tokio::test]
async fn a_cell_has_no_login_or_application() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    for (method, path) in [
        (
            "POST",
            "/cells/test_register/api/channels/eherkenning/login",
        ),
        ("POST", "/cells/test_register/api/application/assessment"),
        ("POST", "/cells/test_register/api/application"),
        (
            "POST",
            "/cells/test_instantie/api/channels/eherkenning/login",
        ),
        ("GET", "/cells/test_instantie/api/examples"),
        ("GET", "/cells/test_afnemer/api/worklist"),
    ] {
        let (status, _, _) = call(&app, method, path, None, Some(json!({}))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}");
    }
}

#[tokio::test]
async fn initial_state_in_an_empty_chronicle_and_not_again() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, chronicle, _) = call(
        &app,
        "GET",
        "/cells/test_register/api/chronicle",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let grams = chronicle.as_array().unwrap();
    assert_eq!(grams.len(), 4);
    for g in grams {
        assert_eq!(g["gram"]["provenance"], "initial_state");
        // A register decision belongs to no case.
        assert!(g["gram"].get("zaakkenmerk").is_none(), "{g}");
        assert!(!g["yaml"].as_str().unwrap().contains("zaakkenmerk"));
        schema::validate(Kind::Gram, &g["gram"]).unwrap();
    }
    // The chronicle is in the cell's own directory.
    let path = dir.path().join("test_register/test_register.jsonl");
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 4);
    // A second start adds nothing: the chronicle is no longer empty.
    drop(app);
    let _ = self::app(dir.path());
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 4);
    // The other cells have their own, empty chronicle.
    assert!(!dir
        .path()
        .join("test_instantie/test_register.jsonl")
        .exists());
}

#[tokio::test]
async fn lexostatus_with_input_as_query() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, l, _) = call(
        &app,
        "GET",
        "/cells/test_register/api/lexostatus/registerstatus?aanduiding=VOORBEELD",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    // The register cell speaks the language of its own regulation.
    assert_eq!(
        l["parameters"],
        json!({"jaar_van_mededeling": 2024, "zetels_toegewezen": 6,
               "datum_mededeling": "2024-11-01", "geblokkeerd": false})
    );
    let (status, l, _) = call(
        &app,
        "GET",
        "/cells/test_register/api/lexostatus/register?aanduiding=VOORBEELD&orgaan=raad",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(
        l["parameters"],
        json!({"is_ingeschreven_in_register": true, "is_geschrapt": false})
    );
    let (status, l, _) = call(
        &app,
        "GET",
        "/cells/test_register/api/lexostatus/registerstatus",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(l["error"], "input 'aanduiding' is absent");
}

fn consumer_concept(aanduiding: Option<&str>) -> Value {
    let mut external = json!({
        "naam": "Vereniging Voorbeeld",
        "adres": "Voorbeeldstraat 1, 1234 AB Voorbeeld",
        "dagtekening": "2025-03-12",
        "gebieden": [
            {"gebied": "Voorbeeldstad", "zetels": 4},
            {"gebied": "Buurdorp", "zetels": 2},
        ],
    });
    if let Some(a) = aanduiding {
        external["aanduiding"] = json!(a);
    }
    json!({"external": external})
}

async fn consumer_assessment(app: &Router, aanduiding: Option<&str>) -> Value {
    let (status, _, cookie) = call(
        app,
        "POST",
        &format!("{CONSUMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(
        app,
        "POST",
        &format!("{CONSUMER}/api/application/assessment"),
        cookie.as_deref(),
        Some(consumer_concept(aanduiding)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[tokio::test]
async fn internal_synthesis_with_provenance_per_parameter() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let body = consumer_assessment(&app, Some("VOORBEELD")).await;
    assert_eq!(body["result"]["value"], json!(true), "{body}");
    assert_eq!(
        body["provenance"]["bevat_aanduiding"],
        json!({"source": "own", "lexostatus": "aanvraag_inhoud"})
    );
    assert_eq!(
        body["provenance"]["zetels_op_lijst"],
        json!({"source": "cell", "cell": "test_register", "lexostatus": "registerstatus", "transport": "internal"})
    );
    // The consumer asks the register under its own name; the source delivers
    // in the language of its regulation, with the orgaan as a fixed input.
    assert_eq!(body["parameters"]["is_ingeschreven_raad"], json!(true));
    assert_eq!(
        body["provenance"]["is_ingeschreven_raad"],
        json!({"source": "cell", "cell": "test_register", "lexostatus": "register", "transport": "internal"})
    );
    assert!(body["parameters"]
        .get("is_ingeschreven_in_register")
        .is_none());
    assert_eq!(body["sources"][0]["status"], "queried");
    assert_eq!(
        body["sources"][0]["input"],
        json!({"aanduiding": "VOORBEELD", "orgaan": "raad"})
    );
    // The aanduiding is not a parameter: it is kept apart in the own lexostatus.
    assert_eq!(
        body["lexostatus"]["extra_fields"]["aanduiding"],
        "VOORBEELD"
    );
    assert!(body["provenance"].get("aanduiding").is_none());
    // Nothing from the synthesis is recorded, not at the consumer and not at
    // the register.
    assert!(!dir.path().join("test_afnemer/test_afnemer.jsonl").exists());
    let rows = std::fs::read_to_string(dir.path().join("test_register/test_register.jsonl"));
    assert_eq!(rows.unwrap().lines().count(), 4);
}

#[tokio::test]
async fn synthesis_without_registration_and_without_input() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    // An unknown aanduiding: not registered and no seats, so not admissible.
    // There is no notice; nobody fills in that date.
    let body = consumer_assessment(&app, Some("ONBEKEND")).await;
    assert_eq!(body["result"]["value"], json!(false), "{body}");
    assert_eq!(body["provenance"]["is_ingeschreven_raad"]["source"], "cell");
    // What the source did not deliver, under its own name.
    assert_eq!(
        body["sources"][1]["not_delivered"],
        json!(["datum_mededeling", "jaar_van_mededeling"])
    );
    assert!(body["provenance"].get("datum_mededeling").is_none());
    // Without an aanduiding the source is not queried.
    let body = consumer_assessment(&app, None).await;
    assert_eq!(body["sources"][0]["status"], "not_queried");
    assert_eq!(body["result"]["absent"], json!(["bevat_aanduiding"]));
    assert!(body["result"]["reason"]
        .as_str()
        .unwrap()
        .starts_with("cannot be judged: source test_register not queried"));
}

/// An adjustment to a `cell.yaml` or `process.yaml`.
type Adjustment<'a> = (&'a str, &'a dyn Fn(String) -> String);

/// Copy fixture cells, fixture processes and all streams to a setup of its
/// own (`cells/`, `processes/`, `chronicles/`), with an adjustment to each
/// `cell.yaml` and `process.yaml`.
fn own_setup(cells: &[Adjustment], processes: &[Adjustment]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let f = fixtures();
    std::fs::create_dir_all(dir.path().join("chronicles")).unwrap();
    std::fs::create_dir_all(dir.path().join("processes")).unwrap();
    for e in std::fs::read_dir(f.join("chronicles")).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(
            &p,
            dir.path().join("chronicles").join(p.file_name().unwrap()),
        )
        .unwrap();
    }
    for (kind, list, file) in [
        ("cells", cells, "cell.yaml"),
        ("processes", processes, "process.yaml"),
    ] {
        for (name, adjust) in list {
            let target = dir.path().join(kind).join(name);
            std::fs::create_dir_all(&target).unwrap();
            for e in std::fs::read_dir(f.join(kind).join(name)).unwrap() {
                let p = e.unwrap().path();
                let text = std::fs::read_to_string(&p).unwrap();
                let text = if p.file_name().unwrap() == file {
                    adjust(text)
                } else {
                    text
                };
                std::fs::write(target.join(p.file_name().unwrap()), text).unwrap();
            }
        }
    }
    dir
}

fn as_is(t: String) -> String {
    t
}

fn with_url(url: String) -> impl Fn(String) -> String {
    move |t: String| {
        // Only the synthesis sources, not the source under rows.
        t.replace(
            "  - cell: test_register\n    lexostatus: registerstatus\n",
            &format!("  - cell: test_register\n    url: {url}\n    lexostatus: registerstatus\n"),
        )
        .replace(
            "  - cell: test_register\n    lexostatus: register\n",
            &format!("  - cell: test_register\n    url: {url}\n    lexostatus: register\n"),
        )
    }
}

#[tokio::test]
async fn synthesis_over_http_to_another_runtime() {
    // Runtime B: the fixtures, on a real port. A reads B with the shared read
    // token; without it only B itself reads.
    let token = "gedeeld-leestoken-van-de-test";
    let data_b = tempfile::tempdir().unwrap();
    let b = runtime_with_read_token(&fixtures(), data_b.path(), token, &[]).router;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, b).await });

    // Runtime A: only the consumer, with a url to B.
    let adjustment = with_url(format!("http://{address}"));
    let setup = own_setup(&[("afnemer", &as_is)], &[("afnemer", &adjustment)]);
    let data_a = tempfile::tempdir().unwrap();
    // Without B among the read-token sources, B does not get it.
    let data_z = tempfile::tempdir().unwrap();
    let without = runtime_with_read_token(setup.path(), data_z.path(), token, &[]);
    let body = consumer_assessment(&without.router, Some("VOORBEELD")).await;
    assert_ne!(body["result"]["value"], json!(true), "{body}");
    let a = runtime_with_read_token(
        setup.path(),
        data_a.path(),
        token,
        &[&format!("http://{address}")],
    );
    // What the runtime cannot see of a source outside it, it reports (the
    // provenance, RFC-043); nothing else.
    let w = a.warnings().await;
    assert_eq!(w.len(), 2, "{w:?}");
    for w in &w {
        assert!(
            w.starts_with("process 'test_afnemer_proces': origin of ")
                && w.contains(&format!("runs outside this runtime (http://{address})")),
            "{w:?}"
        );
    }
    let body = consumer_assessment(&a.router, Some("VOORBEELD")).await;
    assert_eq!(body["result"]["value"], json!(true), "{body}");
    assert_eq!(
        body["provenance"]["is_ingeschreven_raad"]["transport"],
        "http"
    );
    assert_eq!(body["sources"][0]["transport"], "http");
}

#[tokio::test]
async fn unreachable_source_makes_the_assessment_not_judgeable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let adjustment = with_url(format!("http://{address}"));
    let setup = own_setup(&[("afnemer", &as_is)], &[("afnemer", &adjustment)]);
    let data = tempfile::tempdir().unwrap();
    // The source may come later: the runtime does start, with a warning.
    let a = runtime_at(setup.path(), data.path()).unwrap();
    let w = a.warnings().await;
    assert!(
        w.iter().any(|w| w.contains("cannot be checked now")),
        "{w:?}"
    );
    let body = consumer_assessment(&a.router, Some("VOORBEELD")).await;
    assert_eq!(body["result"]["to_assess"], json!(false));
    assert_eq!(
        body["result"]["reason"],
        "cannot be judged: source test_register unreachable"
    );
    assert_eq!(body["sources"][0]["status"], "unreachable");
    // Nothing was filled in.
    assert!(body["provenance"].get("is_ingeschreven_raad").is_none());
}

#[tokio::test]
async fn internal_source_that_does_not_run_is_a_warning() {
    let setup = own_setup(&[("afnemer", &as_is)], &[("afnemer", &as_is)]);
    let data = tempfile::tempdir().unwrap();
    let a = runtime_at(setup.path(), data.path()).unwrap();
    let w = a.warnings().await;
    assert!(
        w.iter()
            .any(|w| w.contains("does not run in this runtime and has no url")),
        "{w:?}"
    );
    assert!(
        w.iter().any(|w| w.contains("no cell 'test_register'")),
        "{w:?}"
    );
}

#[tokio::test]
async fn source_without_the_expected_parameter_is_a_warning() {
    // A parameter the consumer expects but the source does not deliver:
    // remove it from the source's lexostatus.
    let cells = own_setup(
        &[("afnemer", &as_is), ("register", &as_is)],
        &[("afnemer", &as_is)],
    );
    let lexo = cells.path().join("cells/register/lexostatuses.yaml");
    let text = std::fs::read_to_string(&lexo).unwrap();
    std::fs::write(
        &lexo,
        text.replace(
            "        is_geschrapt:\n          filter: {name: aanduiding_geschrapt, orgaan: $orgaan, aanduiding: $aanduiding}\n          exists: true\n          legal_basis: [testregeling_register#1 lid 1]\n",
            "",
        ),
    )
    .unwrap();
    // Then nobody reads `aanduiding_geschrapt` any more: mark the event.
    let stream = cells.path().join("chronicles/test_registers.yaml");
    let text = std::fs::read_to_string(&stream).unwrap();
    std::fs::write(
        &stream,
        text.replace(
            "      orgaan: $external.orgaan\n  - name: uitslag_vastgesteld",
            "      orgaan: $external.orgaan\n    not_reduced:\n      - {field: aanduiding, reason: test}\n      - {field: orgaan, reason: test}\n  - name: uitslag_vastgesteld",
        ),
    )
    .unwrap();
    let data = tempfile::tempdir().unwrap();
    let a = runtime_at(cells.path(), data.path()).unwrap();
    let w = a.warnings().await;
    assert!(
        w.iter()
            .any(|w| w.contains("the source delivers no parameter 'is_geschrapt'")),
        "{w:?}"
    );
}

#[test]
fn synthesis_check_at_startup() {
    let case = |adjustment: &dyn Fn(String) -> String, expected: &str| {
        let setup = own_setup(
            &[
                ("afnemer", &as_is),
                ("register", &as_is),
                ("gebieden", &as_is),
            ],
            &[("afnemer", adjustment)],
        );
        let data = tempfile::tempdir().unwrap();
        let errors = runtime_at(setup.path(), data.path())
            .map(|_| ())
            .expect_err(expected);
        assert!(
            errors
                .iter()
                .any(|f| f.starts_with("process 'test_afnemer_proces': ") && f.contains(expected)),
            "expected '{expected}' in {errors:?}"
        );
    };
    // A parameter that does not fall under the assessment.
    case(
        &|t: String| {
            t.replace(
                "{is_ingeschreven_in_register: is_ingeschreven_raad,",
                "{uitslag_openbaar: uitslag_openbaar, is_ingeschreven_in_register: is_ingeschreven_raad,",
            )
        },
        "'uitslag_openbaar' is not a parameter of testregeling_afnemer#1",
    );
    // A parameter from two sources: the own reduction and the synthesis.
    case(
        &|t: String| {
            t.replace(
                "{is_ingeschreven_in_register: is_ingeschreven_raad,",
                "{bevat_aanduiding: bevat_aanduiding, is_ingeschreven_in_register: is_ingeschreven_raad,",
            )
        },
        "parameter 'bevat_aanduiding' comes from more than one source",
    );
    // An input from a field the assessment lexostatus does not deliver.
    case(
        &|t: String| t.replace("field: aanduiding}", "field: aanduiding_x}"),
        "does not deliver 'aanduiding_x'",
    );
    // Synthesis without a portal and without a decision.
    case(
        &|t: String| {
            let (before, after) = t.split_once("roles:").unwrap();
            let (_, synthesis) = after.split_once("synthesis:").unwrap();
            let (synthesis, _) = synthesis.split_once("handling:").unwrap();
            format!("{before}synthesis:{synthesis}")
        },
        "synthesis without portal and without actions",
    );
}

#[test]
fn decision_check_at_startup() {
    let case = |adjustment: &dyn Fn(String) -> String, expected: &str| {
        let setup = own_setup(
            &[
                ("afnemer", &as_is),
                ("register", &as_is),
                ("gebieden", &as_is),
            ],
            &[("afnemer", adjustment)],
        );
        let data = tempfile::tempdir().unwrap();
        let errors = runtime_at(setup.path(), data.path())
            .map(|_| ())
            .expect_err(expected);
        assert!(
            errors
                .iter()
                .any(|f| f.starts_with("process 'test_afnemer_proces': ") && f.contains(expected)),
            "expected '{expected}' in {errors:?}"
        );
    };
    case(
        &|t: String| t.replace("routes: [handling]", "routes: [counter]"),
        "handling without a role that may use it",
    );
    case(
        &|t: String| {
            t.replace(
                "  aanvrager: {channel: eherkenning, routes: [portal]}\n",
                "",
            )
        },
        "portal without a role that may use it",
    );
    case(
        &|t: String| {
            t.replace(
                "{channel: medewerker, routes: [handling]",
                "{channel: balie, routes: [handling]",
            )
        },
        "role 'behandelaar': channel 'balie' is not listed under channels",
    );
    // The portal event does not bind effective_at to $intake: no counter.
    case(
        &|t: String| t.replace("routes: [handling]", "routes: [handling, counter]"),
        "counter: event 'aanvraag_ontvangen' does not bind effective_at to $intake",
    );
    // On behalf of an authority the law does not know, or no authority for a decision.
    case(
        &|t: String| {
            t.replace(
                "on_behalf_of: {regulation: testregeling_afnemer}",
                "on_behalf_of: {authority: test_afnemer}",
            )
        },
        "on_behalf_of: no loaded regulation names 'test_afnemer' as competent authority",
    );
    case(
        &|t: String| t.replace("on_behalf_of: {regulation: testregeling_afnemer}\n", ""),
        "handling without on_behalf_of",
    );
    case(
        &|t: String| {
            t.replace(
                "on_behalf_of: {regulation: testregeling_afnemer}\n",
                "on_behalf_of: {regulation: testregeling_afnemer}\nmandates:\n  - {authority: Test afnemer, legal_basis: 'testregeling_afnemer#9'}\n",
            )
        },
        "mandate: 'Test afnemer' is the authority the process itself acts for",
    );
    case(
        &|t: String| t.replace("lexostatus: werkvoorraad}", "lexostatus: aanvraag_inhoud}"),
        "worklist 'aanvraag_inhoud' is not a list",
    );
    case(
        &|t: String| t.replace("outputs: [vastgesteld_bedrag,", "outputs: [bestaat_niet,"),
        "has no output 'bestaat_niet'",
    );
    case(
        &|t: String| {
            t.replace(
                "lexostatus: zaakverloop, case: true}",
                "lexostatus: werkvoorraad, case: true}",
            )
        },
        "lexostatus 'werkvoorraad' is a list",
    );
    // A source of the case in a cell other than the process's.
    case(
        &|t: String| {
            t.replace(
                "{cell: test_afnemer, lexostatus: zaakverloop, case: true}",
                "{cell: test_register, lexostatus: zaakverloop, case: true}",
            )
        },
        "cell 'test_register', and the process records in cell 'test_afnemer'",
    );
    // An ordinary source from the own cell: that is a source of the case.
    case(
        &|t: String| {
            t.replace(
                "  - cell: test_register\n    lexostatus: registerstatus\n",
                "  - cell: test_afnemer\n    lexostatus: registerstatus\n",
            )
        },
        "is a source of the case (case: true)",
    );
    // The state at decision is no longer in the configuration: it follows from
    // the procedure of the beschikking. The schema rejects it.
    let with_state = |t: String| {
        t.replacen(
            "      record: {cell: test_afnemer, stream: test_afnemer_zaakverloop, event: besluit_genomen}",
            "      stand_bij_besluit: {bekendgemaakt: false}\n      record: {cell: test_afnemer, stream: test_afnemer_zaakverloop, event: besluit_genomen}",
            1,
        )
    };
    let setup = own_setup(
        &[
            ("afnemer", &as_is),
            ("register", &as_is),
            ("gebieden", &as_is),
        ],
        &[("afnemer", &with_state)],
    );
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert!(
        errors
            .iter()
            .any(|f| f.contains("'stand_bij_besluit' was unexpected")),
        "{errors:?}"
    );
    // Synthesis per row.
    case(
        &|t: String| {
            t.replace(
                "          table: {lexostatus: aanvraag_inhoud, field: gebieden}",
                "          table: {lexostatus: aanvraag_inhoud, field: dorpen}",
            )
        },
        "lexostatus 'aanvraag_inhoud' does not deliver 'dorpen'",
    );
    case(
        &|t: String| t.replace("          table: {lexostatus: aanvraag_inhoud, field: gebieden}", "          table: {lexostatus: werkvoorraad, field: gebieden}"),
        "the table comes from lexostatus 'werkvoorraad', and that is not a lexostatus of the case (case: true)",
    );
    case(
        &|t: String| {
            t.replace(
                "                gebied: {column: gebied}\n                peildatum:",
                "                gebied: {column: gebiedje}\n                peildatum:",
            )
        },
        "column 'gebiedje' is not filled by anything before it",
    );
    case(
        &|t: String| t.replace("- parameter: gebiedstabel", "- parameter: dorpstabel"),
        "'besluit', rows: 'dorpstabel' is not a parameter of testregeling_afnemer#3",
    );
    case(
        &|t: String| {
            t.replace(
                "              columns: {tarief: tarief}",
                "              columns: {tarief: zetels}",
            )
        },
        "column 'zetels' comes from more than one place: the table, source test_gebieden/tarief",
    );
    // Where the decision is recorded: every field of the event is an output
    // or a verdict.
    case(
        &|t: String| t.replace("      outputs: [vastgesteld_bedrag, gebiedsbedrag,", "      outputs: [vastgesteld_bedrag,"),
        "the event records [gebiedsbedrag], and that is neither an output nor a verdict of the decision",
    );
}

// --- The handler: worklist, case and trial decision ---

const CONSUMER: &str = "/processes/test_afnemer_proces";
const CONSUMER_CELL: &str = "/cells/test_afnemer";

async fn consumer_submit(app: &Router, kvk: &str) -> String {
    let (_, _, cookie) = call(
        app,
        "POST",
        &format!("{CONSUMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": kvk, "persoon": "A. Tester"})),
    )
    .await;
    let (status, body, _) = call(
        app,
        "POST",
        &format!("{CONSUMER}/api/application"),
        cookie.as_deref(),
        Some(consumer_concept(Some("VOORBEELD"))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // The application opens the case in stage AANVRAAG (RFC-008); the worklist
    // leaves out only a case with stage BESLUIT.
    assert_eq!(body["gram"]["stage"], "AANVRAAG");
    body["gram"]["id"].as_str().unwrap().to_string()
}

async fn handler(app: &Router) -> String {
    handler_in(app, CONSUMER).await
}

/// Log in as handler of a process.
async fn handler_in(app: &Router, process: &str) -> String {
    let (status, body, cookie) = call(
        app,
        "POST",
        &format!("{process}/api/channels/medewerker/login"),
        None,
        Some(json!({"naam": "B. Behandelaar"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["fields"]["naam"], "B. Behandelaar");
    assert_eq!(body["role"], "behandelaar");
    cookie.unwrap()
}

/// Add a gram to the consumer's chronicle, as recording step by step will do
/// later. Via the runtime's chronicle: it keeps the grams in memory, so a
/// line someone else writes to the file is not seen by it.
/// Record a case-progress gram on the test's clock.
fn add_gram_to(rt: &Runtime, name: &str, case: &str, fields: Value) {
    add_gram_to_at(rt, name, case, fields, "2025-03-12T10:14:03+01:00");
}

/// Record a case-progress gram that legally applies at `effective_at`,
/// recorded on the test's clock.
fn add_gram_to_at(rt: &Runtime, name: &str, case: &str, fields: Value, effective_at: &str) {
    let (type_, stage) = if name == "besluit_genomen" {
        ("decretogram", Some("BESLUIT"))
    } else {
        ("executogram", None)
    };
    let mut gram = json!({
        "kind": "chronolexogram", "type": type_, "name": name,
        "chronicle": "test_afnemer", "recording_actor": "test_afnemer",
        "legal_basis": ["testregeling_afnemer#3"], "effective_at": effective_at,
        "recorded_at": "2025-03-12T10:14:03+01:00",
        "id": uuid::Uuid::now_v7().to_string(),
        "refers_to": {(if stage.is_some() { "op_aanvraag" } else { "aanvraag" }): case},
        "stream": {"id": "test_afnemer_zaakverloop", "sha256": "0".repeat(64)},
        "fields": fields,
    });
    if let Some(s) = stage {
        gram["stage"] = json!(s);
    }
    schema::validate(Kind::Gram, &gram).unwrap();
    rt.cells
        .iter()
        .find(|c| c.cell.id() == "test_afnemer")
        .unwrap()
        .chronicle
        .add(&serde_json::from_value(gram).unwrap())
        .unwrap();
}

#[tokio::test]
async fn roles_decide_who_may_do_what() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let a = consumer_submit(&app, "12345678").await;
    let _two = consumer_submit(&app, "87654321").await;

    // Without login: nothing.
    let (status, _, _) = call(&app, "GET", &format!("{CONSUMER}/api/worklist"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // The applicant: their own grams, no worklist and no case.
    let (_, _, aanvrager) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A"})),
    )
    .await;
    let aanvrager = aanvrager.as_deref();
    for (method, path) in [
        ("GET", format!("{CONSUMER}/api/worklist")),
        ("GET", format!("{CONSUMER}/api/cases/{a}")),
        (
            "POST",
            format!("{CONSUMER}/api/cases/{a}/actions/besluit/trial"),
        ),
        ("GET", format!("{CONSUMER}/api/channels/medewerker/session")),
    ] {
        let (status, body, _) = call(&app, method, &path, aanvrager, Some(json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
    }
    // The chronicle belongs to the cell, without login and without roles: the
    // cell knows no applicant and no handler.
    let (status, chronicle, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER_CELL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(chronicle.as_array().unwrap().len(), 2);

    // The handler: not the portal.
    let b = handler(&app).await;
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/application/assessment"),
        Some(&b),
        Some(consumer_concept(Some("VOORBEELD"))),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/channels/eherkenning/session"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // An empty name is no employee; a process without handling has no
    // worklist, and a channel it does not name does not exist.
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/channels/medewerker/login"),
        None,
        Some(json!({"naam": " "})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    for path in ["/api/channels/bestaat_niet/login", "/api/worklist"] {
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("{AGENCY}{path}"),
            None,
            Some(json!({})),
        )
        .await;
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED,
            "{path}: {status}"
        );
    }
}

#[tokio::test]
async fn worklist_is_a_list_of_cases_without_a_decision() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), data.path()).unwrap();
    let app = as_reader(&rt);
    let a = consumer_submit(&app, "12345678").await;
    let two = consumer_submit(&app, "87654321").await;
    let b = handler(&app).await;

    let (status, w, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    assert_eq!(w["name"], "werkvoorraad");
    assert_eq!(w["parameters"], json!({}), "a list has no parameters");
    let list = w["list"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    let row = list.iter().find(|r| r["root"] == a.as_str()).unwrap();
    assert_eq!(
        row["fields"],
        json!({"ontvangen_op": "2025-03-12", "aanvrager": "Vereniging Voorbeeld", "kvk": "12345678"})
    );

    // A case-progress gram leaves the case in place; a decision takes it off.
    add_gram_to(&rt, "termijn_opgeschort", &a, json!({"dagen": 5}));
    let (_, w, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(w["list"].as_array().unwrap().len(), 2);
    add_gram_to(
        &rt,
        "besluit_genomen",
        &a,
        json!({"vastgesteld_bedrag": 6000, "gebiedsbedrag": 5000, "besluit_tijdig": false,
               "besluitdeadline": "2025-04-11", "zorgvuldig": true}),
    );
    let (_, w, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    let list = w["list"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["root"], two.as_str());

    // The cell list calls the worklist a list, with columns; the process list
    // says who sees it.
    let (_, cells, _) = call(&app, "GET", "/api/cells", None, None).await;
    let worklist = cells[0]["lexostatuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "werkvoorraad")
        .unwrap();
    assert_eq!(worklist["list"], json!(true));
    assert_eq!(worklist["parameters"], json!([]));
    let (_, processes, _) = call(&app, "GET", "/api/processes", None, None).await;
    assert_eq!(
        processes[0]["roles"]["behandelaar"],
        json!({"channel": "medewerker", "routes": ["handling"], "label": "Behandelaar"})
    );
    assert_eq!(processes[0]["handling"]["worklist"], "werkvoorraad");
}

#[tokio::test]
async fn case_with_trial_decision_without_recording() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), data.path()).unwrap();
    let app = as_reader(&rt);
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let chronicle = data.path().join("test_afnemer/test_afnemer.jsonl");
    let before = std::fs::read_to_string(&chronicle).unwrap();

    // Without verdicts: not takeable, and the trial decision says what is missing.
    let (status, z, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{z}");
    assert_eq!(z["grams"].as_array().unwrap().len(), 1);
    // The actions of the process, in the order of process.yaml; the decision
    // first.
    let names: Vec<&str> = z["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["besluit", "bekendmaken", "betalen", "aanvulling_vragen"]
    );
    let decision = &z["actions"][0];
    assert_eq!(decision["article"], "testregeling_afnemer#3");
    assert_eq!(decision["kind"], json!({"kind": "decision"}));
    assert_eq!(decision["stage"], "BESLUIT");
    assert_eq!(decision["available"], json!(true));
    assert_eq!(
        decision["form"],
        json!([
            {"name": "besluitdatum", "label": "Besluitdatum", "type": "date", "group": "Testregeling afnemer, artikel 3", "kind": "verdict"},
            {"name": "feiten_vergaard", "label": "De relevante feiten zijn vergaard", "type": "yes_no", "group": "Testregeling afnemer, artikel 3", "kind": "verdict"}
        ])
    );
    let p = &decision["trial"];
    assert_eq!(p["takeable"], json!(false), "{p}");
    assert!(p.get("outputs").is_none());
    assert!(
        p["reason"]
            .as_str()
            .unwrap()
            .starts_with("not takeable: missing "),
        "{p}"
    );
    let not: Vec<&str> = p["not_delivered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert_eq!(not, ["besluitdatum", "feiten_vergaard"]);

    // With verdicts: takeable, with provenance per parameter.
    let (status, p, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(json!({"form": {"besluitdatum": "2025-03-12", "feiten_vergaard": true}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["takeable"], json!(true), "{p}");
    assert_eq!(p["outputs"]["vastgesteld_bedrag"], json!(6000));
    assert_eq!(p["outputs"]["besluit_tijdig"], json!(false));
    assert_eq!(p["outputs"]["besluitdeadline"], json!("2025-04-11"));
    assert_eq!(p["outputs"]["zorgvuldig"], json!(true));
    assert_eq!(
        p["provenance"]["besluitdatum"],
        json!({"source": "handler"})
    );
    // The announcement comes in a later stage of the procedure: at the
    // decision it has not happened yet. The day is read by the lexostatus
    // besluit (no gram: empty); the rest follows from the procedure.
    assert_eq!(
        p["provenance"]["datum_bekendmaking"],
        json!({"source": "own", "lexostatus": "besluit"})
    );
    assert_eq!(p["parameters"]["datum_bekendmaking"], Value::Null);
    assert_eq!(
        p["provenance"]["bekendgemaakt"],
        json!({"source": "state_at_decision", "stage": "BEKENDMAKING"})
    );
    assert_eq!(p["parameters"]["bekendgemaakt"], json!(false));
    // The reference date is the besluitdatum: the effective_at the event binds
    // to the form.
    assert_eq!(p["reference_date"], "2025-03-12");
    assert!(
        p["reference_date_from"]
            .as_str()
            .unwrap()
            .starts_with("besluitdatum"),
        "{p}"
    );
    assert_eq!(
        p["provenance"]["opgeschorte_dagen"],
        json!({"source": "own", "lexostatus": "zaakverloop"})
    );
    assert_eq!(
        p["provenance"]["aanvraagdatum"],
        json!({"source": "own", "lexostatus": "aanvraag_inhoud"})
    );
    assert_eq!(p["provenance"]["zetels_op_lijst"]["cell"], "test_register");
    // No supplement requested: the reduction reads the absence as null.
    assert_eq!(p["parameters"]["datum_uitnodiging_aanvulling"], Value::Null);
    assert_eq!(p["parameters"]["opgeschorte_dagen"], json!(0));
    assert_eq!(p["not_delivered"], json!([]));

    // A suspension in the case moves the deadline.
    add_gram_to(&rt, "termijn_opgeschort", &case, json!({"dagen": 5}));
    let (_, p, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(json!({"form": {"besluitdatum": "2025-03-12", "feiten_vergaard": true}})),
    )
    .await;
    assert_eq!(p["outputs"]["besluitdeadline"], json!("2025-04-16"));

    // A suspension that only starts after the reference date does not count
    // for this decision: the cell reduces at the decision's reference date.
    add_gram_to_at(
        &rt,
        "termijn_opgeschort",
        &case,
        json!({"dagen": 30}),
        "2025-04-01T09:00:00+02:00",
    );
    let (_, p, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(json!({"form": {"besluitdatum": "2025-03-12", "feiten_vergaard": true}})),
    )
    .await;
    assert_eq!(p["outputs"]["besluitdeadline"], json!("2025-04-16"));
    assert_eq!(p["lexostatuses"][1]["as_of"], json!("2025-03-12"), "{p}");

    // Nothing was recorded, except the case-progress grams of this test.
    let after = std::fs::read_to_string(&chronicle).unwrap();
    assert_eq!(after.lines().count(), before.lines().count() + 2);

    // A verdict the form does not know, and an unknown case.
    let (status, f, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(json!({"form": {"bekendgemaakt": true}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(f["error"].as_str().unwrap().contains("'bekendgemaakt'"));
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/cases/00000000-0000-4000-8000-000000000009"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// A gram without a reference is its own root; the cell gives the id.
/// For an event with a reference, the portal only lets it refer to a gram
/// whose group the applicant knows, and the cell checks that the gram
/// exists.
#[tokio::test]
async fn a_root_and_a_reference() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let c = logins(&app, "12345678").await;
    let application = format!("{AGENCY}/api/application");
    // A reference the event does not have is rejected by the cell.
    let mut with = complete();
    with["refers_to"] = json!({"vorige": "00000000-0000-4000-8000-000000000009"});
    let (status, body, _) = call(&app, "POST", &application, Some(&c), Some(with)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body, _) = call(&app, "POST", &application, Some(&c), Some(complete())).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body["gram"].get("refers_to").is_none());
    let case = body["gram"]["id"].as_str().unwrap().to_string();
    drop(app);

    // The same chronicle, now with a stream in which the application may
    // refer to an earlier application.
    let setup = own_setup(&[("instantie", &as_is)], &[("instantie", &as_is)]);
    let stream = setup.path().join("chronicles/test_aanvragen.yaml");
    let text = std::fs::read_to_string(&stream).unwrap();
    std::fs::write(
        &stream,
        text.replace(
            "  - name: aanvraag_ontvangen\n",
            "  - name: aanvraag_ontvangen\n    refers_to: {vorige: {to: aanvraag_ontvangen}}\n",
        ),
    )
    .unwrap();
    let app = runtime_at(setup.path(), data.path()).unwrap().router;
    let c = logins(&app, "12345678").await;
    let mut unknown = complete();
    unknown["refers_to"] = json!({"vorige": "00000000-0000-4000-8000-000000000009"});
    let (status, body, _) = call(&app, "POST", &application, Some(&c), Some(unknown)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["error"].as_str().unwrap().contains("no root"),
        "{body}"
    );
    let mut follows = complete();
    follows["refers_to"] = json!({"vorige": case});
    // Another KvK does not know this group: the process knows that, not the cell.
    let other = logins(&app, "87654321").await;
    let (status, body, _) = call(
        &app,
        "POST",
        &application,
        Some(&other),
        Some(follows.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["error"].as_str().unwrap().contains("no root"),
        "{body}"
    );
    let (status, body, _) = call(&app, "POST", &application, Some(&c), Some(follows)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["refers_to"]["vorige"], json!(case));
    assert_ne!(body["gram"]["id"], json!(case));
    schema::validate(Kind::Gram, &body["gram"]).unwrap();
}

#[test]
fn a_failing_cell_stops_the_runtime() {
    let broken = |t: String| t.replace("lexostatuses: lexostatuses.yaml", "lexostatuses: weg.yaml");
    let setup = own_setup(
        &[("instantie", &as_is), ("register", &broken)],
        &[("instantie", &as_is)],
    );
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].starts_with("cell 'test_register': "),
        "{errors:?}"
    );
}

#[test]
fn a_failing_process_stops_the_runtime() {
    let broken = |t: String| t.replace("actor: test_instantie", "actor: iemand_anders");
    let setup = own_setup(&[("instantie", &as_is)], &[("instantie", &broken)]);
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].starts_with("process 'test_instantie_proces': portal: stream 'test_aanvragen' has recording_actor 'test_instantie'"),
        "{errors:?}"
    );
}

#[test]
fn without_processes_only_the_cells_run() {
    let data = tempfile::tempdir().unwrap();
    let config = Config {
        cells_path: fixtures().join("cells"),
        processes_path: None,
        regulation_path: fixtures().join("regulation"),
        data_dir: data.path().to_path_buf(),
        port: DEFAULT_PORT,
        read_token: None,
        read_token_sources: Vec::new(),
        reduction: Default::default(),
        registers: None,
    };
    let r = Runtime::load(&config, clock()).unwrap();
    assert_eq!(r.cells.len(), 5);
    assert!(r.processes.is_empty());
}

// --- Synthesis per row and recording the decision ---

/// The consumer's decision form, fully filled in.
fn verdicts() -> Value {
    json!({"form": {"besluitdatum": "2025-03-12", "feiten_vergaard": true}})
}

#[tokio::test]
async fn synthesis_per_row_fills_in_the_table() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let (status, p, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");

    // The rows come from the application; the columns `ingeschreven` and
    // `tarief` from two other cells, queried per row.
    assert_eq!(
        p["parameters"]["gebiedstabel"],
        json!([
            {"gebied": "Voorbeeldstad", "zetels": 4, "ingeschreven": true, "tarief": 1000},
            {"gebied": "Buurdorp", "zetels": 2, "ingeschreven": false, "tarief": 500},
        ])
    );
    // 4 x 1000 + 2 x 500.
    assert_eq!(p["outputs"]["gebiedsbedrag"], json!(5000));
    assert_eq!(
        p["provenance"]["gebiedstabel"],
        json!({"source": "per_row", "lexostatus": "aanvraag_inhoud", "field": "gebieden"})
    );
    let rows = &p["rows"][0];
    assert_eq!(rows["parameter"], "gebiedstabel");
    assert_eq!(rows["sources"][0]["cell"], "test_register");
    assert_eq!(rows["sources"][0]["queried"], json!(2));
    assert_eq!(rows["sources"][1]["cell"], "test_gebieden");
    assert_eq!(rows["sources"][1]["status"], "queried");
    assert!(rows.get("missing").is_none(), "{rows}");

    // The reference date is the year from the register cell (jaar_van),
    // converted to January 1 of that year.
    assert_eq!(p["parameters"]["jaar"], json!(2024));
    assert_eq!(p["provenance"]["jaar"]["cell"], "test_register");
}

#[tokio::test]
async fn an_area_without_a_rate_stays_empty() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    // An area for which no rate has been set: that column stays out, and
    // then the engine cannot give the output.
    let (_, _, cookie) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    let mut concept = consumer_concept(Some("VOORBEELD"));
    concept["external"]["gebieden"] = json!([{"gebied": "Onbekendstad", "zetels": 1}]);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/application"),
        cookie.as_deref(),
        Some(concept),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let case = body["gram"]["id"].as_str().unwrap().to_string();
    let b = handler(&app).await;
    let (_, p, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(p["rows"][0]["missing"], json!(["tarief"]));
    assert_eq!(
        p["parameters"]["gebiedstabel"],
        json!([{"gebied": "Onbekendstad", "zetels": 1, "ingeschreven": false}])
    );
    assert_eq!(p["takeable"], json!(false), "{p}");

    // And then the cell records nothing.
    let (status, f, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(f["error"].as_str().unwrap().starts_with("not takeable"));
    let chronicle =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(chronicle.lines().count(), 1, "only the application");
}

#[tokio::test]
async fn taking_a_decision_records_a_decretogram() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = consumer_submit(&app, "12345678").await;
    let two = consumer_submit(&app, "87654321").await;
    let b = handler(&app).await;

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gram = &body["gram"];
    schema::validate(Kind::Gram, gram).unwrap();
    assert_eq!(gram["type"], "decretogram");
    assert_eq!(gram["stage"], "BESLUIT");
    assert_eq!(gram["refers_to"], json!({"on_application": case}));
    assert_ne!(gram["id"], case.as_str());
    assert_eq!(gram["legal_character"], "BESCHIKKING");
    assert_eq!(gram["legal_basis"], json!(["testregeling_afnemer#3 lid 1"]));
    assert_eq!(gram["decision_type"], "TOEKENNING");
    assert_eq!(gram["regulation"], "testregeling_afnemer");
    assert_eq!(gram["regulation_valid_from"], "2025-01-01");
    // Three axes: who records (the actor of the process), who the law makes
    // competent (literally from the regulation), and who acted, on behalf of
    // that authority.
    assert_eq!(gram["recording_actor"], "test_afnemer");
    assert_eq!(gram["competent_authority"], "Test afnemer");
    assert_eq!(
        gram["acting_actor"],
        json!({"role": "behandelaar", "channel": "medewerker",
               "identity": {"naam": "B. Behandelaar"}, "on_behalf_of": "Test afnemer"})
    );
    assert_eq!(body["warnings"], json!([]), "{body}");

    // The fields are the outputs of the article.
    assert_eq!(
        gram["fields"],
        json!({"vastgesteld_bedrag": 6000, "gebiedsbedrag": 5000, "besluit_tijdig": false,
               "besluitdeadline": "2025-04-11", "zorgvuldig": true})
    );
    // The inputs, each with its provenance.
    let inputs = gram["inputs"].as_object().unwrap();
    assert_eq!(
        inputs["besluitdatum"],
        json!({"value": "2025-03-12", "provenance": {"source": "handler"}})
    );
    assert_eq!(inputs["zetels_op_lijst"]["provenance"]["source"], "cell");
    assert_eq!(inputs["gebiedstabel"]["provenance"]["source"], "per_row");
    assert_eq!(
        inputs["bekendgemaakt"]["provenance"]["source"],
        "state_at_decision"
    );
    // The effective_at of the decision is the besluitdatum, with legal basis.
    assert_eq!(gram["effective_at"], "2025-03-12T00:00:00+01:00");
    assert_eq!(
        gram["effective_at_legal_basis"],
        json!(["testregeling_afnemer#3 lid 1"])
    );
    assert_eq!(
        inputs["aanvraagdatum"]["provenance"],
        json!({"source": "own", "lexostatus": "aanvraag_inhoud"})
    );
    // The receipt: the loaded regulations and the streams, with a hash over them.
    let receipt = &gram["receipt"];
    let regulations: Vec<&str> = receipt["regulations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(regulations.contains(&"testregeling_afnemer"), "{receipt}");
    assert_eq!(receipt["regulations"][0]["valid_from"], "2025-01-01");
    let streams: Vec<&str> = receipt["streams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        streams,
        ["test_afnemer_aanvragen", "test_afnemer_zaakverloop"]
    );
    assert_eq!(receipt["sha256"].as_str().unwrap().len(), 64);

    // The case disappears from the worklist, and the gram is in the case.
    let (_, w, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    let list = w["list"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["root"], two.as_str());
    let (_, z, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(z["grams"].as_array().unwrap().len(), 2);

    // A second decision in the same case: that is an amendment.
    let (status, f, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"].as_str().unwrap().contains(
            "is already in the case; another decision on it requires its own legal basis"
        ),
        "{f}"
    );
    let chronicle =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(
        chronicle.lines().count(),
        3,
        "two applications and one decision"
    );

    // The applicant may not decide.
    let (_, _, aanvrager) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A"})),
    )
    .await;
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{two}/actions/besluit"),
        aanvrager.as_deref(),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The cell gives a process only the case it asks for, and does not know an
    // unknown case.
    let (status, g, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER_CELL}/api/cases/{case}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{g}");
    let g = g.as_array().unwrap();
    assert_eq!(g.len(), 2);
    assert_eq!(g[0]["gram"]["id"], case.as_str());
    assert_eq!(g[1]["gram"]["refers_to"]["on_application"], case.as_str());
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER_CELL}/api/cases/00000000-0000-4000-8000-000000000009"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Two concurrent decisions on the other case: the cell records one,
    // because the stage check and the write share one lock.
    let path = format!("{CONSUMER}/api/cases/{two}/actions/besluit");
    let (a, other) = tokio::join!(
        call(&app, "POST", &path, Some(&b), Some(verdicts())),
        call(&app, "POST", &path, Some(&b), Some(verdicts())),
    );
    let mut statuses = [a.0, other.0];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::CREATED, StatusCode::CONFLICT],
        "{} / {}",
        a.1,
        other.1
    );
    let chronicle =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(
        chronicle.lines().count(),
        4,
        "two applications and two decisions"
    );
}

/// A setup in which article 3 (the beschikking) names an authority other
/// than the one the process acts for ('Test afnemer'), with an adjustment to
/// the process.
fn with_other_authority(
    process: &dyn Fn(String) -> String,
) -> (tempfile::TempDir, tempfile::TempDir, Router) {
    let cells = own_setup(
        &[
            ("afnemer", &as_is),
            ("register", &as_is),
            ("gebieden", &as_is),
        ],
        &[("afnemer", process)],
    );
    for e in std::fs::read_dir(fixtures().join("regulation")).unwrap() {
        let of = e.unwrap().path();
        let to = cells
            .path()
            .join("regulation")
            .join(of.file_name().unwrap());
        std::fs::create_dir_all(&to).unwrap();
        let text = std::fs::read_to_string(of.join("2025-01-01.yaml")).unwrap();
        let text = if of.ends_with("testregeling_afnemer") {
            // Only article 3 is a BESCHIKKING; the authority is added there.
            let with_authority = text.replace(
                "    machine_readable:\n      execution:\n        produces:\n          legal_character: BESCHIKKING",
                "    machine_readable:\n      competent_authority:\n        name: Een andere instantie\n      execution:\n        produces:\n          legal_character: BESCHIKKING",
            );
            assert_ne!(
                with_authority, text,
                "the authority was not put into the regulation"
            );
            with_authority
        } else {
            text
        };
        std::fs::write(to.join("2025-01-01.yaml"), text).unwrap();
    }
    let data = tempfile::tempdir().unwrap();
    let config = Config {
        cells_path: cells.path().join("cells"),
        processes_path: Some(cells.path().join("processes")),
        regulation_path: cells.path().join("regulation"),
        data_dir: data.path().to_path_buf(),
        port: 0,
        read_token: None,
        read_token_sources: Vec::new(),
        reduction: Default::default(),
        registers: None,
    };
    let app = Runtime::load(&config, clock()).unwrap().router;
    (cells, data, app)
}

#[tokio::test]
async fn another_competent_authority_refuses_the_decision() {
    // The law designates an authority other than the one the process acts
    // for, and the process has no mandate: no gram. Names are compared
    // literally, not normalized.
    let (_cells, data, app) = with_other_authority(&as_is);
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let (status, f, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("the law designates 'Een andere instantie' as competent authority, and the process acts on behalf of 'Test afnemer' without a mandate"),
        "{f}"
    );
    let chronicle =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(chronicle.lines().count(), 1, "only the application");
}

/// With a mandate from that authority (Awb 10:1) the cell does record the
/// decision: `competent_authority` is the authority of the law,
/// `recording_actor` the actor of the process, and the acting actor names the
/// handler, on whose behalf, and on which mandate.
#[tokio::test]
async fn a_mandate_allows_deciding_on_behalf_of_another_authority() {
    let with_mandate = |t: String| {
        t.replace(
            "on_behalf_of: {regulation: testregeling_afnemer}\n",
            "on_behalf_of: {regulation: testregeling_afnemer}\nmandates:\n  - {authority: Een andere instantie, legal_basis: 'testregeling_afnemer#7'}\n",
        )
    };
    let (_cells, _data, app) = with_other_authority(&with_mandate);
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let g = &body["gram"];
    assert_eq!(g["recording_actor"], "test_afnemer");
    assert_eq!(g["competent_authority"], "Een andere instantie");
    assert_eq!(
        g["acting_actor"],
        json!({
            "role": "behandelaar",
            "channel": "medewerker",
            "identity": {"naam": "B. Behandelaar"},
            "on_behalf_of": "Een andere instantie",
            "mandate": "testregeling_afnemer#7",
        })
    );
}

// --- Examples per action ---

#[tokio::test]
async fn examples_without_login() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) =
        call(&app, "GET", &format!("{CONSUMER}/api/examples"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["logins"],
        json!([
            {"label": "voorbeeld-login", "channel": "eherkenning", "fields": {"kvk": "12345678", "persoon": "A. Tester"}},
            {"label": "voorbeeld-login-ander", "channel": "eherkenning", "fields": {"kvk": "87654321", "persoon": "B. Tester"}},
        ])
    );
    assert_eq!(
        body["application"],
        consumer_concept(Some("VOORBEELD"))["external"]
    );
    assert_eq!(body["actions"]["besluit"], verdicts()["form"]);
    // "$vandaag" is the date of the clock when requested.
    assert_eq!(
        body["actions"]["bekendmaken"]["datum_bekendmaking"],
        "2025-03-12"
    );

    // A process without examples: empty, no error.
    let (status, body, _) = call(&app, "GET", &format!("{AGENCY}/api/examples"), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"logins": [], "application": null, "actions": {}})
    );
}

#[tokio::test]
async fn the_application_example_can_be_submitted() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (_, v, _) = call(&app, "GET", &format!("{CONSUMER}/api/examples"), None, None).await;
    let (status, _, cookie) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/channels/eherkenning/login"),
        None,
        Some(v["logins"][0]["fields"].clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/application"),
        cookie.as_deref(),
        Some(json!({"external": v["application"]})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[test]
fn examples_check_at_startup() {
    let gone = |t: String| t.replace("voorbeeld-besluit.json", "weg.json");
    let all = [
        ("afnemer", &as_is as &dyn Fn(String) -> String),
        ("register", &as_is),
        ("gebieden", &as_is),
    ];
    let setup = own_setup(&all, &[("afnemer", &gone)]);
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].starts_with("process 'test_afnemer_proces': example weg.json"),
        "{errors:?}"
    );

    let wrong = |t: String| t.replace("voorbeeld-besluit.json", "voorbeeld-login.json");
    let setup = own_setup(&all, &[("afnemer", &wrong)]);
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert!(
        errors[0].contains("voorbeeld-login.json: expected an object with 'form'"),
        "{errors:?}"
    );
}

// --- The cell records and reduces on trial, at the request of a process ---

/// A request to the agency cell for the portal event.
fn record_request(actor: &str) -> Value {
    json!({
        "actor": actor,
        "stream": "test_aanvragen",
        "event": "aanvraag_ontvangen",
        "intake": {"channel": "portaal", "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"}, "burger": {"nummer": null}},
        "external": complete()["external"],
    })
}

#[tokio::test]
async fn the_cell_records_a_gram_for_its_actor() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), dir.path()).unwrap();
    let grams = format!("{AGENCY_CELL}/api/grams");
    let (status, body) = as_runtime(&rt, "POST", &grams, record_request("test_instantie")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    schema::validate(Kind::Gram, &body["gram"]).unwrap();
    // The cell builds the gram from its stream: it gives the id and the
    // moment.
    assert!(body["gram"].get("refers_to").is_none());
    assert!(body["gram"]["id"].is_string());
    assert_eq!(body["gram"]["effective_at"], "2025-03-12T10:14:03+01:00");
    assert_eq!(body["gram"]["recording_actor"], "test_instantie");
    assert!(body["yaml"]
        .as_str()
        .unwrap()
        .starts_with("kind: chronolexogram\n"));
    let path = dir.path().join("test_instantie/test_kroniek.jsonl");
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);

    // An actor that is not the recording_actor of the stream: no gram.
    let (status, body) = as_runtime(&rt, "POST", &grams, record_request("test_afnemer")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        body["error"],
        "actor 'test_afnemer' does not record in stream 'test_aanvragen': the recording_actor is 'test_instantie'"
    );
    // An event the cell does not have, and a field the stream does not know.
    let mut unknown = record_request("test_instantie");
    unknown["event"] = json!("bestaat_niet");
    let (status, _) = as_runtime(&rt, "POST", &grams, unknown).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut field = record_request("test_instantie");
    field["external"]["schoenmaat"] = json!(44);
    let (status, body) = as_runtime(&rt, "POST", &grams, field).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("schoenmaat"));
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[tokio::test]
async fn the_cell_reduces_on_trial_without_recording() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), dir.path()).unwrap();
    let app = as_reader(&rt);
    let trial = format!("{AGENCY_CELL}/api/lexostatus/aanvraag_inhoud/trial");
    let (status, body) = as_runtime(
        &rt,
        "POST",
        &trial,
        json!({"draft": record_request("test_instantie"), "inputs": {}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The gram of the draft, and the reduction with that gram: the case
    // reference of the draft is the input.
    let case = body["gram"]["id"].as_str().unwrap();
    assert_eq!(body["lexostatus"]["root"], case);
    assert_eq!(body["lexostatus"]["parameters"]["bevat_naam"], json!(true));
    assert_eq!(
        body["lexostatus"]["parameters"]["aanvraagdatum"],
        "2025-03-12"
    );
    // Nothing recorded.
    let (_, chronicle, _) = call(
        &app,
        "GET",
        &format!("{AGENCY_CELL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(chronicle, json!([]));
    // On trial, too, only the actor of the stream submits anything.
    let (status, _) = as_runtime(
        &rt,
        "POST",
        &trial,
        json!({"draft": record_request("iemand_anders")}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_trial_reduction_reduces_the_chronicle_with_the_draft() {
    // The consumer's worklist: a list over the whole chronicle. On trial the
    // draft counts alongside what is already there, and nothing of it
    // remains.
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), data.path()).unwrap();
    let app = as_reader(&rt);
    consumer_submit(&app, "12345678").await;
    let concept = json!({
        "actor": "test_afnemer",
        "stream": "test_afnemer_aanvragen",
        "event": "aanvraag_ontvangen",
        "intake": {"channel": "portaal", "eherkenning": {"kvk": "87654321", "persoon": "B. Tester"}, "burger": {"nummer": null}},
        "external": consumer_concept(Some("VOORBEELD"))["external"],
    });
    let (status, body) = as_runtime(
        &rt,
        "POST",
        &format!("{CONSUMER_CELL}/api/lexostatus/werkvoorraad/trial"),
        json!({"draft": concept}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lexostatus"]["list"].as_array().unwrap().len(), 2);
    let (_, w, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER_CELL}/api/lexostatus/werkvoorraad"),
        None,
        None,
    )
    .await;
    assert_eq!(w["list"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn only_the_runtime_records_and_reads() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), dir.path()).unwrap();
    let grams = format!("{AGENCY_CELL}/api/grams");
    let trial = format!("{AGENCY_CELL}/api/lexostatus/aanvraag_inhoud/trial");
    // Without a token: 401, with a different token: 403, for recording and trial.
    for (path, body) in [
        (&grams, record_request("test_instantie")),
        (&trial, json!({"draft": record_request("test_instantie")})),
    ] {
        let (status, error, _) = call(&rt.router, "POST", path, None, Some(body.clone())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: {error}");
        assert!(error["error"]
            .as_str()
            .unwrap()
            .contains("the runtime token is absent"));
        let false_ = [(RUNTIME_TOKEN_HEADER, "0".repeat(64))];
        let false_: Vec<(&str, &str)> = false_.iter().map(|(n, w)| (*n, w.as_str())).collect();
        let (status, _, _) = call_with(&rt.router, "POST", path, &false_, Some(body)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    // The token of another runtime does not count either.
    let other = runtime_at(&fixtures(), tempfile::tempdir().unwrap().path()).unwrap();
    let foreign = [(RUNTIME_TOKEN_HEADER, other.runtime_token.as_str())];
    let (status, _, _) = call_with(
        &rt.router,
        "POST",
        &grams,
        &foreign,
        Some(record_request("test_instantie")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Nothing was recorded. Reading requires a token: the grams carry the
    // identity and the intake of the submitter. Only the stream definitions
    // are open.
    for path in ["chronicle", "cases/z", "lexostatus/aanvraag_inhoud?root=z"] {
        let uri = format!("{AGENCY_CELL}/api/{path}");
        let (status, error, _) = call(&rt.router, "GET", &uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: {error}");
        let (status, _, _) = call_with(&rt.router, "GET", &uri, &foreign, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        // Without a read token in the runtime, a read token does not count.
        let read = [(READ_TOKEN_HEADER, "gedeeld-leestoken-van-de-test")];
        let (status, _, _) = call_with(&rt.router, "GET", &uri, &read, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    let (status, _, _) = call(
        &rt.router,
        "GET",
        &format!("{AGENCY_CELL}/api/stream"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!dir
        .path()
        .join("test_instantie/test_kroniek.jsonl")
        .exists());
    // The process itself does record: the internal transport carries the token.
    let c = logins(&rt.router, "12345678").await;
    let (status, body, _) = call(
        &rt.router,
        "POST",
        &format!("{AGENCY}/api/application"),
        Some(&c),
        Some(complete()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

// --- The offer assesses only what is fixed beforehand ---

#[test]
fn an_offer_on_an_application_fact_stops_the_runtime() {
    let with_offer = |t: String| {
        t.replace(
            "  form:",
            "  offer: {regulation: testregeling_aanvraag, output: aanvraag_volledig}\n  form:",
        )
    };
    let setup = own_setup(&[("instantie", &as_is)], &[("instantie", &with_offer)]);
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert!(
        errors.contains(&"process 'test_instantie_proces': offer: condition relies on 'aanvraagdatum' (BELANGHEBBENDE, grondslag testregeling_aanvraag#1 lid 1), which is not known beforehand".to_string()),
        "{errors:?}"
    );
}

// --- The window of the offer ---

/// The offer runs per window the policy offers: the output `offer.windows` of
/// the regulation, from a run on today's date. The window is the parameter
/// with origin BELANGHEBBENDE and `rol: TIJDVAK`, not a fixed name.
#[tokio::test]
async fn the_offer_runs_per_chosen_window() {
    let with_offer = |t: String| {
        t.replace(
            "    output: aanvraag_toelaatbaar\n",
            "    output: aanvraag_toelaatbaar\n  offer:\n    regulation: testregeling_afnemer\n    output: aanvraag_aangeboden\n    deadline: aanvraagtermijn\n    windows: aangeboden_jaren\n    start: begin_aanvraagjaar\n",
        )
    };
    let setup = own_setup(
        &[
            ("afnemer", &as_is),
            ("register", &as_is),
            ("gebieden", &as_is),
        ],
        &[("afnemer", &with_offer)],
    );
    let data = tempfile::tempdir().unwrap();
    let app = runtime_at(setup.path(), data.path()).unwrap().router;
    let (_, _, cookie) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/possibilities"),
        cookie.as_deref(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let m = body["possibilities"].as_array().unwrap();
    assert_eq!(m.len(), 2, "{body}");
    for (i, year) in [(0, 2025), (1, 2026)] {
        assert_eq!(
            m[i]["possibility"]["window"],
            json!({"parameter": "aanvraagjaar", "value": year}),
            "{body}"
        );
        assert_eq!(m[i]["parameters"]["aanvraagjaar"], json!(year));
        assert_eq!(
            m[i]["provenance"]["aanvraagjaar"],
            json!({"source": "choice"})
        );
        assert_eq!(
            m[i]["possibility"]["deadline"],
            json!(format!("{year}-04-01"))
        );
    }
    // The current year is taken as of today, a coming year as of the start the
    // regulation states (art. 6), not as of a year the code assumes.
    assert_eq!(m[0]["as_of"], json!("2025-03-12"), "{body}");
    assert_eq!(m[1]["as_of"], json!("2026-01-01"), "{body}");
}

/// An offer that asks for a window, without windows: the runtime does not start.
#[test]
fn a_window_without_windows_stops_the_runtime() {
    let with_offer = |t: String| {
        t.replace(
            "    output: aanvraag_toelaatbaar\n",
            "    output: aanvraag_toelaatbaar\n  offer: {regulation: testregeling_afnemer, output: aanvraag_aangeboden}\n",
        )
    };
    let setup = own_setup(
        &[
            ("afnemer", &as_is),
            ("register", &as_is),
            ("gebieden", &as_is),
        ],
        &[("afnemer", &with_offer)],
    );
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert_eq!(
        errors,
        ["process 'test_afnemer_proces': offer: the window 'aanvraagjaar' (rol TIJDVAK) asks for offer.windows: the output of the policy with the windows the portal offers"]
    );
}

/// The windows come from the regulation of the offer, in a run without
/// parameters: an output that does not exist, or one from an article that
/// asks for a parameter, stops the runtime.
#[test]
fn the_windows_come_from_the_policy() {
    for (windows, expected) in [
        (
            "bestaat_niet",
            "portal.offer: regulation 'testregeling_afnemer' has no windows output 'bestaat_niet'",
        ),
        (
            "gebiedsbedrag",
            "portal.offer: windows 'gebiedsbedrag' comes from testregeling_afnemer#3, and that article requires a parameter",
        ),
    ] {
        let with_offer = move |t: String| {
            t.replace(
                "    output: aanvraag_toelaatbaar\n",
                &format!("    output: aanvraag_toelaatbaar\n  offer: {{regulation: testregeling_afnemer, output: aanvraag_aangeboden, windows: {windows}}}\n"),
            )
        };
        let setup = own_setup(
            &[("afnemer", &as_is), ("register", &as_is), ("gebieden", &as_is)],
            &[("afnemer", &with_offer)],
        );
        let data = tempfile::tempdir().unwrap();
        let errors = runtime_at(setup.path(), data.path()).err().unwrap();
        assert!(
            errors.iter().any(|f| f.contains(expected)),
            "expected '{expected}' in {errors:?}"
        );
    }
}

/// A record event with a stage the procedure of the beschikking does not
/// know: the state at decision cannot be derived then, and the runtime does
/// not start.
#[test]
fn a_stage_outside_the_procedure_stops_the_runtime() {
    let setup = own_setup(
        &[
            ("afnemer", &as_is),
            ("register", &as_is),
            ("gebieden", &as_is),
        ],
        &[("afnemer", &as_is)],
    );
    let stream = setup
        .path()
        .join("chronicles/test_afnemer_zaakverloop.yaml");
    let text = std::fs::read_to_string(&stream).unwrap();
    std::fs::write(&stream, text.replace("stage: BESLUIT", "stage: BESLISSING")).unwrap();
    let lexo = setup.path().join("cells/afnemer/lexostatuses.yaml");
    let text = std::fs::read_to_string(&lexo).unwrap();
    std::fs::write(&lexo, text.replace("stage: BESLUIT", "stage: BESLISSING")).unwrap();
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert!(
        errors.iter().any(|f| f.contains(
            "action 'besluit', record test_afnemer_zaakverloop/besluit_genomen: stage 'BESLISSING' is not in procedure 'beschikking' of testregeling_afnemer#3 (AANVRAAG, BESLUIT, BEKENDMAKING, BEZWAAR)"
        )),
        "{errors:?}"
    );
}

// --- The assessment builds a table per row, like the decision ---

/// The rows of the assessment: the gebiedstabel from the trial reduction of
/// the draft, with a column that comes per row from the register cell.
fn with_assessment_rows(t: String) -> String {
    t.replace(
        "    output: aanvraag_toelaatbaar\n",
        "    output: aanvraag_toelaatbaar
    rows:
      - parameter: gebiedstabel
        table: {lexostatus: aanvraag_inhoud, field: gebieden}
        columns: {gebied: gebied}
        sources:
          - cell: test_register
            lexostatus: registratie_per_gebied
            input:
              aanduiding: {lexostatus: aanvraag_inhoud, field: aanduiding}
              gebied: {column: gebied}
            columns: {ingeschreven: ingeschreven}
",
    )
}

#[tokio::test]
async fn the_assessment_builds_a_table_per_row() {
    let setup = own_setup(
        &[
            ("afnemer", &as_is),
            ("register", &as_is),
            ("gebieden", &as_is),
        ],
        &[("afnemer", &with_assessment_rows)],
    );
    let data = tempfile::tempdir().unwrap();
    let a = runtime_at(setup.path(), data.path()).unwrap();
    let body = consumer_assessment(&a.router, Some("VOORBEELD")).await;
    // The rows come from the draft, the column `ingeschreven` per row from the
    // register cell; the table goes to the engine with the other parameters.
    assert_eq!(
        body["parameters"]["gebiedstabel"],
        json!([
            {"gebied": "Voorbeeldstad", "ingeschreven": true},
            {"gebied": "Buurdorp", "ingeschreven": false},
        ]),
        "{body}"
    );
    assert_eq!(
        body["provenance"]["gebiedstabel"],
        json!({"source": "per_row", "lexostatus": "aanvraag_inhoud", "field": "gebieden"})
    );
    assert_eq!(body["rows"][0]["sources"][0]["queried"], json!(2));
    assert!(body["rows"][0].get("missing").is_none(), "{body}");
    assert_eq!(body["result"]["value"], json!(true), "{body}");
    // Nothing recorded.
    assert!(!data.path().join("test_afnemer/test_afnemer.jsonl").exists());

    // Without an aanduiding the per-row source is not queried: the column
    // stays out, nothing is filled in.
    let body = consumer_assessment(&a.router, None).await;
    assert_eq!(
        body["parameters"]["gebiedstabel"],
        json!([
            {"gebied": "Voorbeeldstad"},
            {"gebied": "Buurdorp"},
        ])
    );
    assert_eq!(body["rows"][0]["missing"], json!(["ingeschreven"]));
    assert_eq!(body["rows"][0]["sources"][0]["status"], "not_queried");
}

#[tokio::test]
async fn the_assessment_without_rows_stays_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let body = consumer_assessment(&app, Some("VOORBEELD")).await;
    assert_eq!(body["rows"], json!([]));
    assert!(body["parameters"].get("gebiedstabel").is_none(), "{body}");
    assert_eq!(body["result"]["value"], json!(true), "{body}");
}

#[test]
fn assessment_rows_check_at_startup() {
    let from_other = |t: String| {
        with_assessment_rows(t).replacen(
            "table: {lexostatus: aanvraag_inhoud, field: gebieden}",
            "table: {lexostatus: zaakverloop, field: gebieden}",
            1,
        )
    };
    let setup = own_setup(
        &[
            ("afnemer", &as_is),
            ("register", &as_is),
            ("gebieden", &as_is),
        ],
        &[("afnemer", &from_other)],
    );
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_at(setup.path(), data.path()).err().unwrap();
    assert!(
        errors.contains(&"process 'test_afnemer_proces': assessment, rows 'gebiedstabel': the table comes from lexostatus 'zaakverloop', and that is not the assessment lexostatus".to_string()),
        "{errors:?}"
    );
}

/// A legal basis in the form that points to no article of a loaded
/// regulation, or a paragraph the article does not have: the runtime does not
/// start.
#[test]
fn a_legal_basis_in_the_form_is_checked() {
    for (legal_basis, expected) in [
        (
            "testregeling_aanvraag#9",
            "form, field 'aanvraagjaar': legal basis 'testregeling_aanvraag#9': regulation 'testregeling_aanvraag' has no article 9",
        ),
        (
            "testregeling_aanvraag#1 lid 8",
            "form, field 'aanvraagjaar': legal basis 'testregeling_aanvraag#1 lid 8': article 1 has no paragraph 8",
        ),
    ] {
        let setup = own_setup(&[("instantie", &as_is)], &[("instantie", &as_is)]);
        let path = setup.path().join("processes/instantie/formulier.yaml");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replace("grondslag: testregeling_aanvraag#1 lid 1}", &format!("grondslag: '{legal_basis}'}}")),
        )
        .unwrap();
        let data = tempfile::tempdir().unwrap();
        let errors = runtime_at(setup.path(), data.path()).err().unwrap();
        assert!(
            errors.iter().any(|f| f.contains(expected)),
            "expected '{expected}' in {errors:?}"
        );
    }
}

// --- Time: two times per gram, and an as-of on the reduction ---

/// An application that came in by another route: the counter states the day
/// of receipt (`$intake.received_at`). Legally that day counts (the
/// aanvraagdatum); recording happens on the cell's clock.
#[tokio::test]
async fn an_earlier_receipt_is_the_application_date() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), dir.path()).unwrap();
    let app = as_reader(&rt);
    let mut v = record_request("test_instantie");
    v["intake"] = json!({"channel": "counter", "received_at": "2025-03-05",
                         "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"},
                         "burger": {"nummer": null}});
    let (status, body) = as_runtime(&rt, "POST", &format!("{AGENCY_CELL}/api/grams"), v).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let g = &body["gram"];
    schema::validate(Kind::Gram, g).unwrap();
    assert_eq!(g["effective_at"], "2025-03-05T00:00:00+01:00");
    assert_eq!(g["recorded_at"], "2025-03-12T10:14:03+01:00");
    assert_eq!(
        g["effective_at_legal_basis"],
        json!(["testregeling_aanvraag#1"])
    );
    let case = g["id"].as_str().unwrap();
    let (status, l, _) = call(
        &app,
        "GET",
        &format!("{AGENCY_CELL}/api/lexostatus/aanvraag_inhoud?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(l["parameters"]["aanvraagdatum"], "2025-03-05");
    assert_eq!(l["recorded_at"], "2025-03-12T10:14:03+01:00");

    // A receipt after the recording is not a fact.
    let mut v = record_request("test_instantie");
    v["intake"]["received_at"] = json!("2025-03-20");
    let (status, body) = as_runtime(&rt, "POST", &format!("{AGENCY_CELL}/api/grams"), v).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("lies after the recording"));
}

/// `GET lexostatus` with an as-of: the same chronicle gives a different
/// lexostatus at a different moment. The initial state legally applies at its
/// own moments, but is only known since it was loaded.
#[tokio::test]
async fn lexostatus_at_an_as_of_moment() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    // The seats from `registerstatus`, the registration from the register of
    // the raad; together in one answer.
    let status_at = |query: &'static str| {
        let app = app.clone();
        async move {
            let (s, mut l, _) = call(
                &app,
                "GET",
                &format!("/cells/test_register/api/lexostatus/registerstatus?aanduiding=VOORBEELD{query}"),
                None,
                None,
            )
            .await;
            if s == StatusCode::OK {
                let (_, r, _) = call(
                    &app,
                    "GET",
                    &format!("/cells/test_register/api/lexostatus/register?aanduiding=VOORBEELD&orgaan=raad{query}"),
                    None,
                    None,
                )
                .await;
                l["parameters"]["is_ingeschreven_raad"] =
                    r["parameters"]["is_ingeschreven_in_register"].clone();
            }
            (s, l)
        }
    };
    let (s, now) = status_at("").await;
    assert_eq!(s, StatusCode::OK, "{now}");
    assert_eq!(now["parameters"]["zetels_toegewezen"], json!(6));
    assert!(now.get("as_of").is_none());

    // Legally on January 1, 2024: not yet registered, no result.
    let (s, jan) = status_at("&as_of=2024-01-01").await;
    assert_eq!(s, StatusCode::OK, "{jan}");
    assert_eq!(jan["as_of"], "2024-01-01");
    assert_eq!(jan["parameters"]["is_ingeschreven_raad"], json!(false));
    assert_eq!(jan["parameters"]["zetels_toegewezen"], json!(0));
    // A moment with a time zone works too (in a query as %2B for '+').
    let (s, apr) = status_at("&as_of=2024-04-01T00:00:00%2B02:00").await;
    assert_eq!(s, StatusCode::OK, "{apr}");
    assert_eq!(apr["parameters"]["zetels_toegewezen"], json!(6));
    assert_eq!(apr["parameters"]["is_ingeschreven_raad"], json!(true));

    // As known before loading the initial state (the test's clock): the cell
    // did not know anything yet.
    let (s, earlier) = status_at("&known_at=2025-03-11").await;
    assert_eq!(s, StatusCode::OK, "{earlier}");
    assert_eq!(earlier["parameters"]["is_ingeschreven_raad"], json!(false));
    let (_, then) = status_at("&known_at=2025-03-12").await;
    assert_eq!(then["parameters"], now["parameters"]);

    let (s, f) = status_at("&as_of=morgen").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(f["error"].as_str().unwrap().contains("invalid as_of"));
}

/// The trial route takes an as-of too: a draft counts as recorded on the
/// clock of now, so as known on an earlier day it is not there.
#[tokio::test]
async fn the_trial_route_also_takes_an_as_of() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), dir.path()).unwrap();
    let trial = format!("{AGENCY_CELL}/api/lexostatus/aanvraag_inhoud/trial");
    let (status, body) = as_runtime(
        &rt,
        "POST",
        &trial,
        json!({"draft": record_request("test_instantie"), "inputs": {"as_of": "2025-03-12"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lexostatus"]["as_of"], "2025-03-12");
    let (status, _) = as_runtime(
        &rt,
        "POST",
        &trial,
        json!({"draft": record_request("test_instantie"), "inputs": {"known_at": "2025-03-11"}}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// A chronicle from before chronolex v0.2.0 (without id, with zaakkenmerk) is
/// not converted: the runtime does not start, and says why.
#[tokio::test]
async fn an_old_chronicle_does_not_start() {
    let data = tempfile::tempdir().unwrap();
    let case = {
        let app = app(data.path());
        consumer_submit(&app, "12345678").await
    };
    let path = data.path().join("test_afnemer/test_afnemer.jsonl");
    let rows: Vec<String> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|r| {
            let mut g: Value = serde_json::from_str(r).unwrap();
            let o = g.as_object_mut().unwrap();
            o.remove("id");
            o.insert("case".into(), json!("opent"));
            o.insert("zaakkenmerk".into(), json!(case));
            g.to_string()
        })
        .collect();
    std::fs::write(&path, rows.join("\n") + "\n").unwrap();
    let f = runtime_at(&fixtures(), data.path())
        .err()
        .unwrap()
        .join("\n");
    assert!(f.contains("from before chronolex v0.2.0"), "{f}");
    assert!(f.contains("empty DATA_DIR"), "{f}");
}

// --- Channels and roles as configuration ---

/// A second channel of the same portal: a citizen logs in with a nine-digit
/// number that passes the elfproef. Their number goes into the gram under the
/// intake path of their channel; what the other channel delivers stays
/// empty.
#[tokio::test]
async fn a_second_channel_with_the_elfproef() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let login = format!("{AGENCY}/api/channels/burger/login");
    for error in ["123456789", "12345678", "1234567890"] {
        let (status, body, _) =
            call(&app, "POST", &login, None, Some(json!({"nummer": error}))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}: {body}");
        assert_eq!(body["error"], "dit is geen geldig burgernummer");
    }
    let (status, session, cookie) = call(
        &app,
        "POST",
        &login,
        None,
        Some(json!({"nummer": "123456782"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(
        session,
        json!({"role": "burger", "channel": "burger", "fields": {"nummer": "123456782"}})
    );
    let cookie = cookie.unwrap();
    // The session belongs to this channel, not to the other.
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{AGENCY}/api/channels/eherkenning/session"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, s, _) = call(
        &app,
        "GET",
        &format!("{AGENCY}/api/session"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["channel"], "burger");
    // The portal belongs to both channels.
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application"),
        Some(&cookie),
        Some(complete()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let o = &body["gram"]["fields"]["core"]["signed_via"];
    assert_eq!(
        o,
        &json!({"kanaal": "portaal", "kvk_nummer": null, "gemachtigde": null, "burgernummer": "123456782"})
    );
    // An unknown channel does not exist.
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/channels/digid/login"),
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// A role names its channel and its routes. Through the agency's employee
/// channel only the role loket logs in; it may use the counter and not the
/// portal, and the applicant the portal and not the counter.
#[tokio::test]
async fn a_role_may_only_use_its_routes() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let employee = format!("{AGENCY}/api/channels/medewerker/login");
    let (status, body, _) = call(
        &app,
        "POST",
        &employee,
        None,
        Some(json!({"naam": "L. Loket", "role": "aanvrager"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, session, counter) = call(
        &app,
        "POST",
        &employee,
        None,
        Some(json!({"naam": "L. Loket"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(session["role"], "loket");
    let counter = counter.unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application"),
        Some(&counter),
        Some(complete()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("only for the role aanvrager or burger"),
        "{body}"
    );
    let aanvrager = logins(&app, "12345678").await;
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/counter/application"),
        Some(&aanvrager),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/counter/application"),
        Some(&aanvrager),
        Some(counter_input("2025-03-05")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // The consumer process has no counter.
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/counter/application"),
        None,
        Some(counter_input("2025-03-05")),
    )
    .await;
    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED);
}

fn counter_input(received_at: &str) -> Value {
    json!({
        "applicant": {"channel": "eherkenning", "kvk": "12345678", "persoon": "A. Tester"},
        "received_at": received_at,
        "external": complete()["external"],
    })
}

async fn counter(app: &Router) -> String {
    let (status, body, cookie) = call(
        app,
        "POST",
        &format!("{AGENCY}/api/channels/medewerker/login"),
        None,
        Some(json!({"naam": "L. Loket"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    cookie.unwrap()
}

/// The counter enters an application that came in by another route, on
/// behalf of the applicant, with the day of receipt (Awb 4:1, 4:13). That day
/// is the `effective_at`, the entry `recorded_at`. The counter refuses a day
/// after today; the counter identifies the applicant with the fields of a
/// portal channel.
#[tokio::test]
async fn the_counter_enters_an_earlier_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let l = counter(&app).await;
    let path = format!("{AGENCY}/api/counter/application");
    let (status, body, _) = call(
        &app,
        "POST",
        &path,
        Some(&l),
        Some(counter_input("2025-03-05")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let g = &body["gram"];
    assert_eq!(g["effective_at"], "2025-03-05T00:00:00+01:00");
    assert_eq!(g["recorded_at"], "2025-03-12T10:14:03+01:00");
    assert_eq!(
        g["fields"]["core"]["signed_via"],
        json!({"kanaal": "counter", "kvk_nummer": "12345678", "gemachtigde": "A. Tester", "burgernummer": null})
    );
    // The applicant follows their paper application: the number is theirs.
    let case = g["id"].as_str().unwrap();
    let (status, l2, _) = call(
        &app,
        "GET",
        &format!("{AGENCY_CELL}/api/lexostatus/aanvraag_inhoud?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(l2["parameters"]["aanvraagdatum"], "2025-03-05");

    for (input, error) in [
        (counter_input("2025-03-13"), "is after today"),
        (counter_input("vorige week"), "invalid received_at"),
        (
            json!({"applicant": {"kvk": "12345678", "persoon": "A"}, "received_at": "2025-03-05"}),
            "applicant: name the channel",
        ),
        (
            json!({"applicant": {"channel": "eherkenning", "kvk": "1", "persoon": "A"}, "received_at": "2025-03-05"}),
            "applicant: een organisatienummer heeft acht cijfers",
        ),
    ] {
        let (status, body, _) = call(&app, "POST", &path, Some(&l), Some(input)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"].as_str().unwrap().contains(error),
            "{error}: {body}"
        );
    }
}

/// If the policy names an opening of the window (`offer.opening`), the
/// counter enters no receipt from before that day. The window comes from the
/// field of the application.
#[tokio::test]
async fn the_counter_refuses_a_receipt_before_the_opening() {
    let with_offer = |t: String| {
        t.replace(
            "  form:",
            "  offer:\n    regulation: testregeling_aanvraag\n    output: aanvraag_aangeboden\n    windows: aangeboden_jaren\n    opening: openstelling_aanvraagjaar\n  form:",
        )
    };
    let setup = own_setup(&[("instantie", &as_is)], &[("instantie", &with_offer)]);
    let data = tempfile::tempdir().unwrap();
    let app = runtime_at(setup.path(), data.path()).unwrap().router;
    let l = counter(&app).await;
    let path = format!("{AGENCY}/api/counter/application");
    // Aanvraagjaar 2025 is open from January 1, 2025.
    let (status, body, _) = call(
        &app,
        "POST",
        &path,
        Some(&l),
        Some(counter_input("2024-12-20")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("is before the opening of the window (2025-01-01)"),
        "{body}"
    );
    let (status, body, _) = call(
        &app,
        "POST",
        &path,
        Some(&l),
        Some(counter_input("2025-01-02")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // Without a window the opening cannot be assessed.
    let mut without = counter_input("2025-01-02");
    without["external"]["aanvraagjaar"] = Value::Null;
    let (status, body, _) = call(&app, "POST", &path, Some(&l), Some(without)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("the window (aanvraagjaar) is absent"),
        "{body}"
    );
}

// --- Actions after the decision: announcing, paying, and the case progress ---

async fn action(
    app: &Router,
    b: &str,
    case: &str,
    name: &str,
    trial: bool,
    form: Value,
) -> (StatusCode, Value) {
    action_in(app, CONSUMER, b, case, name, trial, json!({ "form": form })).await
}

/// An action in a case of a process, on trial or taken, with the body as the
/// route reads it (`form`, and if needed `happened`).
async fn action_in(
    app: &Router,
    process: &str,
    b: &str,
    case: &str,
    name: &str,
    trial: bool,
    body: Value,
) -> (StatusCode, Value) {
    let path = if trial {
        format!("{process}/api/cases/{case}/actions/{name}/trial")
    } else {
        format!("{process}/api/cases/{case}/actions/{name}")
    };
    let (status, body, _) = call(app, "POST", &path, Some(b), Some(body)).await;
    (status, body)
}

/// After the decision: the announcement is a follow-up to the decision (the
/// engine runs stage BEKENDMAKING on the inputs of the recorded decision, and
/// the hook of that stage computes the objection period); the payment is an
/// executogram that is only recorded if the assessment of its legal basis is
/// true. The law refuses a second payment above the amount.
#[tokio::test]
async fn decision_announce_and_pay() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let payment = |amount: i64| json!({"bedrag": amount, "datum_betaling": "2025-03-12"});

    // Before the decision: announcing and paying wait for the decision they
    // follow.
    let (status, f) = action(
        &app,
        &b,
        &case,
        "bekendmaken",
        false,
        json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": true}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("waiting for the decision"),
        "{f}"
    );
    let (status, f) = action(&app, &b, &case, "betalen", false, payment(6000)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("waiting for the decision"),
        "{f}"
    );

    let (status, body) = action(
        &app,
        &b,
        &case,
        "besluit",
        false,
        verdicts()["form"].clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // After the decision, before the announcement: the decision is not in force.
    let (status, f) = action(&app, &b, &case, "betalen", false, payment(6000)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"].as_str().unwrap().contains("betaling_conform"),
        "{f}"
    );

    // The form of the announcement is what the stage asks for.
    let (_, z, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    let known = &z["actions"][1];
    assert_eq!(known["available"], json!(true), "{known}");
    let fields: Vec<&str> = known["form"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(fields, ["datum_bekendmaking", "bekendgemaakt"]);
    assert_eq!(z["actions"][0]["available"], json!(false));

    // An announcement not made in the prescribed manner gives no objection
    // period: not takeable.
    let (status, p) = action(
        &app,
        &b,
        &case,
        "bekendmaken",
        true,
        json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": false}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["takeable"], json!(false), "{p}");
    // A conclusion about the content: if it happened anyway, it can be reported.
    assert_eq!(p["reportable"], json!(true), "{p}");

    let (status, body) = action(
        &app,
        &b,
        &case,
        "bekendmaken",
        false,
        json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": true}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gram = &body["gram"];
    assert_eq!(gram["stage"], "BEKENDMAKING");
    assert_eq!(gram["type"], "act");
    assert_eq!(gram["fields"]["aanvang_bezwaartermijn"], "2025-03-13");
    assert_eq!(gram["fields"]["einde_bezwaartermijn"], "2025-04-23");
    // Art. 3 in the stage BEKENDMAKING: now the decision is announced.
    assert_eq!(gram["fields"]["besluit_tijdig"], json!(true));
    assert_eq!(gram["effective_at"], "2025-03-12T00:00:00+01:00");

    // Paying, in two parts; the reduction adds up the payments.
    let (status, body) = action(&app, &b, &case, "betalen", false, payment(4000)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["type"], "executogram");
    assert_eq!(body["trial"]["outputs"]["nog_te_betalen"], json!(2000));
    let (status, f) = action(&app, &b, &case, "betalen", false, payment(2001)).await;
    assert_eq!(status, StatusCode::CONFLICT, "above the amount: {f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("report it as happened"),
        "{f}"
    );
    let (status, body) = action(&app, &b, &case, "betalen", false, payment(2000)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["trial"]["outputs"]["nog_te_betalen"], json!(0));
    let (status, _) = action(&app, &b, &case, "betalen", false, payment(1)).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // The case: 0 still to pay, and the objection period from the procedure.
    let (_, z, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    let pay = z["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "betalen")
        .unwrap();
    assert_eq!(pay["trial"]["outputs"]["nog_te_betalen"], json!(0), "{pay}");
    assert_eq!(pay["recorded"], json!(2));
    assert_eq!(pay["decision"], z["decisions"][0]["id"]);
    let decision = &z["decisions"][0];
    assert_eq!(decision["action"], "besluit");
    assert_eq!(
        decision["actions"],
        json!(["bekendmaken", "betalen"]),
        "{decision}"
    );
    let r = &decision["legal_protection"];
    assert_eq!(r["after"], "BEKENDMAKING");
    assert_eq!(r["stage"], "BEZWAAR");
    assert_eq!(r["legal_basis"], json!(["testregeling_awb#4"]));
    assert_eq!(r["outputs"]["einde_bezwaartermijn"], "2025-04-23");
    // The lexostatus of the decision contains the route.
    let (_, l, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER_CELL}/api/lexostatus/besluit?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(
        l["extra_fields"]["einde_bezwaartermijn"], "2025-04-23",
        "{l}"
    );
    assert_eq!(l["parameters"]["betaald_bedrag"], json!(6000));
}

/// Two concurrent payments that together exceed the set amount: each computes
/// what is still to be paid on the case as the process read it, and the cell
/// only records if the case has not changed since (`case_grams`, under the
/// write lock). One gets through.
#[tokio::test]
async fn two_concurrent_payments() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let (status, body) = action(
        &app,
        &b,
        &case,
        "besluit",
        false,
        verdicts()["form"].clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = action(
        &app,
        &b,
        &case,
        "bekendmaken",
        false,
        json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": true}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let payment = json!({"bedrag": 4000, "datum_betaling": "2025-03-12"});
    let (a, other) = tokio::join!(
        action(&app, &b, &case, "betalen", false, payment.clone()),
        action(&app, &b, &case, "betalen", false, payment.clone()),
    );
    let mut statuses = [a.0, other.0];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::CREATED, StatusCode::CONFLICT],
        "{} / {}",
        a.1,
        other.1
    );
    // Which refusal the second one gets depends on the order. If both trials
    // read the case before the first was recorded, the cell refuses on the
    // optimistic check. If the second read it afterwards, the law says no
    // (it has already been paid) and the process does not take the action.
    // Recording twice happens in neither case.
    let refused = if a.0 == StatusCode::CONFLICT {
        &a.1
    } else {
        &other.1
    };
    let error = refused["error"].as_str().unwrap();
    assert!(
        error.contains("changed since the process read it") || error.contains("says no"),
        "{refused}"
    );
    let (_, l, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER_CELL}/api/lexostatus/besluit?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(l["parameters"]["betaald_bedrag"], json!(4000), "{l}");
}

/// A second action in the case progress: a request for a supplement counts on
/// trial, and after recording the decision reads it.
#[tokio::test]
async fn requesting_a_supplement_carries_into_the_decision() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let (_, p) = action(&app, &b, &case, "aanvulling_vragen", true, json!({})).await;
    // Without the date the fact does not count: not takeable.
    assert_eq!(p["takeable"], json!(false), "{p}");
    assert!(
        p["reason"].as_str().unwrap().contains("datum_uitnodiging"),
        "{p}"
    );
    let (_, p) = action(
        &app,
        &b,
        &case,
        "aanvulling_vragen",
        true,
        json!({"datum_uitnodiging": "2025-03-12"}),
    )
    .await;
    assert_eq!(p["outputs"]["termijn_opgeschort"], json!(true), "{p}");
    let (status, body) = action(
        &app,
        &b,
        &case,
        "aanvulling_vragen",
        false,
        json!({"datum_uitnodiging": "2025-03-12"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (_, p) = action(&app, &b, &case, "besluit", true, verdicts()["form"].clone()).await;
    assert_eq!(
        p["parameters"]["datum_uitnodiging_aanvulling"], "2025-03-12",
        "{p}"
    );
}

/// Take an action with `happened`: the handler reports a fact that happened
/// while the trial said no on the content.
async fn report(app: &Router, b: &str, case: &str, name: &str, form: Value) -> (StatusCode, Value) {
    let (status, body, _) = call(
        app,
        "POST",
        &format!("{CONSUMER}/api/cases/{case}/actions/{name}"),
        Some(b),
        Some(json!({"form": form, "happened": true})),
    )
    .await;
    (status, body)
}

/// The process concludes before it acts; what happened, the cell records
/// anyway. The process does not make a payment above the amount on its own,
/// but reported as happened it is recorded, and then the excess is paid
/// unduly. An announcement that does not comply with the law is recorded, as
/// reported, without an objection period. A decision is not reported, and an
/// incomplete form is not recorded.
#[tokio::test]
async fn a_reported_fact_is_recorded_by_the_cell() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let b = handler(&app).await;
    let payment = |amount: i64| json!({"bedrag": amount, "datum_betaling": "2025-03-12"});

    // A decision is not reported.
    let case = consumer_submit(&app, "12345678").await;
    let (status, f) = report(&app, &b, &case, "besluit", verdicts()["form"].clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{f}");
    let (status, _) = action(
        &app,
        &b,
        &case,
        "besluit",
        false,
        verdicts()["form"].clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // An announcement not made in the prescribed manner: reported, it is
    // recorded, without an objection period.
    let (status, body) = report(
        &app,
        &b,
        &case,
        "bekendmaken",
        json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": false}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["stage"], "BEKENDMAKING");
    assert_eq!(
        body["gram"]["fields"]["aanvang_bezwaartermijn"],
        Value::Null
    );
    assert_eq!(body["gram"]["fields"]["einde_bezwaartermijn"], Value::Null);
    assert!(
        body["warnings"][0]
            .as_str()
            .unwrap()
            .contains("reported as happened"),
        "{body}"
    );
    let (_, l, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    let r = &l["decisions"][0]["legal_protection"];
    assert_eq!(r["stage"], "BEZWAAR", "{l}");
    assert_eq!(r["outputs"]["einde_bezwaartermijn"], Value::Null, "{l}");

    // Paying: the amount, and then one cent too much.
    let (status, body) = action(&app, &b, &case, "betalen", false, payment(6000)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, p) = action(&app, &b, &case, "betalen", true, payment(1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["takeable"], json!(false), "{p}");
    assert_eq!(p["reportable"], json!(true), "{p}");
    assert_eq!(p["assessments"]["betaling_conform"], json!(false), "{p}");
    let (status, body) = report(&app, &b, &case, "betalen", payment(1)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["type"], "executogram");
    assert_eq!(body["gram"]["fields"]["bedrag"], json!(1));
    // The consequences: the cell adds up the payments, the law says what was
    // paid unduly.
    let (_, p) = action(&app, &b, &case, "betalen", true, payment(0)).await;
    assert_eq!(p["parameters"]["betaald_bedrag"], json!(6001), "{p}");
    assert_eq!(p["outputs"]["onverschuldigd_betaald"], json!(1), "{p}");

    // Nobody records a fact without a filled-in form, not even when reported.
    let (status, f) = report(&app, &b, &case, "aanvulling_vragen", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(f["error"].as_str().unwrap().contains("fill in"), "{f}");
}

/// The moment of an action: not in the future and not before the case. The
/// process says so on trial; the cell also refuses such a gram itself.
#[tokio::test]
async fn a_moment_does_not_lie_before_the_case_or_in_the_future() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), data.path()).unwrap();
    let app = as_reader(&rt);
    let b = handler(&app).await;
    let case = consumer_submit(&app, "12345678").await;
    let decision = |date: &str| json!({"besluitdatum": date, "feiten_vergaard": true});

    let (_, p) = action(&app, &b, &case, "besluit", true, decision("2025-03-11")).await;
    assert_eq!(p["takeable"], json!(false), "{p}");
    assert_eq!(p["reportable"], json!(false), "{p}");
    assert!(
        p["reason"]
            .as_str()
            .unwrap()
            .contains("lies before the case"),
        "{p}"
    );
    let (status, f) = action(&app, &b, &case, "besluit", false, decision("2025-03-11")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    let (_, p) = action(&app, &b, &case, "besluit", true, decision("2025-03-13")).await;
    assert!(
        p["reason"].as_str().unwrap().contains("lies after today"),
        "{p}"
    );
    // The same day as the application is allowed: it is about the day.
    let (_, p) = action(&app, &b, &case, "besluit", true, decision("2025-03-12")).await;
    assert_eq!(p["takeable"], json!(true), "{p}");

    // The cell itself: a decision that refers to the application, with an
    // earlier day than the application.
    let (status, f) = as_runtime(
        &rt,
        "POST",
        &format!("{CONSUMER_CELL}/api/grams"),
        json!({
            "actor": "test_afnemer",
            "stream": "test_afnemer_zaakverloop",
            "event": "besluit_genomen",
            "external": {"besluitdatum": "2025-03-01"},
            "refers_to": {"on_application": case},
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("is before the gram it refers to"),
        "{f}"
    );
}

/// The state of a case is a lexostatus of the cell that the runtime offers:
/// the stages with what their gram recorded, the count per event, and on
/// request whether someone knows the case.
#[tokio::test]
async fn the_cell_gives_the_state_of_a_case() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), data.path()).unwrap();
    let app = as_reader(&rt);
    let b = handler(&app).await;
    let case = consumer_submit(&app, "12345678").await;
    let (status, _) = action(
        &app,
        &b,
        &case,
        "besluit",
        false,
        verdicts()["form"].clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let read = |q: String| {
        let rt = &rt;
        async move {
            let headers = [(RUNTIME_TOKEN_HEADER, rt.runtime_token.as_str())];
            let (status, body, _) = call_with(
                &rt.router,
                "GET",
                &format!("{CONSUMER_CELL}/api/lexostatus/case_state?{q}"),
                &headers,
                None,
            )
            .await;
            (status, body)
        }
    };
    let (status, l) = read(format!("root={case}")).await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(l["name"], "case_state");
    assert_eq!(l["parameters"], json!({}), "never goes to the engine");
    let z = &l["extra_fields"];
    assert_eq!(z["grams"], json!(2), "{z}");
    assert_eq!(
        z["events"]["test_afnemer_zaakverloop/besluit_genomen"],
        json!(1)
    );
    // The decision is a gram of its own that refers to the application; the
    // application is the root.
    let decision = &z["decisions"][0];
    assert!(
        decision["id"].is_string() && decision["id"] != case.as_str(),
        "{z}"
    );
    assert_eq!(decision["event"], "besluit_genomen");
    assert_eq!(
        decision["stages"]["BESLUIT"]["fields"]["vastgesteld_bedrag"],
        json!(6000),
        "{z}"
    );
    assert!(decision["stages"]["BESLUIT"]["input"].is_object(), "{z}");
    assert!(z["stages"]["AANVRAAG"].is_object(), "{z}");
    assert!(z["stages"].get("BESLUIT").is_none(), "{z}");
    assert!(z.get("owner").is_none());

    let (_, l) = read(format!(
        "root={case}&owner_path=eherkenning.kvk&owner=12345678"
    ))
    .await;
    assert_eq!(l["extra_fields"]["owner"], json!(true), "{l}");
    let (_, l) = read(format!(
        "root={case}&owner_path=eherkenning.kvk&owner=87654321"
    ))
    .await;
    assert_eq!(l["extra_fields"]["owner"], json!(false), "{l}");
    let (status, _) = read("root=bestaat-niet".into()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = read(format!("root={case}&owner=12345678")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The cell lists it among its lexostatuses, as a lexostatus of the runtime.
    let (_, cells, _) = call(&app, "GET", "/api/cells", None, None).await;
    let consumer = cells
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "test_afnemer")
        .unwrap();
    assert!(
        consumer["lexostatuses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["name"] == "case_state" && l["runtime"] == json!(true)),
        "{consumer}"
    );
}

/// The read token gives reading, not recording.
#[tokio::test]
async fn the_read_token_gives_only_reading() {
    let data = tempfile::tempdir().unwrap();
    let token = "gedeeld-leestoken-van-de-test";
    let rt = runtime_with_read_token(&fixtures(), data.path(), token, &[]);
    let read = [(READ_TOKEN_HEADER, token)];
    let (status, body, _) = call_with(
        &rt.router,
        "GET",
        &format!("{AGENCY_CELL}/api/chronicle"),
        &read,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, _) = call_with(
        &rt.router,
        "POST",
        &format!("{AGENCY_CELL}/api/grams"),
        &read,
        Some(record_request("test_instantie")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// A handler sees the chronicle and the lexostatuses of the cells their
/// process reads, via the process; an applicant and someone not logged in do
/// not.
#[tokio::test]
async fn inspection_of_the_cells_via_the_process() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), data.path()).unwrap();
    let app = rt.router.clone();
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let chronicle = format!("{CONSUMER}/api/inspection/test_afnemer/chronicle");

    let (status, grams, _) = call(&app, "GET", &chronicle, Some(&b), None).await;
    assert_eq!(status, StatusCode::OK, "{grams}");
    assert_eq!(grams.as_array().unwrap().len(), 1);
    let (status, l, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/inspection/test_afnemer/lexostatus/aanvraag_inhoud?root={case}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(l["root"], json!(case));
    // A source of the process in this runtime is allowed too; another cell is not.
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/inspection/test_register/chronicle"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/inspection/test_instantie/chronicle"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Without login 401, as applicant 403.
    let (status, _, _) = call(&app, "GET", &chronicle, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, _, a) = call(
        &app,
        "POST",
        &format!("{CONSUMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    let (status, _, _) = call(&app, "GET", &chronicle, a.as_deref(), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // The process names the cells a handler can inspect.
    let (_, processes, _) = call(&app, "GET", "/api/processes", None, None).await;
    let p = processes
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "test_afnemer_proces")
        .unwrap();
    assert_eq!(
        p["inspection"],
        json!(["test_afnemer", "test_gebieden", "test_register"])
    );
}

// --- A second case study: a monthly allowance, with several decisions in a case ---
//
// The same binary, the same routes: only the configuration and the
// regulations differ (processes/toeslag, cells/toeslag,
// regulation/testregeling_toeslag and testbeleid_toeslag).

const TOESLAG: &str = "/processes/test_toeslag_proces";
const TOESLAG_CELL: &str = "/cells/test_toeslag";

async fn toeslag_logins(app: &Router) -> String {
    let (status, body, cookie) = call(
        app,
        "POST",
        &format!("{TOESLAG}/api/channels/persoon/login"),
        None,
        Some(json!({"nummer": "123456789"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    cookie.unwrap()
}

/// Submit an application for a monthly allowance; returns the case it opens.
async fn toeslag_submit(app: &Router, month: &str, estimated_income: i64) -> String {
    let a = toeslag_logins(app).await;
    let (status, body, _) = call(
        app,
        "POST",
        &format!("{TOESLAG}/api/application"),
        Some(&a),
        Some(json!({"external": {
            "adres_aanvrager": "Voorbeeldstraat 1, 1234 AB Voorbeeld",
            "dagtekening": "2025-03-12",
            "maand": month,
            "geschat_inkomen": estimated_income,
        }})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["gram"]["id"].as_str().unwrap().to_string()
}

/// An action in an allowance case; `happened` reports a fact that happened
/// anyway.
async fn toeslag(
    app: &Router,
    b: &str,
    case: &str,
    name: &str,
    form: Value,
    happened: bool,
) -> (StatusCode, Value) {
    let body = json!({"form": form, "happened": happened});
    action_in(app, TOESLAG, b, case, name, false, body).await
}

fn notification() -> Value {
    json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": true})
}

/// The window is a month: the policy offers months (as their first day), a
/// month that has yet to begin is taken as of its start, and the cell derives
/// the month of the application with periode_van, with the period the
/// regulation names (temporal.period_type: month).
#[tokio::test]
async fn a_month_as_window() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let a = toeslag_logins(&app).await;
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{TOESLAG}/api/possibilities"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let m = body["possibilities"].as_array().unwrap();
    let months: Vec<&Value> = m
        .iter()
        .map(|m| &m["possibility"]["window"]["value"])
        .collect();
    assert_eq!(
        months,
        [
            &json!("2025-03-01"),
            &json!("2025-04-01"),
            &json!("2025-05-01")
        ]
    );
    assert_eq!(m[0]["possibility"]["window"]["parameter"], "maand");
    assert_eq!(m[0]["possibility"]["window"]["field"], "maand");
    assert_eq!(m[0]["possibility"]["verdict"], "possible", "{body}");
    assert_eq!(m[0]["possibility"]["deadline"], "2025-04-30");
    // The current month is taken as of today, a coming one as of its first day.
    assert_eq!(m[0]["as_of"], "2025-03-12", "{}", m[0]);
    assert!(
        m[1]["as_of"].as_str().unwrap().starts_with("2025-04-01"),
        "{}",
        m[1]
    );
    // The cell derives the month from the form: a day in the month is the
    // month, as its first day.
    assert_eq!(m[1]["parameters"]["maand"], "2025-04-01");

    let case = toeslag_submit(&app, "2025-03-17", 90000).await;
    let b = handler_in(&app, TOESLAG).await;
    let (status, p) = action_in(
        &app,
        TOESLAG,
        &b,
        &case,
        "voorschot",
        true,
        json!({"form": {"voorschotdatum": "2025-03-12"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["parameters"]["maand"], "2025-03-01", "{p}");
    assert_eq!(p["provenance"]["maand"]["source"], "own", "{p}");
}

/// A case with four decisions: an advance, a determination, an amendment of
/// that determination and a recovery, each announced with its own objection
/// period, with the payment of the advance and the repayment of what was
/// recovered. The cell refuses a second determination without grounds for
/// amendment. Not a line of code differs from the consumer.
#[tokio::test]
async fn several_decisions_in_one_case() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_at(&fixtures(), data.path()).unwrap();
    let app = as_reader(&rt);
    let case = toeslag_submit(&app, "2025-03-01", 90000).await;
    let b = handler_in(&app, TOESLAG).await;

    // Before the advance: announcing and paying wait for the decision.
    let (status, f) = toeslag(
        &app,
        &b,
        &case,
        "voorschot_bekendmaken",
        notification(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("waiting for the decision (Voorschot verlenen)"),
        "{f}"
    );

    // 1. The advance: the first decision in the case.
    let (status, v) = toeslag(
        &app,
        &b,
        &case,
        "voorschot",
        json!({"voorschotdatum": "2025-03-12"}),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["gram"]["refers_to"], json!({"on_application": case}));
    let k1 = v["gram"]["id"].as_str().unwrap().to_string();
    assert_eq!(v["gram"]["fields"]["voorschot"], json!(12000));
    // A second advance in the same case: no legal basis of its own.
    let (status, f) = toeslag(
        &app,
        &b,
        &case,
        "voorschot",
        json!({"voorschotdatum": "2025-03-12"}),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    let (status, bm) = toeslag(
        &app,
        &b,
        &case,
        "voorschot_bekendmaken",
        notification(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bm}");
    assert_eq!(bm["gram"]["refers_to"]["decision"], k1.as_str());
    assert_eq!(bm["gram"]["fields"]["einde_bezwaartermijn"], "2025-04-23");
    // A decision is announced once.
    let (status, f) = toeslag(
        &app,
        &b,
        &case,
        "voorschot_bekendmaken",
        notification(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("is already in decision"),
        "{f}"
    );
    let payment = json!({"bedrag": 12000, "datum_betaling": "2025-03-12"});
    let (status, bt) = toeslag(&app, &b, &case, "voorschot_betalen", payment, false).await;
    assert_eq!(status, StatusCode::CREATED, "{bt}");
    assert_eq!(bt["gram"]["refers_to"]["decision"], k1.as_str());
    assert_eq!(bt["trial"]["outputs"]["nog_te_betalen_voorschot"], json!(0));

    // 2. The determination: a second decision, from an article of its own.
    let determination = json!({"vastgesteld_inkomen": 150000, "vaststellingsdatum": "2025-03-12"});
    let (status, vs) = toeslag(&app, &b, &case, "vaststellen", determination.clone(), false).await;
    assert_eq!(status, StatusCode::CREATED, "{vs}");
    let k2 = vs["gram"]["id"].as_str().unwrap().to_string();
    assert_eq!(vs["gram"]["refers_to"]["on_application"], case.as_str());
    assert_eq!(vs["gram"]["fields"]["vastgestelde_toeslag"], json!(6000));
    // A second determination without grounds for amendment: the trial says
    // so, and the cell also refuses such a gram itself.
    let (status, f) = toeslag(&app, &b, &case, "vaststellen", determination, false).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"].as_str().unwrap().contains("its own legal basis"),
        "{f}"
    );
    let (status, f) = as_runtime(
        &rt,
        "POST",
        &format!("{TOESLAG_CELL}/api/grams"),
        json!({
            "actor": "test_toeslagdienst",
            "stream": "test_toeslag_zaakverloop",
            "event": "toeslag_vastgesteld",
            "external": {"vastgestelde_toeslag": 1, "vaststellingsdatum": "2025-03-12"},
            "refers_to": {"on_application": case},
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"].as_str().unwrap().contains("an amends reference"),
        "{f}"
    );
    // A gram that follows a decision names a decision that is in the case.
    let notification_of = |decision: Option<String>| {
        let mut v = json!({
            "actor": "test_toeslagdienst",
            "stream": "test_toeslag_zaakverloop",
            "event": "besluit_bekendgemaakt",
            "external": {"datum_bekendmaking": "2025-03-12", "bekendgemaakt": true},
        });
        if let Some(b) = decision {
            v["refers_to"] = json!({"decision": b});
        }
        v
    };
    for (decision, message) in [
        (
            Some("00000000-0000-4000-8000-000000000009".to_string()),
            "no gram",
        ),
        (None, "must refer with 'decision'"),
    ] {
        let (status, f) = as_runtime(
            &rt,
            "POST",
            &format!("{TOESLAG_CELL}/api/grams"),
            notification_of(decision),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{f}");
        assert!(f["error"].as_str().unwrap().contains(message), "{f}");
    }
    let (status, bm) = toeslag(
        &app,
        &b,
        &case,
        "vaststelling_bekendmaken",
        notification(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bm}");
    assert_eq!(bm["gram"]["refers_to"]["decision"], k2.as_str());

    // 3. The amendment of the determination: a decision of its own with its
    // own legal basis. Without new facts there is nothing to amend.
    let amendment = |new: bool| json!({"gecorrigeerd_inkomen": 250000, "nieuwe_feiten": new, "wijzigingsdatum": "2025-03-12"});
    let (status, f) = toeslag(
        &app,
        &b,
        &case,
        "vaststelling_wijzigen",
        amendment(false),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("gives no value for gewijzigde_toeslag"),
        "{f}"
    );
    let (status, w) = toeslag(
        &app,
        &b,
        &case,
        "vaststelling_wijzigen",
        amendment(true),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{w}");
    let k3 = w["gram"]["id"].as_str().unwrap().to_string();
    assert_eq!(w["gram"]["refers_to"], json!({"amends": k2}));
    assert_eq!(w["gram"]["fields"]["vastgestelde_toeslag"], json!(0));
    assert_eq!(w["trial"]["decision"]["id"], k2.as_str());
    let (status, bm) = toeslag(
        &app,
        &b,
        &case,
        "wijziging_bekendmaken",
        notification(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bm}");
    assert_eq!(bm["gram"]["refers_to"]["decision"], k3.as_str());

    // An action acts on the latest decision, unless the handler names one: a
    // second amendment can amend the original determination. The process
    // refuses a decision the action does not act on.
    let amend = |id: &str| json!({"form": amendment(true), "decision": id});
    let (status, p) = action_in(
        &app,
        TOESLAG,
        &b,
        &case,
        "vaststelling_wijzigen",
        true,
        json!({"form": amendment(true)}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["decision"]["id"], k3.as_str());
    let (status, p) = action_in(
        &app,
        TOESLAG,
        &b,
        &case,
        "vaststelling_wijzigen",
        true,
        amend(&k2),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["decision"]["id"], k2.as_str());
    let (status, p) = action_in(
        &app,
        TOESLAG,
        &b,
        &case,
        "vaststelling_wijzigen",
        true,
        amend(&k1),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["takeable"], json!(false), "{p}");
    assert!(
        p["reason"]
            .as_str()
            .unwrap()
            .contains("is not a decision that action 'vaststelling_wijzigen' acts on"),
        "{p}"
    );

    // 4. The recovery: the advance minus the allowance as currently
    // determined.
    let (status, t) = toeslag(
        &app,
        &b,
        &case,
        "terugvorderen",
        json!({"terugvorderingsdatum": "2025-03-12"}),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{t}");
    let k4 = t["gram"]["id"].as_str().unwrap().to_string();
    assert_eq!(t["gram"]["refers_to"]["on_application"], case.as_str());
    assert_eq!(t["gram"]["decision_type"], "BETALINGSVERPLICHTING");
    assert_eq!(t["gram"]["fields"]["terug_te_vorderen"], json!(12000));
    let (status, bm) = toeslag(
        &app,
        &b,
        &case,
        "terugvordering_bekendmaken",
        notification(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bm}");

    // The repayment executes the recovery (after Awb 4:57).
    let back = |amount: i64| json!({"bedrag": amount, "datum_terugbetaling": "2025-03-12"});
    let (status, tb) = toeslag(&app, &b, &case, "terugbetalen", back(5000), false).await;
    assert_eq!(status, StatusCode::CREATED, "{tb}");
    assert_eq!(tb["gram"]["refers_to"]["decision"], k4.as_str());
    assert_eq!(tb["trial"]["outputs"]["nog_terug_te_betalen"], json!(7000));
    // The type and the unit come from the regulation, not from the name.
    assert_eq!(
        tb["trial"]["types"]["nog_terug_te_betalen"],
        json!({"type": "amount", "unit": "eurocent"})
    );
    assert_eq!(
        tb["trial"]["types"]["terugbetaling_conform"],
        json!({"type": "boolean"})
    );
    let (status, f) = toeslag(&app, &b, &case, "terugbetalen", back(7001), false).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "above the recovered amount: {f}"
    );
    let (status, tb) = toeslag(&app, &b, &case, "terugbetalen", back(7001), true).await;
    assert_eq!(status, StatusCode::CREATED, "reported as happened: {tb}");

    // The case screen: four decisions, each with its stages, its route and
    // its actions.
    let (status, z, _) = call(
        &app,
        "GET",
        &format!("{TOESLAG}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{z}");
    let decisions = z["decisions"].as_array().unwrap();
    let actions: Vec<&str> = decisions
        .iter()
        .map(|b| b["action"].as_str().unwrap())
        .collect();
    assert_eq!(
        actions,
        [
            "voorschot",
            "vaststellen",
            "vaststelling_wijzigen",
            "terugvorderen"
        ]
    );
    for (i, decision) in decisions.iter().enumerate() {
        assert_eq!(decision["id"], [&k1, &k2, &k3, &k4][i].as_str());
        let r = &decision["legal_protection"];
        assert_eq!(r["stage"], "BEZWAAR", "{decision}");
        assert_eq!(
            r["outputs"]["einde_bezwaartermijn"], "2025-04-23",
            "{decision}"
        );
        let stages: Vec<(&str, bool)> = decision["procedure"]["stages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s["name"].as_str().unwrap(),
                    s["recorded"].as_bool().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            stages,
            [
                ("AANVRAAG", true),
                ("BESLUIT", true),
                ("BEKENDMAKING", true),
                ("BEZWAAR", false)
            ]
        );
    }
    assert_eq!(decisions[2]["amends"], k2.as_str());
    assert_eq!(
        decisions[0]["actions"],
        json!(["voorschot_bekendmaken", "voorschot_betalen"])
    );
    assert_eq!(
        decisions[3]["actions"],
        json!(["terugvordering_bekendmaken", "terugbetalen"])
    );
    let repay = z["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "terugbetalen")
        .unwrap();
    assert_eq!(repay["decision"], k4.as_str());
    assert_eq!(repay["recorded"], json!(2));
    let amount = repay["form"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "bedrag")
        .unwrap();
    assert_eq!(amount["type"], "amount", "{amount}");
    assert_eq!(amount["unit"], "eurocent", "{amount}");
    assert_eq!(
        repay["trial"]["outputs"]["nog_terug_te_betalen"],
        json!(0),
        "{repay}"
    );
    let determine = z["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "vaststellen")
        .unwrap();
    assert_eq!(determine["available"], json!(false));

    // The case state of the cell: the decisions, each with its stages.
    let (status, l, _) = call(
        &app,
        "GET",
        &format!("{TOESLAG_CELL}/api/lexostatus/case_state?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    let zs = &l["extra_fields"];
    assert_eq!(zs["decisions"].as_array().unwrap().len(), 4, "{zs}");
    assert_eq!(
        zs["decisions"][0]["events"]["test_toeslag_zaakverloop/voorschot_betaald"],
        json!(1)
    );
    assert_eq!(
        zs["decisions"][3]["events"]["test_toeslag_zaakverloop/terugbetaling_ontvangen"],
        json!(2)
    );
    assert_eq!(
        zs["decisions"][1]["stages"]["BEKENDMAKING"]["fields"]["einde_bezwaartermijn"],
        "2025-04-23"
    );
    assert_eq!(
        zs["stages"].as_object().unwrap().keys().collect::<Vec<_>>(),
        ["AANVRAAG"]
    );
}

// --- Experiment A: the engine route (CELL_REDUCTION) ---

/// A runtime over the fixtures with this reduction mode.
fn runtime_with_reduction(data: &Path, reduction: ReductionMode) -> Result<Runtime, Vec<String>> {
    let config = Config {
        cells_path: fixtures().join("cells"),
        processes_path: Some(fixtures().join("processes")),
        regulation_path: fixtures().join("regulation"),
        data_dir: data.to_path_buf(),
        port: DEFAULT_PORT,
        read_token: None,
        read_token_sources: Vec::new(),
        reduction,
        registers: None,
    };
    Runtime::load(&config, clock())
}

fn binding() -> PathBuf {
    fixtures().join("experiment/engine/koppeling.yaml")
}

/// What a response says, without what differs per run (timestamps, hashes)
/// and without the route of the reduction.
fn without_run(w: &Value) -> Value {
    const GONE: &[&str] = &[
        "effective_at",
        "recorded_at",
        "reduction",
        "yaml",
        "sha256",
        "receipt",
        "trace_text",
    ];
    match w {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, _)| !GONE.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), without_run(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(without_run).collect()),
        Value::String(t) => Value::String(without_uuid(t)),
        _ => w.clone(),
    }
}

/// A text with every reference (a 36-character uuid) as `<kenmerk>`.
fn without_uuid(t: &str) -> String {
    let is_uuid = |w: &str| {
        w.len() == 36
            && w.char_indices().all(|(i, c)| {
                if [8, 13, 18, 23].contains(&i) {
                    c == '-'
                } else {
                    c.is_ascii_hexdigit()
                }
            })
    };
    let mut out = String::new();
    let mut i = 0;
    while i < t.len() {
        if t.is_char_boundary(i)
            && t.len() - i >= 36
            && t.is_char_boundary(i + 36)
            && is_uuid(&t[i..i + 36])
        {
            out.push_str("<kenmerk>");
            i += 36;
        } else {
            let c = t[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// The whole way through the fixtures: assessment and submission at the agency
/// and the consumer, and at the consumer the decision (with the synthesis per
/// row), the announcement, the payment and the case. Every response, without
/// what differs per run.
async fn fixtureflow(rt: &Runtime) -> Vec<(String, StatusCode, Value)> {
    let app = as_reader(rt);
    let mut out = Vec::new();
    let mut note = |step: &str, status: StatusCode, w: &Value| {
        out.push((step.to_string(), status, without_run(w)));
    };
    let c = logins(&app, "12345678").await;
    let (s, w, _) = call(
        &app,
        "POST",
        &format!("{AGENCY}/api/application/assessment"),
        Some(&c),
        Some(complete()),
    )
    .await;
    note("assessment agency", s, &w);
    for a in [Some("VOORBEELD"), None] {
        let w = consumer_assessment(&app, a).await;
        note("assessment consumer", StatusCode::OK, &w);
    }
    let case = consumer_submit(&app, "12345678").await;
    let b = handler(&app).await;
    let (s, w) = action(&app, &b, &case, "aanvulling_vragen", true, json!({})).await;
    note("supplement on trial", s, &w);
    let (s, w) = action(&app, &b, &case, "besluit", true, verdicts()["form"].clone()).await;
    note("decision on trial", s, &w);
    let (s, w) = action(
        &app,
        &b,
        &case,
        "besluit",
        false,
        verdicts()["form"].clone(),
    )
    .await;
    note("decision", s, &w);
    let (s, w) = action(
        &app,
        &b,
        &case,
        "bekendmaken",
        false,
        json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": true}),
    )
    .await;
    note("announce", s, &w);
    let (s, w) = action(
        &app,
        &b,
        &case,
        "betalen",
        false,
        json!({"bedrag": 6000, "datum_betaling": "2025-03-12"}),
    )
    .await;
    note("pay", s, &w);
    let (s, w, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    note("case", s, &w);
    let (s, w, _) = call(
        &app,
        "GET",
        &format!("{CONSUMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    note("worklist", s, &w);
    out
}

/// The engine route gives the same responses as the reduction DSL all the
/// way. In `compare` every cell also reduces via the DSL and every
/// difference is an error, so a difference in a source that the synthesis
/// would silently show as an error stands out here too.
#[tokio::test]
async fn the_engine_route_gives_the_same_outputs_as_the_dsl() {
    let (d1, d2, d3) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let dsl = fixtureflow(&runtime_with_reduction(d1.path(), ReductionMode::Dsl).unwrap()).await;
    for (d, compare) in [(&d2, false), (&d3, true)] {
        let rt = runtime_with_reduction(
            d.path(),
            ReductionMode::Engine {
                binding: binding(),
                compare,
            },
        )
        .unwrap();
        let engine = fixtureflow(&rt).await;
        for ((step, s1, w1), (_, s2, w2)) in dsl.iter().zip(&engine) {
            assert_eq!((s1, w1), (s2, w2), "{step} (compare: {compare})");
        }
    }
    // The consumer's decision: 4 x 1000 + 2 x 500 and the rest, here too.
    let decision = &dsl
        .iter()
        .find(|(s, _, _)| s == "decision on trial")
        .unwrap()
        .2;
    assert_eq!(
        decision["outputs"]["gebiedsbedrag"],
        json!(5000),
        "{decision}"
    );
}

/// In a runtime with the engine route every lexostatus says which route it
/// came by, with the regulation; the description of the cell says it per
/// lexostatus, also for a lexostatus that deliberately goes via the DSL; on
/// request the trace of the engine run comes along.
#[tokio::test]
async fn the_engine_route_is_visible() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_with_reduction(
        data.path(),
        ReductionMode::Engine {
            binding: binding(),
            compare: false,
        },
    )
    .unwrap();
    let app = as_reader(&rt);
    let (_, l, _) = call(
        &app,
        "GET",
        "/cells/test_register/api/lexostatus/registerstatus?aanduiding=VOORBEELD&engine_trace=1",
        None,
        None,
    )
    .await;
    assert_eq!(l["reduction"]["route"], "engine", "{l}");
    assert_eq!(l["reduction"]["regulation"], "lexostatus_registerstatus");
    assert!(
        l["reduction"]["trace_text"]
            .as_str()
            .is_some_and(|t| !t.is_empty()),
        "{l}"
    );
    assert_eq!(l["parameters"]["zetels_toegewezen"], json!(6));
    let (_, cells, _) = call(&app, "GET", "/api/cells", None, None).await;
    let consumer = cells
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "test_afnemer")
        .unwrap();
    assert_eq!(consumer["reduction"], "engine");
    let worklist = consumer["lexostatuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "werkvoorraad")
        .unwrap();
    assert_eq!(worklist["reduction"]["route"], "dsl");
    // The case state is runtime code, not a reduction: that is shown too.
    let case_state = consumer["lexostatuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["runtime"] == json!(true))
        .unwrap();
    assert_eq!(case_state["reduction"]["route"], "runtime");
    // Without the engine route the description says nothing about it.
    let d = tempfile::tempdir().unwrap();
    let (_, cells, _) = call(&app_dsl(d.path()), "GET", "/api/cells", None, None).await;
    assert!(cells[0].get("reduction").is_none(), "{cells}");
}

fn app_dsl(data: &Path) -> Router {
    as_reader(&runtime_with_reduction(data, ReductionMode::Dsl).unwrap())
}

/// No silent fallback: a lexostatus without a binding, a regulation without
/// the output of a derivation, or a list via the engine stops the runtime,
/// with every error.
#[test]
fn an_incomplete_binding_stops_the_runtime() {
    let map = tempfile::tempdir().unwrap();
    let source = std::fs::read_to_string(binding()).unwrap();
    let broken = source
        .replace("    tarief: gebieden_tarief.yaml\n", "    {}\n")
        .replace(
            "registratie_per_gebied: register_registratie_per_gebied.yaml",
            "registratie_per_gebied: register_register.yaml",
        )
        .replace(
            "    werkvoorraad:\n      dsl: een lijst met een regel per zaak (groepeer); de engine kent geen groeperen\n",
            "    werkvoorraad: afnemer_besluit.yaml\n",
        );
    // The regulations stay where they are: their paths become absolute.
    let at = binding().parent().unwrap().display().to_string();
    let broken = broken
        .replace("  test_gebieden:\n    {}\n", "  test_gebieden: {}\n")
        .replace(": ../", &format!(": {at}/../"))
        .lines()
        .map(|r| match r.split_once(": ") {
            Some((k, v)) if v.ends_with(".yaml") && !v.starts_with('/') => {
                format!("{k}: {at}/{v}")
            }
            _ => r.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let path = map.path().join("koppeling.yaml");
    std::fs::write(&path, broken).unwrap();
    let data = tempfile::tempdir().unwrap();
    let errors = runtime_with_reduction(
        data.path(),
        ReductionMode::Engine {
            binding: path,
            compare: false,
        },
    )
    .err()
    .unwrap()
    .join("\n");
    assert!(
        errors.contains("cell 'test_gebieden', lexostatus 'tarief': no binding"),
        "{errors}"
    );
    assert!(errors.contains("has no output 'ingeschreven'"), "{errors}");
    assert!(
        errors.contains("lexostatus 'werkvoorraad': a list lexostatus"),
        "{errors}"
    );
}

#[test]
fn the_reduction_mode_comes_from_the_environment() {
    assert_eq!(ReductionMode::out(None, None), Ok(ReductionMode::Dsl));
    assert_eq!(
        ReductionMode::out(Some("dsl"), None),
        Ok(ReductionMode::Dsl)
    );
    assert_eq!(
        ReductionMode::out(Some("engine"), Some("k.yaml")),
        Ok(ReductionMode::Engine {
            binding: PathBuf::from("k.yaml"),
            compare: false
        })
    );
    assert!(ReductionMode::out(Some("engine"), None).is_err());
    assert!(ReductionMode::out(None, Some("k.yaml")).is_err());
    assert!(ReductionMode::out(Some("anders"), Some("k.yaml")).is_err());
}

// The gram from the law (note "het gram uit de wet", 29-09-2026): the stream of
// the allowance holds only the registration; the fields come from the law.

/// Log in with a person number on the allowance portal.
async fn toeslag_login_as(app: &Router, number: &str) -> String {
    let (status, body, cookie) = call(
        app,
        "POST",
        &format!("{TOESLAG}/api/channels/persoon/login"),
        None,
        Some(json!({"nummer": number})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    cookie.unwrap()
}

/// The field of the form with this name.
fn form_field<'f>(form: &'f Value, name: &str) -> Option<&'f Value> {
    form["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == name)
}

/// The fixed core of an application comes from the fictitious Awb (art. 9, a
/// hook on the stage AANVRAAG), the content from the allowance regulation
/// (art. 1), and what the channel supplies from the policy (art. 4): the
/// stream names none of it. The decision requested is fixed by what art. 1
/// requests; the portal channel supplies the signature and the person
/// number.
#[tokio::test]
async fn the_application_gram_comes_from_the_law() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let a = toeslag_login_as(&app, "123456789").await;
    let (status, form, _) = call(&app, "GET", &format!("{TOESLAG}/api/form"), Some(&a), None).await;
    assert_eq!(status, StatusCode::OK, "{form}");
    let names: Vec<&str> = form["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    for n in [
        "naam_aanvrager",
        "adres_aanvrager",
        "dagtekening",
        "ondertekening",
        "maand",
        "geschat_inkomen",
    ] {
        assert!(names.contains(&n), "{n} not in {names:?}");
    }
    // The decision requested is no question: the law fixes it.
    assert!(!names.contains(&"gevraagde_beschikking"), "{names:?}");
    // What the channel supplies comes along, with the policy that says so.
    let o = form_field(&form, "ondertekening").unwrap();
    assert_eq!(o["supplied"]["value"], "123456789", "{o}");
    assert_eq!(o["supplied"]["source"], "channel");
    // The label and the legal basis come from the law; the Awb field may
    // be left out (required: false).
    let d = form_field(&form, "adres_aanvrager").unwrap();
    assert_eq!(d["label"], "Adres van de aanvrager", "{d}");
    assert_eq!(d["legal_basis"], json!(["testregeling_awb#9 lid 1"]));
    assert_eq!(d["optional"], json!(true));
    // The stream document shows per field where it comes from.
    let (_, stream, _) = call(
        &app,
        "GET",
        &format!("{TOESLAG_CELL}/api/stream"),
        None,
        None,
    )
    .await;
    let e = &stream["streams"][0]["stream"]["events"][0];
    assert_eq!(e["type"], "submission", "{e}");
    assert_eq!(e["stage"], "AANVRAAG");
    let via: Vec<(String, String)> = e["field_sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["name"].as_str().unwrap().into(),
                f["via"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert!(
        via.contains(&("dagtekening".into(), "hook".into())),
        "{via:?}"
    );
    assert!(
        via.contains(&("maand".into(), "establishes".into())),
        "{via:?}"
    );
    assert!(
        via.contains(&("kanaal".into(), "extends".into())),
        "{via:?}"
    );
    // The combination is an execution (RFC-046): running art. 1, the
    // engine fires the fictitious Awb art. 9 as a hook on the application,
    // because art. 2 and 3 decide on it and are beschikkingen.
    let (_, assessed, _) = call(
        &app,
        "POST",
        &format!("{TOESLAG}/api/application/assessment"),
        Some(&a),
        Some(json!({"external": {"maand": "2025-03-01", "geschat_inkomen": 90000}})),
    )
    .await;
    let trace = assessed["result"]["trace_text"]
        .as_str()
        .unwrap_or_default();
    assert!(trace.contains("testregeling_awb:9"), "{assessed}");

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{TOESLAG}/api/application"),
        Some(&a),
        Some(json!({"external": {"maand": "2025-03-01", "geschat_inkomen": 90000, "dagtekening": "2025-03-12"}})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let g = &body["gram"];
    assert_eq!(g["subtype"], "aanvraag");
    assert_eq!(
        g["fields"]["gevraagde_beschikking"],
        "testregeling_toeslag#2, testregeling_toeslag#3"
    );
    assert_eq!(g["fields"]["ondertekening"], "123456789");
    assert_eq!(g["fields"]["kanaal"], "portaal");
    assert_eq!(g["field_provenance"]["ondertekening"]["source"], "channel");
    let lb: Vec<&str> = g["legal_basis"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(lb[0], "testregeling_toeslag#1", "{lb:?}");
    assert!(lb.contains(&"testregeling_awb#9 lid 1"), "{lb:?}");
    schema::validate(Kind::Gram, g).unwrap();
}

/// Decision 3 of 29-09-2026: a register fact is in the gram every time, and
/// the gram records where it came from. Known to the register (the policy
/// fills the name in beforehand from the person register): provenance
/// register. Not known: the applicant fills it in, provenance applicant. The
/// assessment of the application gives the same in both cases.
#[tokio::test]
async fn a_register_fact_with_and_without_the_register() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let application = |naam: Option<&str>| {
        let mut e = json!({"maand": "2025-03-01", "geschat_inkomen": 90000, "adres_aanvrager": "Voorbeeldstraat 1", "dagtekening": "2025-03-12"});
        if let Some(n) = naam {
            e["naam_aanvrager"] = json!(n);
        }
        json!({"external": e})
    };

    // The register knows person 123456789.
    let a = toeslag_login_as(&app, "123456789").await;
    let (_, form, _) = call(&app, "GET", &format!("{TOESLAG}/api/form"), Some(&a), None).await;
    let n = form_field(&form, "naam_aanvrager").unwrap();
    assert_eq!(n["supplied"]["value"], "A. Voorbeeld", "{n}");
    assert_eq!(n["supplied"]["source"], "register");
    // The form shows the trace of the run that supplied the value ...
    assert!(
        n["supplied"]["trace_text"]
            .as_str()
            .is_some_and(|t| t.contains("testbeleid_toeslag")),
        "{n}"
    );
    let (_, known, _) = call(
        &app,
        "POST",
        &format!("{TOESLAG}/api/application/assessment"),
        Some(&a),
        Some(application(None)),
    )
    .await;
    assert_eq!(known["result"]["value"], json!(true), "{known}");
    // The applicant may not change what the register supplies.
    let (status, f, _) = call(
        &app,
        "POST",
        &format!("{TOESLAG}/api/application"),
        Some(&a),
        Some(application(Some("C. Anders"))),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("supplied by the register"),
        "{f}"
    );
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{TOESLAG}/api/application"),
        Some(&a),
        Some(application(None)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["fields"]["naam_aanvrager"], "A. Voorbeeld");
    // ... the gram does not carry it.
    assert!(!body.to_string().contains("trace_text"), "{body}");
    assert_eq!(
        body["gram"]["field_provenance"]["naam_aanvrager"],
        json!({"source": "register", "legal_basis": ["testbeleid_toeslag#5"]})
    );

    // The register does not know person 111222333: the applicant fills it in.
    let b = toeslag_login_as(&app, "111222333").await;
    let (_, form, _) = call(&app, "GET", &format!("{TOESLAG}/api/form"), Some(&b), None).await;
    assert!(
        form_field(&form, "naam_aanvrager")
            .unwrap()
            .get("supplied")
            .is_none(),
        "{form}"
    );
    let (_, unknown, _) = call(
        &app,
        "POST",
        &format!("{TOESLAG}/api/application/assessment"),
        Some(&b),
        Some(application(Some("B. Voorbeeld"))),
    )
    .await;
    assert_eq!(
        unknown["result"]["value"], known["result"]["value"],
        "{unknown}"
    );
    // Without the name the application cannot be judged: the field is
    // needed, wherever it comes from.
    let (_, without, _) = call(
        &app,
        "POST",
        &format!("{TOESLAG}/api/application/assessment"),
        Some(&b),
        Some(application(None)),
    )
    .await;
    assert_eq!(without["result"]["to_assess"], json!(false), "{without}");
    assert_eq!(
        without["result"]["missing"],
        json!(["naam_aanvrager"]),
        "{without}"
    );
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{TOESLAG}/api/application"),
        Some(&b),
        Some(application(Some("B. Voorbeeld"))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["fields"]["naam_aanvrager"], "B. Voorbeeld");
    assert_eq!(
        body["gram"]["field_provenance"]["naam_aanvrager"]["source"],
        "applicant"
    );
}

/// The fragment routes give the block the cell loaded, with file and line;
/// an unknown article, an unknown configuration or a file the runtime did
/// not load is 404.
#[tokio::test]
async fn a_step_opens_to_its_yaml() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let (status, f, _) = call(
        &app,
        "GET",
        &format!("{TOESLAG}/api/law/testregeling_awb/9"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{f}");
    assert_eq!(f["file"], "regulation/testregeling_awb/2025-01-01.yaml");
    assert!(
        f["yaml"].as_str().unwrap().starts_with("  - number: '9'"),
        "{f}"
    );
    assert!(f["line"].as_u64().unwrap() < f["end_line"].as_u64().unwrap());
    let (status, f, _) = call(
        &app,
        "GET",
        &format!("{TOESLAG}/api/config/stream/test_toeslag_aanvragen?anchor=aanvraag_ontvangen"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{f}");
    assert_eq!(f["file"], "chronicles/test_toeslag_aanvragen.yaml");
    assert!(f["yaml"]
        .as_str()
        .unwrap()
        .contains("name: aanvraag_ontvangen"));
    let (status, f, _) = call(
        &app,
        "GET",
        &format!("{TOESLAG}/api/config/process?anchor=portal"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{f}");
    assert_eq!(f["file"], "processes/toeslag/process.yaml");
    // The form file of a process with a form: a field, and without an anchor
    // the whole file.
    let (status, f, _) = call(
        &app,
        "GET",
        &format!("{AGENCY}/api/config/form?anchor=naam"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{f}");
    assert_eq!(f["file"], "processes/instantie/formulier.yaml");
    assert!(f["yaml"].as_str().unwrap().contains("{id: naam,"), "{f}");
    let (status, f, _) = call(
        &app,
        "GET",
        &format!("{AGENCY}/api/config/form"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{f}");
    assert_eq!(
        (f["line"].as_u64(), f["end_line"].as_u64()),
        (Some(1), Some(33))
    );
    for uri in [
        "/api/config/form",
        "/api/law/testregeling_awb/99",
        "/api/law/bestaat_niet/1",
        "/api/config/stream/bestaat_niet",
        "/api/config/../../etc/passwd",
        "/api/config/process?anchor=bestaat_niet",
    ] {
        let (status, _, _) = call(&app, "GET", &format!("{TOESLAG}{uri}"), None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}

/// Copy a directory tree.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

/// A runtime over the fixtures with its own regulation directory and clock.
fn runtime_with(regulation: &Path, data: &Path, now: &'static str) -> Runtime {
    let config = Config {
        cells_path: fixtures().join("cells"),
        processes_path: Some(fixtures().join("processes")),
        regulation_path: regulation.to_path_buf(),
        data_dir: data.to_path_buf(),
        port: DEFAULT_PORT,
        read_token: None,
        read_token_sources: Vec::new(),
        reduction: Default::default(),
        registers: None,
    };
    Runtime::load(
        &config,
        Arc::new(move || DateTime::parse_from_rfc3339(now).unwrap()),
    )
    .unwrap()
}

/// A change of the general law reaches every application without touching
/// a stream or a form: the fictitious Awb gets, per 1 January 2026, a
/// telephone number in art. 9 (a second version of the regulation). An
/// application on the portal on 5 January 2026 has the field in its form and
/// its gram; one on 20 December 2025 does not. The stream is the same file
/// (the same hash in the gram).
#[tokio::test]
async fn a_change_of_the_general_law_reaches_the_application() {
    // Not a hidden directory: the corpus loader skips those.
    let regulation = tempfile::Builder::new()
        .prefix("regulation")
        .tempdir()
        .unwrap();
    copy_tree(&fixtures().join("regulation"), regulation.path());
    let dir = regulation.path().join("testregeling_awb");
    let old = std::fs::read_to_string(dir.join("2025-01-01.yaml")).unwrap();
    let phone = "          - name: telefoon_aanvrager\n            type: string\n            nullable: true\n            required: false\n            description: 'Naam: Telefoonnummer van de aanvrager.'\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_awb#9 lid 1}\n        output:\n          - name: aanvraag_bevat_kern\n";
    let new = old
        .replace("valid_from: '2025-01-01'", "valid_from: '2026-01-01'")
        .replace(
            "        output:\n          - name: aanvraag_bevat_kern\n",
            phone,
        );
    assert_ne!(new, old);
    std::fs::write(dir.join("2026-01-01.yaml"), new).unwrap();

    let mut hashes = Vec::new();
    for (now, month, expected) in [
        ("2025-12-20T10:00:00+01:00", "2025-12-01", false),
        ("2026-01-05T10:00:00+01:00", "2026-01-01", true),
    ] {
        let data = tempfile::tempdir().unwrap();
        let app = as_reader(&runtime_with(regulation.path(), data.path(), now));
        let a = toeslag_login_as(&app, "123456789").await;
        let (_, form, _) = call(&app, "GET", &format!("{TOESLAG}/api/form"), Some(&a), None).await;
        assert_eq!(
            form_field(&form, "telefoon_aanvrager").is_some(),
            expected,
            "{now}: {form}"
        );
        let mut external = json!({"maand": month, "geschat_inkomen": 90000});
        if expected {
            external["telefoon_aanvrager"] = json!("010-1234567");
        }
        let (status, body, _) = call(
            &app,
            "POST",
            &format!("{TOESLAG}/api/application"),
            Some(&a),
            Some(json!({"external": external})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{now}: {body}");
        let fields = body["gram"]["fields"].as_object().unwrap();
        assert_eq!(
            fields.contains_key("telefoon_aanvrager"),
            expected,
            "{now}: {fields:?}"
        );
        hashes.push(body["gram"]["stream"]["sha256"].clone());
    }
    assert_eq!(hashes[0], hashes[1]);
}

/// The kinds of the steps of a chain, in order.
fn kinds(steps: &Value) -> Vec<&str> {
    steps
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect()
}

/// Every source a `why` of the form names resolves to a fragment.
async fn every_source_resolves(app: &Router, process: &str, form: &Value) {
    let mut steps: Vec<&Value> = form["why"]["event"].as_array().unwrap().iter().collect();
    for f in form["fields"].as_array().unwrap() {
        for part in ["here", "value"] {
            steps.extend(f["why"][part].as_array().unwrap());
        }
    }
    for s in steps {
        let source = &s["source"];
        let uri = match source["law"].as_str() {
            Some(law) => format!("{process}/api/law/{}", law.replace('#', "/")),
            None => format!(
                "{process}/api/config/{}?anchor={}",
                source["config"].as_str().unwrap(),
                source["anchor"].as_str().unwrap()
            ),
        };
        let (status, body, _) = call(app, "GET", &uri, None, None).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
}

/// The form says per field why it is there and why it has its value, and
/// for the form as a whole the chain from the portal to the presentation.
/// It is the same composition as the stream document: no second copy.
#[tokio::test]
async fn the_form_says_why() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let a = toeslag_login_as(&app, "123456789").await;
    let (_, form, _) = call(&app, "GET", &format!("{TOESLAG}/api/form"), Some(&a), None).await;
    let chain = kinds(&form["why"]["event"]);
    assert_eq!(chain.first(), Some(&"process"), "{chain:?}");
    assert_eq!(
        chain[1..4],
        ["stream", "submission", "decides_on"],
        "{chain:?}"
    );
    assert!(
        chain.contains(&"hook") && chain.contains(&"origin"),
        "{chain:?}"
    );
    // Without a form file the presentation comes from the law and the stream.
    assert_eq!(chain.last(), Some(&"presentation"), "{chain:?}");
    assert!(form["why"]["excluded"].is_array(), "{form}");
    // Via the hook of the general law.
    let adres = form_field(&form, "adres_aanvrager").unwrap();
    assert!(kinds(&adres["why"]["here"]).contains(&"hook"), "{adres}");
    assert_eq!(adres["why"]["value"][0]["kind"], "origin");
    assert!(adres["why"]["value"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("BELANGHEBBENDE"));
    // Without a form file the label comes from the law.
    let last = adres["why"]["here"].as_array().unwrap().last().unwrap();
    assert_eq!(last["kind"], "presentation", "{adres}");
    assert!(last["source"]["law"].is_string(), "{adres}");
    // Prefilled from the register.
    let naam = form_field(&form, "naam_aanvrager").unwrap();
    assert!(kinds(&naam["why"]["value"]).contains(&"prefill"), "{naam}");
    // The channel supplies the signature: the policy overrides the origin,
    // the channel names what it supplies.
    let o = form_field(&form, "ondertekening").unwrap();
    assert_eq!(
        kinds(&o["why"]["value"]),
        ["origin", "origin", "supply"],
        "{o}"
    );
    let supply = o["why"]["value"].as_array().unwrap().last().unwrap();
    assert_eq!(
        supply["source"],
        json!({"config": "process", "anchor": "persoon"}),
        "{o}"
    );
    every_source_resolves(&app, TOESLAG, &form).await;
    // The stream document carries the same chain of the event.
    let (_, stream, _) = call(
        &app,
        "GET",
        &format!("{TOESLAG_CELL}/api/stream"),
        None,
        None,
    )
    .await;
    assert!(stream.to_string().contains("\"explanation\""), "{stream}");

    // With a form file: order, groups and labels come from the form.
    let (_, form, _) = call(&app, "GET", &format!("{AGENCY}/api/form"), None, None).await;
    let chain = form["why"]["event"].as_array().unwrap();
    let last = chain.last().unwrap();
    assert_eq!(last["kind"], "presentation", "{last}");
    assert_eq!(last["source"]["config"], "form", "{last}");
    let naam = form_field(&form, "naam").unwrap();
    let last = naam["why"]["here"].as_array().unwrap().last().unwrap();
    assert_eq!(
        last["source"],
        json!({"config": "form", "anchor": "naam"}),
        "{naam}"
    );
    every_source_resolves(&app, AGENCY, &form).await;
}

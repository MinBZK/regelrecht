//! De routes, door de echte router, op de generieke fixtures.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use chrono::DateTime;
use http_body_util::BodyExt;
use regelrecht_cel::api::{router, AppState, Klok};
use regelrecht_cel::config::{Cel, Config, STANDAARD_POORT};
use regelrecht_cel::eherkenning::Sessies;
use regelrecht_cel::kroniek::Kroniek;
use regelrecht_cel::schema::{self, Soort};
use serde_json::{json, Value};
use tower::ServiceExt;

fn app(data: &Path) -> Router {
    let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let config = Config {
        regulation_path: f.join("regulation"),
        chronicles_path: f.join("chronicles"),
        cell_config_path: f.join("cel/lexostatussen.yaml"),
        data_dir: data.to_path_buf(),
        port: STANDAARD_POORT,
    };
    let klok: Klok =
        Arc::new(|| DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap());
    router(AppState {
        cel: Arc::new(Cel::laad(&config).unwrap()),
        kroniek: Arc::new(Kroniek::open(data).unwrap()),
        sessies: Arc::new(Sessies::default()),
        klok,
    })
}

async fn vraag(
    app: &Router,
    methode: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder().method(methode).uri(uri);
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
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
    let waarde = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, waarde, cookie)
}

async fn inloggen(app: &Router, kvk: &str) -> String {
    let (status, _, cookie) = vraag(
        app,
        "POST",
        "/api/eherkenning/login",
        None,
        Some(json!({"kvk": kvk, "persoon": "A. Tester", "machtiging": "volledig"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    cookie.unwrap()
}

fn volledig() -> Value {
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
async fn login_weigert_ongeldige_kvk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(
        &app,
        "POST",
        "/api/eherkenning/login",
        None,
        Some(json!({"kvk": "123", "persoon": "A", "machtiging": "volledig"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["fout"].as_str().unwrap().contains("acht cijfers"));
}

#[tokio::test]
async fn stroom_geeft_velden_met_labels() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(&app, "GET", "/api/stroom", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["event"], "aanvraag_ontvangen");
    assert_eq!(body["velden"][0]["label"], "Naam van de aanvrager");
    schema::valideer(Soort::Stroom, &body["stroom"]).unwrap();
}

#[tokio::test]
async fn toets_zonder_login_mag_niet() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, _, _) = vraag(&app, "POST", "/api/aanvraag/toets", None, Some(volledig())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn toets_volledig_en_onvolledig_zonder_vastleggen() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = inloggen(&app, "12345678").await;

    let (status, body, _) = vraag(
        &app,
        "POST",
        "/api/aanvraag/toets",
        Some(&c),
        Some(volledig()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["uitslag"]["waarde"], json!(true), "{body}");
    assert_eq!(
        body["lexostatus"]["parameters"]["aanvraagdatum"],
        "2025-03-12"
    );

    let mut onvolledig = volledig();
    onvolledig["external"]
        .as_object_mut()
        .unwrap()
        .remove("aanduiding");
    let (_, body, _) = vraag(
        &app,
        "POST",
        "/api/aanvraag/toets",
        Some(&c),
        Some(onvolledig),
    )
    .await;
    assert_eq!(body["uitslag"]["waarde"], json!(false), "{body}");
    assert_eq!(body["uitslag"]["ontbreekt"], json!(["bevat_aanduiding"]));

    let mut zonder_jaar = volledig();
    zonder_jaar["external"]
        .as_object_mut()
        .unwrap()
        .remove("aanvraagjaar");
    let (_, body, _) = vraag(
        &app,
        "POST",
        "/api/aanvraag/toets",
        Some(&c),
        Some(zonder_jaar),
    )
    .await;
    assert_eq!(body["uitslag"]["te_beoordelen"], json!(false));
    assert_eq!(
        body["uitslag"]["reden"],
        "niet te beoordelen: mist aanvraagjaar"
    );

    // Een toets legt niets vast.
    let (_, kroniek, _) = vraag(&app, "GET", "/api/kroniek", Some(&c), None).await;
    assert_eq!(kroniek, json!([]));
}

#[tokio::test]
async fn onbekend_veld_wordt_geweigerd() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = inloggen(&app, "12345678").await;
    let (status, body, _) = vraag(
        &app,
        "POST",
        "/api/aanvraag",
        Some(&c),
        Some(json!({"external": {"schoenmaat": 44}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["fout"].as_str().unwrap().contains("schoenmaat"));
}

#[tokio::test]
async fn onbekende_tabelkolom_wordt_geweigerd() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = inloggen(&app, "12345678").await;
    let mut body = volledig();
    body["external"]["organen"][1]["kleur"] = json!("rood");
    let (status, antwoord, _) = vraag(&app, "POST", "/api/aanvraag", Some(&c), Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        antwoord["fout"]
            .as_str()
            .unwrap()
            .contains("onbekend veld 'organen[1].kleur'"),
        "{antwoord}"
    );
    // Er is niets vastgelegd.
    let (_, kroniek, _) = vraag(&app, "GET", "/api/kroniek", Some(&c), None).await;
    assert_eq!(kroniek, json!([]));
}

#[tokio::test]
async fn indienen_legt_een_gram_vast_per_kvk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = inloggen(&app, "12345678").await;

    let (status, body, _) = vraag(&app, "POST", "/api/aanvraag", Some(&c), Some(volledig())).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gram = &body["gram"];
    schema::valideer(Soort::Gram, gram).unwrap();
    assert_eq!(gram["type"], "indiening");
    assert_eq!(gram["soort"], "aanvraag");
    assert_eq!(
        gram["fields"]["kern"]["ondertekend_via"]["kvk_nummer"],
        "12345678"
    );
    let yaml = body["yaml"].as_str().unwrap();
    assert!(
        yaml.starts_with("kind: chronolexogram\ntype: indiening\nsoort: aanvraag\n"),
        "{yaml}"
    );
    // Velden in de volgorde van de stroom: kern voor inhoud.
    assert!(yaml.find("kern:").unwrap() < yaml.find("inhoud:").unwrap());
    let zaak = gram["zaakkenmerk"].as_str().unwrap().to_string();

    // Een onvolledige aanvraag wordt ook vastgelegd, in een nieuwe zaak.
    let mut onvolledig = volledig();
    onvolledig["external"]
        .as_object_mut()
        .unwrap()
        .remove("adres");
    let (status, body, _) = vraag(&app, "POST", "/api/aanvraag", Some(&c), Some(onvolledig)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        body["gram"]["fields"]["kern"]["aanvrager"]["adres"],
        Value::Null
    );
    assert_ne!(body["gram"]["zaakkenmerk"], json!(zaak));

    let (_, kroniek, _) = vraag(&app, "GET", "/api/kroniek", Some(&c), None).await;
    assert_eq!(kroniek.as_array().unwrap().len(), 2);
    for item in kroniek.as_array().unwrap() {
        schema::valideer(Soort::Gram, &item["gram"]).unwrap();
    }
    let regels = std::fs::read_to_string(dir.path().join("test_kroniek.jsonl")).unwrap();
    assert_eq!(regels.lines().count(), 2);

    let (status, lexo, _) = vraag(
        &app,
        "GET",
        &format!("/api/lexostatus/aanvraag_inhoud?zaakkenmerk={zaak}"),
        Some(&c),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lexo}");
    assert_eq!(lexo["parameters"]["bevat_naam"], json!(true));

    // Een andere KvK ziet deze grammen en deze zaak niet.
    let ander = inloggen(&app, "87654321").await;
    let (_, kroniek, _) = vraag(&app, "GET", "/api/kroniek", Some(&ander), None).await;
    assert_eq!(kroniek, json!([]));
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("/api/lexostatus/aanvraag_inhoud?zaakkenmerk={zaak}"),
        Some(&ander),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn onbekende_lexostatus() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = inloggen(&app, "12345678").await;
    let (status, _, _) = vraag(
        &app,
        "GET",
        "/api/lexostatus/bestaat_niet?zaakkenmerk=x",
        Some(&c),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test]
fn fixtures_valideren_tegen_de_schemas() {
    let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (bestand, soort) in [
        ("chronicles/test_aanvragen.yaml", Soort::Stroom),
        ("cel/lexostatussen.yaml", Soort::Lexostatus),
    ] {
        let doc: Value =
            serde_yaml_ng::from_str(&std::fs::read_to_string(f.join(bestand)).unwrap()).unwrap();
        let uitslag = schema::valideer(soort, &doc);
        assert!(uitslag.is_ok(), "{bestand}: {uitslag:?}");
    }
}

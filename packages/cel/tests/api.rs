//! De routes, door de echte router, op de generieke fixtures.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use chrono::DateTime;
use http_body_util::BodyExt;
use regelrecht_cel::api::Klok;
use regelrecht_cel::config::{Config, STANDAARD_POORT};
use regelrecht_cel::runtime::Runtime;
use regelrecht_cel::schema::{self, Soort};
use regelrecht_cel::transport::RUNTIME_TOKEN_HEADER;
use serde_json::{json, Value};
use tower::ServiceExt;

/// Het proces met een portaal, en de cel waarin het vastlegt.
const INSTANTIE: &str = "/processen/test_instantie_proces";
const INSTANTIE_CEL: &str = "/cellen/test_instantie";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn klok() -> Klok {
    Arc::new(|| DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap())
}

/// Een runtime over `<opstelling>/cellen` en `<opstelling>/processes`.
fn runtime_op(opstelling: &Path, data: &Path) -> Result<Runtime, Vec<String>> {
    let config = Config {
        cells_path: opstelling.join("cellen"),
        processes_path: Some(opstelling.join("processes")),
        regulation_path: fixtures().join("regulation"),
        data_dir: data.to_path_buf(),
        port: STANDAARD_POORT,
    };
    Runtime::laad(&config, klok())
}

fn app(data: &Path) -> Router {
    runtime_op(&fixtures(), data).unwrap().router
}

async fn vraag(
    app: &Router,
    methode: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let headers: Vec<(&str, &str)> = cookie.map(|c| ("cookie", c)).into_iter().collect();
    vraag_met(app, methode, uri, &headers, body).await
}

/// Een verzoek aan een cel zoals een proces van de runtime het doet: met het
/// runtime-token.
async fn als_runtime(rt: &Runtime, methode: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let headers = [(RUNTIME_TOKEN_HEADER, rt.runtime_token.als_str())];
    let (status, body, _) = vraag_met(&rt.router, methode, uri, &headers, Some(body)).await;
    (status, body)
}

async fn vraag_met(
    app: &Router,
    methode: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder().method(methode).uri(uri);
    for (naam, waarde) in headers {
        req = req.header(*naam, *waarde);
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
        &format!("{INSTANTIE}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": kvk, "persoon": "A. Tester"})),
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
        &format!("{INSTANTIE}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": "123", "persoon": "A"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["fout"].as_str().unwrap().contains("acht cijfers"));
}

#[tokio::test]
async fn formulier_geeft_velden_met_labels() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/formulier"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cel"], "test_instantie");
    assert_eq!(body["event"], "aanvraag_ontvangen");
    assert_eq!(body["velden"][0]["label"], "Naam van de aanvrager");
    schema::valideer(Soort::Stroom, &body["stroom"]).unwrap();
}

#[tokio::test]
async fn de_cel_geeft_haar_stromen_met_hun_hash() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(&app, "GET", "/cellen/test_afnemer/api/stroom", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<&str> = body["strommen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["test_afnemer_aanvragen", "test_afnemer_zaakverloop"]);
    for s in body["strommen"].as_array().unwrap() {
        assert_eq!(s["sha256"].as_str().unwrap().len(), 64);
        schema::valideer(Soort::Stroom, &s["stroom"]).unwrap();
    }
}

#[tokio::test]
async fn toets_zonder_login_mag_niet() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/aanvraag/toets"),
        None,
        Some(volledig()),
    )
    .await;
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
        &format!("{INSTANTIE}/api/aanvraag/toets"),
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
        &format!("{INSTANTIE}/api/aanvraag/toets"),
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
        &format!("{INSTANTIE}/api/aanvraag/toets"),
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
    let (_, kroniek, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/kroniek"),
        None,
        None,
    )
    .await;
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
        &format!("{INSTANTIE}/api/aanvraag"),
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
    let (status, antwoord, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/aanvraag"),
        Some(&c),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        antwoord["fout"]
            .as_str()
            .unwrap()
            .contains("onbekend veld 'organen[1].kleur'"),
        "{antwoord}"
    );
    // Er is niets vastgelegd.
    let (_, kroniek, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/kroniek"),
        None,
        None,
    )
    .await;
    assert_eq!(kroniek, json!([]));
}

#[tokio::test]
async fn indienen_legt_een_gram_vast_per_kvk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = inloggen(&app, "12345678").await;

    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/aanvraag"),
        Some(&c),
        Some(volledig()),
    )
    .await;
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
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/aanvraag"),
        Some(&c),
        Some(onvolledig),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        body["gram"]["fields"]["kern"]["aanvrager"]["adres"],
        Value::Null
    );
    assert_ne!(body["gram"]["zaakkenmerk"], json!(zaak));

    // De kroniek en de lexostatussen zijn van de cel, zonder login: tussen
    // een afnemer en de cel is geen beveiligingscontext.
    let (_, kroniek, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/kroniek"),
        None,
        None,
    )
    .await;
    assert_eq!(kroniek.as_array().unwrap().len(), 2);
    for item in kroniek.as_array().unwrap() {
        schema::valideer(Soort::Gram, &item["gram"]).unwrap();
    }
    let regels =
        std::fs::read_to_string(dir.path().join("test_instantie/test_kroniek.jsonl")).unwrap();
    assert_eq!(regels.lines().count(), 2);

    let (status, lexo, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud?zaakkenmerk={zaak}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lexo}");
    assert_eq!(lexo["parameters"]["bevat_naam"], json!(true));

    // Het proces heeft geen kroniek en geen lexostatus.
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/kroniek"),
        Some(&c),
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
        &format!("{INSTANTIE_CEL}/api/lexostatus/bestaat_niet?zaakkenmerk=x"),
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
        ("chronicles/test_registers.yaml", Soort::Stroom),
        ("chronicles/test_afnemer_aanvragen.yaml", Soort::Stroom),
        ("cellen/instantie/lexostatussen.yaml", Soort::Lexostatus),
        ("cellen/register/lexostatussen.yaml", Soort::Lexostatus),
        ("cellen/afnemer/lexostatussen.yaml", Soort::Lexostatus),
        ("cellen/instantie/cel.yaml", Soort::Cel),
        ("cellen/register/cel.yaml", Soort::Cel),
        ("cellen/afnemer/cel.yaml", Soort::Cel),
        ("cellen/gebieden/cel.yaml", Soort::Cel),
        ("processes/instantie/proces.yaml", Soort::Proces),
        ("processes/afnemer/proces.yaml", Soort::Proces),
    ] {
        let doc: Value =
            serde_yaml_ng::from_str(&std::fs::read_to_string(f.join(bestand)).unwrap()).unwrap();
        let uitslag = schema::valideer(soort, &doc);
        assert!(uitslag.is_ok(), "{bestand}: {uitslag:?}");
    }
}

// --- De runtime: meer cellen, elk met een eigen kroniek ---

#[tokio::test]
async fn cellen_worden_opgesomd_met_hun_mogelijkheden() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(&app, "GET", "/api/cellen", None, None).await;
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
            "test_register"
        ]
    );
    let register = &body[3];
    assert_eq!(register["recording_actor"], "test_register");
    assert_eq!(register["lexostatussen"][0]["name"], "registerstatus");
    assert_eq!(
        register["lexostatussen"][0]["inputs"][0]["name"],
        "aanduiding"
    );
    assert_eq!(
        body[0]["lexostatussen"][0]["extra_velden"],
        json!(["aanduiding", "gebieden"])
    );
    // Een cel zegt niets over wie er handelt: dat staat bij het proces.
    for sleutel in ["portaal", "rollen", "behandeling", "synthese"] {
        assert!(body[0].get(sleutel).is_none(), "{sleutel}");
    }
}

#[tokio::test]
async fn processen_worden_opgesomd_met_hun_cel() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(&app, "GET", "/api/processen", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let afnemer = &body[0];
    assert_eq!(afnemer["id"], "test_afnemer_proces");
    assert_eq!(afnemer["actor"], "test_afnemer");
    assert_eq!(afnemer["cel"], "test_afnemer");
    assert_eq!(afnemer["portaal"], json!(true));
    // De eigen cel is een bron zoals de andere, voor de zaak.
    assert_eq!(
        afnemer["synthese"][0],
        json!({"cel": "test_afnemer", "lexostatus": "aanvraag_inhoud", "zaak": true, "transport": "intern", "parameters": []})
    );
    assert_eq!(afnemer["synthese"][2]["cel"], "test_register");
    assert_eq!(afnemer["synthese"][2]["transport"], "intern");
    let instantie = &body[1];
    assert_eq!(instantie["id"], "test_instantie_proces");
    assert_eq!(instantie["behandeling"], Value::Null);
    assert_eq!(
        instantie["rollen"],
        json!({"aanvrager": true, "behandelaar": false})
    );
}

#[tokio::test]
async fn een_cel_heeft_geen_login_of_aanvraag() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    for (methode, pad) in [
        ("POST", "/cellen/test_register/api/eherkenning/login"),
        ("POST", "/cellen/test_register/api/aanvraag/toets"),
        ("POST", "/cellen/test_register/api/aanvraag"),
        ("POST", "/cellen/test_instantie/api/eherkenning/login"),
        ("GET", "/cellen/test_instantie/api/voorbeelden"),
        ("GET", "/cellen/test_afnemer/api/werkvoorraad"),
    ] {
        let (status, _, _) = vraag(&app, methode, pad, None, Some(json!({}))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{methode} {pad}");
    }
}

#[tokio::test]
async fn startstand_in_een_lege_kroniek_en_niet_nog_eens() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, kroniek, _) =
        vraag(&app, "GET", "/cellen/test_register/api/kroniek", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let grammen = kroniek.as_array().unwrap();
    assert_eq!(grammen.len(), 4);
    for g in grammen {
        assert_eq!(g["gram"]["herkomst"], "startstand");
        // Een registerbesluit hoort bij geen zaak.
        assert!(g["gram"].get("zaakkenmerk").is_none(), "{g}");
        assert!(!g["yaml"].as_str().unwrap().contains("zaakkenmerk"));
        schema::valideer(Soort::Gram, &g["gram"]).unwrap();
    }
    // De kroniek staat in de eigen map van de cel.
    let pad = dir.path().join("test_register/test_register.jsonl");
    assert_eq!(std::fs::read_to_string(&pad).unwrap().lines().count(), 4);
    // Een tweede start voegt niets toe: de kroniek is niet meer leeg.
    drop(app);
    let _ = self::app(dir.path());
    assert_eq!(std::fs::read_to_string(&pad).unwrap().lines().count(), 4);
    // De andere cellen hebben een eigen, lege kroniek.
    assert!(!dir
        .path()
        .join("test_instantie/test_register.jsonl")
        .exists());
}

#[tokio::test]
async fn lexostatus_met_invoer_als_query() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, l, _) = vraag(
        &app,
        "GET",
        "/cellen/test_register/api/lexostatus/registerstatus?aanduiding=VOORBEELD",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(
        l["parameters"],
        json!({"is_ingeschreven_raad": true, "is_geschrapt_raad": false, "jaar": 2024,
               "zetels_op_lijst": 6, "datum_mededeling": "2024-11-01", "geblokkeerd_raad": false})
    );
    let (status, l, _) = vraag(
        &app,
        "GET",
        "/cellen/test_register/api/lexostatus/registerstatus",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(l["fout"], "input 'aanduiding' ontbreekt");
}

fn afnemer_concept(aanduiding: Option<&str>) -> Value {
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

async fn afnemer_toets(app: &Router, aanduiding: Option<&str>) -> Value {
    let (status, _, cookie) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/aanvraag/toets"),
        cookie.as_deref(),
        Some(afnemer_concept(aanduiding)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[tokio::test]
async fn synthese_intern_met_herkomst_per_parameter() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let body = afnemer_toets(&app, Some("VOORBEELD")).await;
    assert_eq!(body["uitslag"]["waarde"], json!(true), "{body}");
    assert_eq!(
        body["herkomst"]["bevat_aanduiding"],
        json!({"bron": "eigen", "lexostatus": "aanvraag_inhoud"})
    );
    assert_eq!(
        body["herkomst"]["zetels_op_lijst"],
        json!({"bron": "cel", "cel": "test_register", "lexostatus": "registerstatus", "transport": "intern"})
    );
    assert_eq!(body["bronnen"][0]["status"], "bevraagd");
    assert_eq!(
        body["bronnen"][0]["invoer"],
        json!({"aanduiding": "VOORBEELD"})
    );
    // De aanduiding is geen parameter: ze staat apart in de eigen lexostatus.
    assert_eq!(
        body["lexostatus"]["extra_velden"]["aanduiding"],
        "VOORBEELD"
    );
    assert!(body["herkomst"].get("aanduiding").is_none());
    // Niets uit de synthese wordt vastgelegd, niet bij de afnemer en niet bij
    // het register.
    assert!(!dir.path().join("test_afnemer/test_afnemer.jsonl").exists());
    let regels = std::fs::read_to_string(dir.path().join("test_register/test_register.jsonl"));
    assert_eq!(regels.unwrap().lines().count(), 4);
}

#[tokio::test]
async fn synthese_zonder_registratie_en_zonder_invoer() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    // Een onbekende aanduiding: niet ingeschreven en geen zetels, dus niet
    // toelaatbaar. Een mededeling is er niet; die datum vult niemand aan.
    let body = afnemer_toets(&app, Some("ONBEKEND")).await;
    assert_eq!(body["uitslag"]["waarde"], json!(false), "{body}");
    assert_eq!(body["herkomst"]["is_ingeschreven_raad"]["bron"], "cel");
    assert_eq!(
        body["bronnen"][0]["niet_geleverd"],
        json!(["datum_mededeling", "jaar"])
    );
    assert!(body["herkomst"].get("datum_mededeling").is_none());
    // Zonder aanduiding wordt de bron niet bevraagd.
    let body = afnemer_toets(&app, None).await;
    assert_eq!(body["bronnen"][0]["status"], "niet_bevraagd");
    assert_eq!(body["uitslag"]["ontbreekt"], json!(["bevat_aanduiding"]));
    assert!(body["uitslag"]["reden"]
        .as_str()
        .unwrap()
        .starts_with("niet te beoordelen: bron test_register niet bevraagd"));
}

/// Een aanpassing aan een `cel.yaml` of `proces.yaml`.
type Aanpassing<'a> = (&'a str, &'a dyn Fn(String) -> String);

/// Kopieer fixture-cellen, fixture-processen en alle stromen naar een eigen
/// opstelling (`cellen/`, `processes/`, `chronicles/`), met een aanpassing
/// aan elke `cel.yaml` en `proces.yaml`.
fn eigen_opstelling(cellen: &[Aanpassing], processen: &[Aanpassing]) -> tempfile::TempDir {
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
    for (soort, lijst, bestand) in [
        ("cellen", cellen, "cel.yaml"),
        ("processes", processen, "proces.yaml"),
    ] {
        for (naam, pas_aan) in lijst {
            let doel = dir.path().join(soort).join(naam);
            std::fs::create_dir_all(&doel).unwrap();
            for e in std::fs::read_dir(f.join(soort).join(naam)).unwrap() {
                let p = e.unwrap().path();
                let tekst = std::fs::read_to_string(&p).unwrap();
                let tekst = if p.file_name().unwrap() == bestand {
                    pas_aan(tekst)
                } else {
                    tekst
                };
                std::fs::write(doel.join(p.file_name().unwrap()), tekst).unwrap();
            }
        }
    }
    dir
}

fn zo(t: String) -> String {
    t
}

fn met_url(url: String) -> impl Fn(String) -> String {
    move |t: String| {
        // Alleen de synthese-bron, niet de gelijknamige bron onder rijen.
        t.replace(
            "  - cel: test_register\n    lexostatus: registerstatus\n",
            &format!("  - cel: test_register\n    url: {url}\n    lexostatus: registerstatus\n"),
        )
    }
}

#[tokio::test]
async fn synthese_over_http_naar_een_andere_runtime() {
    // Runtime B: de fixtures, op een echte poort.
    let data_b = tempfile::tempdir().unwrap();
    let b = app(data_b.path());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let adres = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, b).await });

    // Runtime A: alleen de afnemer, met een url naar B.
    let aanpassing = met_url(format!("http://{adres}"));
    let opstelling = eigen_opstelling(&[("afnemer", &zo)], &[("afnemer", &aanpassing)]);
    let data_a = tempfile::tempdir().unwrap();
    let a = runtime_op(opstelling.path(), data_a.path()).unwrap();
    assert!(
        a.waarschuwingen().await.is_empty(),
        "{:?}",
        a.waarschuwingen().await
    );
    let body = afnemer_toets(&a.router, Some("VOORBEELD")).await;
    assert_eq!(body["uitslag"]["waarde"], json!(true), "{body}");
    assert_eq!(
        body["herkomst"]["is_ingeschreven_raad"]["transport"],
        "http"
    );
    assert_eq!(body["bronnen"][0]["transport"], "http");
}

#[tokio::test]
async fn onbereikbare_bron_maakt_de_toets_niet_te_beoordelen() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let adres = listener.local_addr().unwrap();
    drop(listener);
    let aanpassing = met_url(format!("http://{adres}"));
    let opstelling = eigen_opstelling(&[("afnemer", &zo)], &[("afnemer", &aanpassing)]);
    let data = tempfile::tempdir().unwrap();
    // De bron mag later komen: de runtime start wel, met een waarschuwing.
    let a = runtime_op(opstelling.path(), data.path()).unwrap();
    let w = a.waarschuwingen().await;
    assert!(
        w.iter().any(|w| w.contains("nu niet te controleren")),
        "{w:?}"
    );
    let body = afnemer_toets(&a.router, Some("VOORBEELD")).await;
    assert_eq!(body["uitslag"]["te_beoordelen"], json!(false));
    assert_eq!(
        body["uitslag"]["reden"],
        "niet te beoordelen: bron test_register onbereikbaar"
    );
    assert_eq!(body["bronnen"][0]["status"], "onbereikbaar");
    // Er is niets aangevuld.
    assert!(body["herkomst"].get("is_ingeschreven_raad").is_none());
}

#[tokio::test]
async fn interne_bron_die_niet_draait_is_een_waarschuwing() {
    let opstelling = eigen_opstelling(&[("afnemer", &zo)], &[("afnemer", &zo)]);
    let data = tempfile::tempdir().unwrap();
    let a = runtime_op(opstelling.path(), data.path()).unwrap();
    let w = a.waarschuwingen().await;
    assert!(
        w.iter()
            .any(|w| w.contains("draait niet in deze runtime en heeft geen url")),
        "{w:?}"
    );
    assert!(
        w.iter().any(|w| w.contains("geen cel 'test_register'")),
        "{w:?}"
    );
}

#[tokio::test]
async fn bron_zonder_de_verwachte_parameter_is_een_waarschuwing() {
    // Een parameter die de afnemer verwacht maar de bron niet levert: haal
    // hem weg uit de lexostatus van de bron.
    let cellen = eigen_opstelling(&[("afnemer", &zo), ("register", &zo)], &[("afnemer", &zo)]);
    let lexo = cellen.path().join("cellen/register/lexostatussen.yaml");
    let tekst = std::fs::read_to_string(&lexo).unwrap();
    std::fs::write(
        &lexo,
        tekst.replace(
            "        is_geschrapt_raad: {filter: {name: aanduiding_geschrapt, orgaan: raad, aanduiding: $aanduiding}, bestaat: true}\n",
            "",
        ),
    )
    .unwrap();
    // `aanduiding_geschrapt` leest dan niemand meer: markeer het event.
    let stroom = cellen.path().join("chronicles/test_registers.yaml");
    let tekst = std::fs::read_to_string(&stroom).unwrap();
    std::fs::write(
        &stroom,
        tekst.replace(
            "      orgaan: $external.orgaan\n  - name: uitslag_vastgesteld",
            "      orgaan: $external.orgaan\n    niet_gereduceerd:\n      - {veld: aanduiding, reden: test}\n      - {veld: orgaan, reden: test}\n  - name: uitslag_vastgesteld",
        ),
    )
    .unwrap();
    let data = tempfile::tempdir().unwrap();
    let a = runtime_op(cellen.path(), data.path()).unwrap();
    let w = a.waarschuwingen().await;
    assert!(
        w.iter()
            .any(|w| w.contains("de bron levert geen parameter 'is_geschrapt_raad'")),
        "{w:?}"
    );
}

#[test]
fn synthese_controle_bij_het_opstarten() {
    let geval = |aanpassing: &dyn Fn(String) -> String, verwacht: &str| {
        let opstelling = eigen_opstelling(
            &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
            &[("afnemer", aanpassing)],
        );
        let data = tempfile::tempdir().unwrap();
        let fouten = runtime_op(opstelling.path(), data.path())
            .map(|_| ())
            .expect_err(verwacht);
        assert!(
            fouten
                .iter()
                .any(|f| f.starts_with("proces 'test_afnemer_proces': ") && f.contains(verwacht)),
            "verwacht '{verwacht}' in {fouten:?}"
        );
    };
    // Een parameter die niet onder de toets valt.
    geval(
        &|t: String| {
            t.replace(
                "[is_ingeschreven_raad,",
                "[uitslag_openbaar, is_ingeschreven_raad,",
            )
        },
        "'uitslag_openbaar' is geen parameter van testregeling_afnemer#1",
    );
    // Een parameter uit twee bronnen: de eigen reductie en de synthese.
    geval(
        &|t: String| {
            t.replace(
                "[is_ingeschreven_raad,",
                "[bevat_aanduiding, is_ingeschreven_raad,",
            )
        },
        "parameter 'bevat_aanduiding' komt uit meer dan een bron",
    );
    // Een invoer uit een veld dat de toets-lexostatus niet levert.
    geval(
        &|t: String| t.replace("veld: aanduiding}", "veld: aanduiding_x}"),
        "levert geen 'aanduiding_x'",
    );
    // Synthese zonder portaal en zonder besluit.
    geval(
        &|t: String| {
            let (voor, na) = t.split_once("rollen:").unwrap();
            let (_, synthese) = na.split_once("synthese:").unwrap();
            let (synthese, _) = synthese.split_once("behandeling:").unwrap();
            format!("{voor}synthese:{synthese}")
        },
        "synthese zonder portaal en zonder besluit",
    );
}

#[test]
fn besluit_controle_bij_het_opstarten() {
    let geval = |aanpassing: &dyn Fn(String) -> String, verwacht: &str| {
        let opstelling = eigen_opstelling(
            &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
            &[("afnemer", aanpassing)],
        );
        let data = tempfile::tempdir().unwrap();
        let fouten = runtime_op(opstelling.path(), data.path())
            .map(|_| ())
            .expect_err(verwacht);
        assert!(
            fouten
                .iter()
                .any(|f| f.starts_with("proces 'test_afnemer_proces': ") && f.contains(verwacht)),
            "verwacht '{verwacht}' in {fouten:?}"
        );
    };
    geval(
        &|t: String| t.replace("  behandelaar: medewerker\n", ""),
        "behandeling zonder rol behandelaar",
    );
    geval(
        &|t: String| t.replace("  aanvrager: eherkenning\n", ""),
        "portaal zonder rol aanvrager",
    );
    geval(
        &|t: String| t.replace("lexostatus: werkvoorraad}", "lexostatus: aanvraag_inhoud}"),
        "werkvoorraad 'aanvraag_inhoud' is geen lijst",
    );
    geval(
        &|t: String| {
            t.replace(
                "uitkomsten: [vastgesteld_bedrag,",
                "uitkomsten: [bestaat_niet,",
            )
        },
        "heeft geen uitkomst 'bestaat_niet'",
    );
    geval(
        &|t: String| {
            t.replace(
                "lexostatus: zaakverloop, zaak: true}",
                "lexostatus: werkvoorraad, zaak: true}",
            )
        },
        "lexostatus 'werkvoorraad' is een lijst",
    );
    // Een bron van de zaak in een andere cel dan die van het proces.
    geval(
        &|t: String| {
            t.replace(
                "{cel: test_afnemer, lexostatus: zaakverloop, zaak: true}",
                "{cel: test_register, lexostatus: zaakverloop, zaak: true}",
            )
        },
        "cel 'test_register', en het proces legt vast in cel 'test_afnemer'",
    );
    // Een gewone bron uit de eigen cel: dat is een bron van de zaak.
    geval(
        &|t: String| {
            t.replace(
                "  - cel: test_register\n    lexostatus: registerstatus\n",
                "  - cel: test_afnemer\n    lexostatus: registerstatus\n",
            )
        },
        "is een bron van de zaak (zaak: true)",
    );
    geval(
        &|t: String| {
            t.replace(
                "      bekendgemaakt: false\n",
                "      bekendgemaakt: false\n      besluitdatum: null\n",
            )
        },
        "parameter 'besluitdatum' komt uit meer dan een bron",
    );
    geval(
        &|t: String| t.replace("      datum_bekendmaking: null\n", "      datum_bekendmaking: null\n      opgeschorte_dagen: null\n"),
        "parameter 'opgeschorte_dagen' komt uit meer dan een bron: de eigen lexostatus 'zaakverloop', de stand bij besluit",
    );
    // Synthese per regel.
    geval(
        &|t: String| {
            t.replace(
                "        tabel: {lexostatus: aanvraag_inhoud, veld: gebieden}",
                "        tabel: {lexostatus: aanvraag_inhoud, veld: dorpen}",
            )
        },
        "lexostatus 'aanvraag_inhoud' levert geen 'dorpen'",
    );
    geval(
        &|t: String| t.replace("        tabel: {lexostatus: aanvraag_inhoud, veld: gebieden}", "        tabel: {lexostatus: werkvoorraad, veld: gebieden}"),
        "de tabel komt uit lexostatus 'werkvoorraad', en die is geen lexostatus van de zaak (zaak: true)",
    );
    geval(
        &|t: String| {
            t.replace(
                "              gebied: {kolom: gebied}\n              peildatum:",
                "              gebied: {kolom: gebiedje}\n              peildatum:",
            )
        },
        "kolom 'gebiedje' wordt door niets ervoor gevuld",
    );
    geval(
        &|t: String| t.replace("- parameter: gebiedstabel", "- parameter: dorpstabel"),
        "besluit, rijen: 'dorpstabel' is geen parameter van testregeling_afnemer#3",
    );
    geval(
        &|t: String| {
            t.replace(
                "            kolommen: {tarief: tarief}",
                "            kolommen: {tarief: zetels}",
            )
        },
        "kolom 'zetels' komt uit meer dan een plek: de tabel, bron test_gebieden/tarief",
    );
    // Waar het besluit wordt vastgelegd.
    geval(
        &|t: String| {
            t.replace(
                "      event: besluit_genomen",
                "      event: termijn_opgeschort",
            )
        },
        "het event heeft geen stage",
    );
    geval(
        &|t: String| t.replace("    uitkomsten: [vastgesteld_bedrag, gebiedsbedrag,", "    uitkomsten: [vastgesteld_bedrag,"),
        "het besluit heeft de uitkomsten [besluit_tijdig, besluitdeadline, vastgesteld_bedrag, zorgvuldig]",
    );
}

// --- De behandelaar: werkvoorraad, zaak en proefbesluit ---

const AFNEMER: &str = "/processen/test_afnemer_proces";
const AFNEMER_CEL: &str = "/cellen/test_afnemer";

async fn afnemer_indienen(app: &Router, kvk: &str) -> String {
    let (_, _, cookie) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": kvk, "persoon": "A. Tester"})),
    )
    .await;
    let (status, body, _) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/aanvraag"),
        cookie.as_deref(),
        Some(afnemer_concept(Some("VOORBEELD"))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // De aanvraag opent de zaak in stage AANVRAAG (RFC-008); de werkvoorraad
    // laat alleen een zaak met stage BESLUIT weg.
    assert_eq!(body["gram"]["stage"], "AANVRAAG");
    body["gram"]["zaakkenmerk"].as_str().unwrap().to_string()
}

async fn behandelaar(app: &Router) -> String {
    let (status, body, cookie) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/medewerker/login"),
        None,
        Some(json!({"naam": "B. Behandelaar"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["naam"], "B. Behandelaar");
    cookie.unwrap()
}

/// Voeg een gram toe aan de kroniek van de afnemer, zoals stap voor stap
/// vastleggen dat later zal doen. Langs de kroniek van de runtime: die houdt
/// de grammen in het geheugen, dus een regel die iemand anders in het bestand
/// schrijft, ziet zij niet.
/// Leg een verloopgram vast op de klok van de test.
fn voeg_gram_toe(rt: &Runtime, name: &str, zaak: &str, fields: Value) {
    voeg_gram_toe_op(rt, name, zaak, fields, "2025-03-12T10:14:03+01:00");
}

/// Leg een verloopgram vast dat rechtens geldt op `op_moment`, vastgelegd op
/// de klok van de test.
fn voeg_gram_toe_op(rt: &Runtime, name: &str, zaak: &str, fields: Value, op_moment: &str) {
    let (type_, stage) = if name == "besluit_genomen" {
        ("decretogram", Some("BESLUIT"))
    } else {
        ("executogram", None)
    };
    let mut gram = json!({
        "kind": "chronolexogram", "type": type_, "name": name,
        "chronicle": "test_afnemer", "recording_actor": "test_afnemer",
        "grondslag": ["testregeling_afnemer#3"], "op_moment": op_moment,
        "vastgelegd_op": "2025-03-12T10:14:03+01:00",
        "zaak": "volgt", "zaakkenmerk": zaak,
        "stroom": {"id": "test_afnemer_zaakverloop", "sha256": "0".repeat(64)},
        "fields": fields,
    });
    if let Some(s) = stage {
        gram["stage"] = json!(s);
    }
    schema::valideer(Soort::Gram, &gram).unwrap();
    rt.cellen
        .iter()
        .find(|c| c.cel.id() == "test_afnemer")
        .unwrap()
        .kroniek
        .voeg_toe(&serde_json::from_value(gram).unwrap())
        .unwrap();
}

#[tokio::test]
async fn rollen_bepalen_wie_wat_mag() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let een = afnemer_indienen(&app, "12345678").await;
    let _twee = afnemer_indienen(&app, "87654321").await;

    // Zonder login: niets.
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/werkvoorraad"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // De aanvrager: zijn eigen grammen, geen werkvoorraad en geen zaak.
    let (_, _, aanvrager) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A"})),
    )
    .await;
    let aanvrager = aanvrager.as_deref();
    for (methode, pad) in [
        ("GET", format!("{AFNEMER}/api/werkvoorraad")),
        ("GET", format!("{AFNEMER}/api/zaken/{een}")),
        ("POST", format!("{AFNEMER}/api/zaken/{een}/proefbesluit")),
        ("GET", format!("{AFNEMER}/api/medewerker/sessie")),
    ] {
        let (status, body, _) = vraag(&app, methode, &pad, aanvrager, Some(json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{methode} {pad}: {body}");
    }
    // De kroniek is van de cel, zonder login en zonder rollen: de cel kent
    // geen aanvrager en geen behandelaar.
    let (status, kroniek, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/kroniek"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(kroniek.as_array().unwrap().len(), 2);

    // De behandelaar: het portaal niet.
    let b = behandelaar(&app).await;
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/aanvraag/toets"),
        Some(&b),
        Some(afnemer_concept(Some("VOORBEELD"))),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/eherkenning/sessie"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Een lege naam is geen medewerker; een cel zonder rol behandelaar heeft
    // geen medewerkerslogin en geen werkvoorraad.
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/medewerker/login"),
        None,
        Some(json!({"naam": " "})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    for pad in ["/api/medewerker/login", "/api/werkvoorraad"] {
        let (status, _, _) = vraag(
            &app,
            "POST",
            &format!("{INSTANTIE}{pad}"),
            None,
            Some(json!({})),
        )
        .await;
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED,
            "{pad}: {status}"
        );
    }
}

#[tokio::test]
async fn werkvoorraad_is_een_lijst_van_zaken_zonder_besluit() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = rt.router.clone();
    let een = afnemer_indienen(&app, "12345678").await;
    let twee = afnemer_indienen(&app, "87654321").await;
    let b = behandelaar(&app).await;

    let (status, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/werkvoorraad"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    assert_eq!(w["naam"], "werkvoorraad");
    assert_eq!(
        w["parameters"],
        json!({}),
        "een lijst heeft geen parameters"
    );
    let lijst = w["lijst"].as_array().unwrap();
    assert_eq!(lijst.len(), 2);
    let regel = lijst
        .iter()
        .find(|r| r["zaakkenmerk"] == een.as_str())
        .unwrap();
    assert_eq!(
        regel["velden"],
        json!({"ontvangen_op": "2025-03-12", "aanvrager": "Vereniging Voorbeeld", "kvk": "12345678"})
    );

    // Een verloopgram laat de zaak staan; een besluit haalt haar eraf.
    voeg_gram_toe(&rt, "termijn_opgeschort", &een, json!({"dagen": 5}));
    let (_, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/werkvoorraad"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(w["lijst"].as_array().unwrap().len(), 2);
    voeg_gram_toe(
        &rt,
        "besluit_genomen",
        &een,
        json!({"vastgesteld_bedrag": 6000, "gebiedsbedrag": 5000, "besluit_tijdig": false,
               "besluitdeadline": "2025-04-11", "zorgvuldig": true}),
    );
    let (_, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/werkvoorraad"),
        Some(&b),
        None,
    )
    .await;
    let lijst = w["lijst"].as_array().unwrap();
    assert_eq!(lijst.len(), 1);
    assert_eq!(lijst[0]["zaakkenmerk"], twee.as_str());

    // De cellenlijst noemt de werkvoorraad een lijst, met kolommen; de
    // processenlijst zegt wie haar ziet.
    let (_, cellen, _) = vraag(&app, "GET", "/api/cellen", None, None).await;
    let werkvoorraad = cellen[0]["lexostatussen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "werkvoorraad")
        .unwrap();
    assert_eq!(werkvoorraad["lijst"], json!(true));
    assert_eq!(werkvoorraad["parameters"], json!([]));
    let (_, processen, _) = vraag(&app, "GET", "/api/processen", None, None).await;
    assert_eq!(
        processen[0]["rollen"],
        json!({"aanvrager": true, "behandelaar": true})
    );
    assert_eq!(processen[0]["behandeling"]["werkvoorraad"], "werkvoorraad");
}

#[tokio::test]
async fn zaak_met_proefbesluit_zonder_vastleggen() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = rt.router.clone();
    let zaak = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let kroniek = data.path().join("test_afnemer/test_afnemer.jsonl");
    let voor = std::fs::read_to_string(&kroniek).unwrap();

    // Zonder oordelen: niet te nemen, en het proefbesluit zegt wat er mist.
    let (status, z, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/zaken/{zaak}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{z}");
    assert_eq!(z["grammen"].as_array().unwrap().len(), 1);
    assert_eq!(z["besluit"]["artikel"], "testregeling_afnemer#3");
    assert_eq!(
        z["besluit"]["formulier"],
        json!([
            {"naam": "besluitdatum", "label": "Besluitdatum", "type": "datum", "groep": "Testregeling afnemer, artikel 3"},
            {"naam": "feiten_vergaard", "label": "De relevante feiten zijn vergaard", "type": "janee", "groep": "Testregeling afnemer, artikel 3"}
        ])
    );
    let p = &z["proefbesluit"];
    assert_eq!(p["te_nemen"], json!(false), "{p}");
    assert!(p.get("uitkomsten").is_none());
    assert!(
        p["reden"]
            .as_str()
            .unwrap()
            .starts_with("niet te nemen: mist "),
        "{p}"
    );
    let niet: Vec<&str> = p["niet_geleverd"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["naam"].as_str().unwrap())
        .collect();
    assert_eq!(niet, ["besluitdatum", "feiten_vergaard"]);

    // Met oordelen: te nemen, met herkomst per parameter.
    let (status, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/proefbesluit"),
        Some(&b),
        Some(json!({"formulier": {"besluitdatum": "2025-03-20", "feiten_vergaard": true}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["te_nemen"], json!(true), "{p}");
    assert_eq!(p["uitkomsten"]["vastgesteld_bedrag"], json!(6000));
    assert_eq!(p["uitkomsten"]["besluit_tijdig"], json!(false));
    assert_eq!(p["uitkomsten"]["besluitdeadline"], json!("2025-04-11"));
    assert_eq!(p["uitkomsten"]["zorgvuldig"], json!(true));
    assert_eq!(
        p["herkomst"]["besluitdatum"],
        json!({"bron": "behandelaar"})
    );
    assert_eq!(
        p["herkomst"]["datum_bekendmaking"],
        json!({"bron": "stand_bij_besluit"})
    );
    assert_eq!(
        p["herkomst"]["opgeschorte_dagen"],
        json!({"bron": "eigen", "lexostatus": "zaakverloop"})
    );
    assert_eq!(
        p["herkomst"]["aanvraagdatum"],
        json!({"bron": "eigen", "lexostatus": "aanvraag_inhoud"})
    );
    assert_eq!(p["herkomst"]["zetels_op_lijst"]["cel"], "test_register");
    // Geen aanvulling gevraagd: de reductie leest het ontbreken als null.
    assert_eq!(p["parameters"]["datum_uitnodiging_aanvulling"], Value::Null);
    assert_eq!(p["parameters"]["opgeschorte_dagen"], json!(0));
    assert_eq!(p["niet_geleverd"], json!([]));

    // Een opschorting in de zaak schuift de uiterste datum op.
    voeg_gram_toe(&rt, "termijn_opgeschort", &zaak, json!({"dagen": 5}));
    let (_, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/proefbesluit"),
        Some(&b),
        Some(json!({"formulier": {"besluitdatum": "2025-03-20", "feiten_vergaard": true}})),
    )
    .await;
    assert_eq!(p["uitkomsten"]["besluitdeadline"], json!("2025-04-16"));

    // Een opschorting die pas na de peildatum ingaat, telt bij dit besluit
    // niet mee: de cel reduceert op de peildatum van het besluit.
    voeg_gram_toe_op(
        &rt,
        "termijn_opgeschort",
        &zaak,
        json!({"dagen": 30}),
        "2025-04-01T09:00:00+02:00",
    );
    let (_, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/proefbesluit"),
        Some(&b),
        Some(json!({"formulier": {"besluitdatum": "2025-03-20", "feiten_vergaard": true}})),
    )
    .await;
    assert_eq!(p["uitkomsten"]["besluitdeadline"], json!("2025-04-16"));
    assert_eq!(
        p["lexostatussen"][1]["peilmoment"],
        json!("2025-03-12"),
        "{p}"
    );

    // Er is niets vastgelegd, behalve de verloopgrammen van deze test.
    let na = std::fs::read_to_string(&kroniek).unwrap();
    assert_eq!(na.lines().count(), voor.lines().count() + 2);

    // Een oordeel dat het formulier niet kent, en een onbekende zaak.
    let (status, f, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/proefbesluit"),
        Some(&b),
        Some(json!({"formulier": {"bekendgemaakt": true}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(f["fout"].as_str().unwrap().contains("'bekendgemaakt'"));
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/zaken/00000000-0000-4000-8000-000000000009"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn zaak_opent_en_volgt() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let c = inloggen(&app, "12345678").await;
    let aanvraag = format!("{INSTANTIE}/api/aanvraag");
    // Een event dat een zaak opent: de cel geeft het kenmerk, het concept niet.
    let mut met_kenmerk = volledig();
    met_kenmerk["zaakkenmerk"] = json!("00000000-0000-4000-8000-000000000009");
    let (status, body, _) = vraag(&app, "POST", &aanvraag, Some(&c), Some(met_kenmerk)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["fout"].as_str().unwrap().contains("opent een zaak"),
        "{body}"
    );
    let (status, body, _) = vraag(&app, "POST", &aanvraag, Some(&c), Some(volledig())).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["zaak"], "opent");
    let zaak = body["gram"]["zaakkenmerk"].as_str().unwrap().to_string();
    drop(app);

    // Dezelfde kroniek, nu met een stroom waarin het event een zaak volgt.
    let opstelling = eigen_opstelling(&[("instantie", &zo)], &[("instantie", &zo)]);
    let stroom = opstelling.path().join("chronicles/test_aanvragen.yaml");
    let tekst = std::fs::read_to_string(&stroom).unwrap();
    std::fs::write(&stroom, tekst.replace("zaak: opent", "zaak: volgt")).unwrap();
    let app = runtime_op(opstelling.path(), data.path()).unwrap().router;
    let c = inloggen(&app, "12345678").await;
    let (status, body, _) = vraag(&app, "POST", &aanvraag, Some(&c), Some(volledig())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["fout"]
            .as_str()
            .unwrap()
            .contains("geef het zaakkenmerk mee"),
        "{body}"
    );
    let mut onbekend = volledig();
    onbekend["zaakkenmerk"] = json!("00000000-0000-4000-8000-000000000009");
    let (status, body, _) = vraag(&app, "POST", &aanvraag, Some(&c), Some(onbekend)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["fout"].as_str().unwrap().contains("geen zaak"),
        "{body}"
    );
    let mut volgt = volledig();
    volgt["zaakkenmerk"] = json!(zaak);
    // Een andere KvK kent deze zaak niet: dat weet het proces, niet de cel.
    let ander = inloggen(&app, "87654321").await;
    let (status, body, _) = vraag(&app, "POST", &aanvraag, Some(&ander), Some(volgt.clone())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["fout"].as_str().unwrap().contains("geen zaak"),
        "{body}"
    );
    let (status, body, _) = vraag(&app, "POST", &aanvraag, Some(&c), Some(volgt)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["zaak"], "volgt");
    assert_eq!(body["gram"]["zaakkenmerk"], json!(zaak));
    schema::valideer(Soort::Gram, &body["gram"]).unwrap();
}

#[test]
fn een_falende_cel_houdt_de_runtime_tegen() {
    let kapot = |t: String| {
        t.replace(
            "lexostatussen: lexostatussen.yaml",
            "lexostatussen: weg.yaml",
        )
    };
    let opstelling = eigen_opstelling(
        &[("instantie", &zo), ("register", &kapot)],
        &[("instantie", &zo)],
    );
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert_eq!(fouten.len(), 1, "{fouten:?}");
    assert!(fouten[0].starts_with("cel 'test_register': "), "{fouten:?}");
}

#[test]
fn een_falend_proces_houdt_de_runtime_tegen() {
    let kapot = |t: String| t.replace("actor: test_instantie", "actor: iemand_anders");
    let opstelling = eigen_opstelling(&[("instantie", &zo)], &[("instantie", &kapot)]);
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert_eq!(fouten.len(), 1, "{fouten:?}");
    assert!(
        fouten[0].starts_with("proces 'test_instantie_proces': portaal: stroom 'test_aanvragen' heeft recording_actor 'test_instantie'"),
        "{fouten:?}"
    );
}

#[test]
fn zonder_processen_draaien_alleen_de_cellen() {
    let data = tempfile::tempdir().unwrap();
    let config = Config {
        cells_path: fixtures().join("cellen"),
        processes_path: None,
        regulation_path: fixtures().join("regulation"),
        data_dir: data.path().to_path_buf(),
        port: STANDAARD_POORT,
    };
    let r = Runtime::laad(&config, klok()).unwrap();
    assert_eq!(r.cellen.len(), 4);
    assert!(r.processen.is_empty());
}

// --- Synthese per regel en het vastleggen van het besluit ---

/// Het besluitformulier van de afnemer, volledig ingevuld.
fn oordelen() -> Value {
    json!({"formulier": {"besluitdatum": "2025-03-20", "feiten_vergaard": true}})
}

#[tokio::test]
async fn synthese_per_regel_vult_de_tabel_aan() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let zaak = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let (status, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/proefbesluit"),
        Some(&b),
        Some(oordelen()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");

    // De regels komen uit de aanvraag; de kolommen `ingeschreven` en
    // `tarief` uit twee andere cellen, per regel bevraagd.
    assert_eq!(
        p["parameters"]["gebiedstabel"],
        json!([
            {"gebied": "Voorbeeldstad", "zetels": 4, "ingeschreven": true, "tarief": 1000},
            {"gebied": "Buurdorp", "zetels": 2, "ingeschreven": false, "tarief": 500},
        ])
    );
    // 4 x 1000 + 2 x 500.
    assert_eq!(p["uitkomsten"]["gebiedsbedrag"], json!(5000));
    assert_eq!(
        p["herkomst"]["gebiedstabel"],
        json!({"bron": "per_regel", "lexostatus": "aanvraag_inhoud", "veld": "gebieden"})
    );
    let rijen = &p["rijen"][0];
    assert_eq!(rijen["parameter"], "gebiedstabel");
    assert_eq!(rijen["bronnen"][0]["cel"], "test_register");
    assert_eq!(rijen["bronnen"][0]["bevraagd"], json!(2));
    assert_eq!(rijen["bronnen"][1]["cel"], "test_gebieden");
    assert_eq!(rijen["bronnen"][1]["status"], "bevraagd");
    assert!(rijen.get("mist").is_none(), "{rijen}");

    // De peildatum is het jaartal uit de registercel (jaar_van), omgezet
    // naar 1 januari van dat jaar.
    assert_eq!(p["parameters"]["jaar"], json!(2024));
    assert_eq!(p["herkomst"]["jaar"]["cel"], "test_register");
}

#[tokio::test]
async fn een_gebied_zonder_tarief_blijft_leeg() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    // Een gebied waarvoor geen tarief is vastgesteld: die kolom blijft weg,
    // en de engine kan de uitkomst dan niet geven.
    let (_, _, cookie) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    let mut concept = afnemer_concept(Some("VOORBEELD"));
    concept["external"]["gebieden"] = json!([{"gebied": "Onbekendstad", "zetels": 1}]);
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/aanvraag"),
        cookie.as_deref(),
        Some(concept),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let zaak = body["gram"]["zaakkenmerk"].as_str().unwrap().to_string();
    let b = behandelaar(&app).await;
    let (_, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/proefbesluit"),
        Some(&b),
        Some(oordelen()),
    )
    .await;
    assert_eq!(p["rijen"][0]["mist"], json!(["tarief"]));
    assert_eq!(
        p["parameters"]["gebiedstabel"],
        json!([{"gebied": "Onbekendstad", "zetels": 1, "ingeschreven": false}])
    );
    assert_eq!(p["te_nemen"], json!(false), "{p}");

    // En dan legt de cel niets vast.
    let (status, f, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/besluit"),
        Some(&b),
        Some(oordelen()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(f["fout"].as_str().unwrap().starts_with("niet te nemen"));
    let kroniek =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(kroniek.lines().count(), 1, "alleen de aanvraag");
}

#[tokio::test]
async fn besluit_nemen_legt_een_decretogram_vast() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let zaak = afnemer_indienen(&app, "12345678").await;
    let twee = afnemer_indienen(&app, "87654321").await;
    let b = behandelaar(&app).await;

    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/besluit"),
        Some(&b),
        Some(oordelen()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gram = &body["gram"];
    schema::valideer(Soort::Gram, gram).unwrap();
    assert_eq!(gram["type"], "decretogram");
    assert_eq!(gram["stage"], "BESLUIT");
    assert_eq!(gram["zaak"], "volgt");
    assert_eq!(gram["zaakkenmerk"], zaak.as_str());
    assert_eq!(gram["legal_character"], "BESCHIKKING");
    assert_eq!(gram["grondslag"], json!(["testregeling_afnemer#3 lid 1"]));
    assert_eq!(gram["decision_type"], "TOEKENNING");
    assert_eq!(gram["regulation"], "testregeling_afnemer");
    assert_eq!(gram["regulation_valid_from"], "2025-01-01");
    // De testregeling noemt geen bevoegd gezag: vastleggen met een
    // waarschuwing, en zonder een verzonnen gezag.
    assert!(gram.get("competent_authority").is_none(), "{gram}");
    assert_eq!(body["waarschuwingen"].as_array().unwrap().len(), 1);
    assert!(body["waarschuwingen"][0]
        .as_str()
        .unwrap()
        .contains("geen bevoegd gezag"));

    // De velden zijn de uitkomsten van het artikel.
    assert_eq!(
        gram["fields"],
        json!({"vastgesteld_bedrag": 6000, "gebiedsbedrag": 5000, "besluit_tijdig": false,
               "besluitdeadline": "2025-04-11", "zorgvuldig": true})
    );
    // De invoer, elk met haar herkomst.
    let inputs = gram["inputs"].as_object().unwrap();
    assert_eq!(
        inputs["besluitdatum"],
        json!({"waarde": "2025-03-20", "herkomst": {"bron": "behandelaar"}})
    );
    assert_eq!(inputs["zetels_op_lijst"]["herkomst"]["bron"], "cel");
    assert_eq!(inputs["gebiedstabel"]["herkomst"]["bron"], "per_regel");
    assert_eq!(
        inputs["datum_bekendmaking"]["herkomst"]["bron"],
        "stand_bij_besluit"
    );
    assert_eq!(
        inputs["aanvraagdatum"]["herkomst"],
        json!({"bron": "eigen", "lexostatus": "aanvraag_inhoud"})
    );
    // Het receipt: de geladen regelingen en de stromen, met een hash erover.
    let receipt = &gram["receipt"];
    let regelingen: Vec<&str> = receipt["regelingen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(regelingen.contains(&"testregeling_afnemer"), "{receipt}");
    assert_eq!(receipt["regelingen"][0]["valid_from"], "2025-01-01");
    let stromen: Vec<&str> = receipt["stromen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        stromen,
        ["test_afnemer_aanvragen", "test_afnemer_zaakverloop"]
    );
    assert_eq!(receipt["sha256"].as_str().unwrap().len(), 64);

    // De zaak verdwijnt uit de werkvoorraad, en het gram staat in de zaak.
    let (_, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/werkvoorraad"),
        Some(&b),
        None,
    )
    .await;
    let lijst = w["lijst"].as_array().unwrap();
    assert_eq!(lijst.len(), 1);
    assert_eq!(lijst[0]["zaakkenmerk"], twee.as_str());
    let (_, z, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/zaken/{zaak}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(z["grammen"].as_array().unwrap().len(), 2);

    // Een tweede besluit in dezelfde zaak: dat is een wijziging.
    let (status, f, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/besluit"),
        Some(&b),
        Some(oordelen()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["fout"]
            .as_str()
            .unwrap()
            .contains("ligt al een gram met stage BESLUIT"),
        "{f}"
    );
    let kroniek =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(kroniek.lines().count(), 3, "twee aanvragen en een besluit");

    // De aanvrager mag niet besluiten.
    let (_, _, aanvrager) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A"})),
    )
    .await;
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{twee}/besluit"),
        aanvrager.as_deref(),
        Some(oordelen()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // De cel geeft een proces alleen de zaak die het vraagt, en kent een
    // onbekende zaak niet.
    let (status, g, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/zaken/{zaak}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{g}");
    let g = g.as_array().unwrap();
    assert_eq!(g.len(), 2);
    assert!(g.iter().all(|i| i["gram"]["zaakkenmerk"] == zaak.as_str()));
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/zaken/00000000-0000-4000-8000-000000000009"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Twee gelijktijdige besluiten op de andere zaak: de cel legt er één
    // vast, want de toets op de stage en het schrijven delen één slot.
    let pad = format!("{AFNEMER}/api/zaken/{twee}/besluit");
    let (een, ander) = tokio::join!(
        vraag(&app, "POST", &pad, Some(&b), Some(oordelen())),
        vraag(&app, "POST", &pad, Some(&b), Some(oordelen())),
    );
    let mut statussen = [een.0, ander.0];
    statussen.sort();
    assert_eq!(
        statussen,
        [StatusCode::CREATED, StatusCode::CONFLICT],
        "{} / {}",
        een.1,
        ander.1
    );
    let kroniek =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(
        kroniek.lines().count(),
        4,
        "twee aanvragen en twee besluiten"
    );
}

#[tokio::test]
async fn een_ander_bevoegd_gezag_weigert_het_besluit() {
    // De regeling wijst een ander gezag aan dan de actor van het proces:
    // geen gram.
    let cellen = eigen_opstelling(
        &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
        &[("afnemer", &zo)],
    );
    let doel = cellen.path().join("regulation/testregeling_afnemer");
    std::fs::create_dir_all(&doel).unwrap();
    let tekst =
        std::fs::read_to_string(fixtures().join("regulation/testregeling_afnemer/2025-01-01.yaml"))
            .unwrap();
    // Alleen artikel 3 is een BESCHIKKING; daar komt het gezag bij te staan.
    let met_gezag = tekst.replace(
        "    machine_readable:\n      execution:\n        produces:\n          legal_character: BESCHIKKING",
        "    machine_readable:\n      competent_authority:\n        name: Een andere instantie\n      execution:\n        produces:\n          legal_character: BESCHIKKING",
    );
    assert_ne!(met_gezag, tekst, "het gezag is niet in de regeling gezet");
    std::fs::write(doel.join("2025-01-01.yaml"), met_gezag).unwrap();
    for naam in ["testregeling_register", "testregeling_awb"] {
        let van = fixtures().join(format!("regulation/{naam}/2025-01-01.yaml"));
        let naar = cellen.path().join(format!("regulation/{naam}"));
        std::fs::create_dir_all(&naar).unwrap();
        std::fs::copy(van, naar.join("2025-01-01.yaml")).unwrap();
    }
    let data = tempfile::tempdir().unwrap();
    let config = Config {
        cells_path: cellen.path().join("cellen"),
        processes_path: Some(cellen.path().join("processes")),
        regulation_path: cellen.path().join("regulation"),
        data_dir: data.path().to_path_buf(),
        port: 0,
    };
    let app = Runtime::laad(&config, klok()).unwrap().router;
    let zaak = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let (status, f, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/zaken/{zaak}/besluit"),
        Some(&b),
        Some(oordelen()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["fout"]
            .as_str()
            .unwrap()
            .contains("'Een andere instantie' aan als bevoegd gezag"),
        "{f}"
    );
    let kroniek =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(kroniek.lines().count(), 1, "alleen de aanvraag");
}

// --- Voorbeelden per handeling ---

#[tokio::test]
async fn voorbeelden_zonder_login() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/voorbeelden"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["inloggen"],
        json!([
            {"label": "voorbeeld-login", "kvk": "12345678", "persoon": "A. Tester"},
            {"label": "voorbeeld-login-ander", "kvk": "87654321", "persoon": "B. Tester"},
        ])
    );
    assert_eq!(
        body["aanvraag"],
        afnemer_concept(Some("VOORBEELD"))["external"]
    );
    assert_eq!(body["besluit"], oordelen()["formulier"]);

    // Een proces zonder voorbeelden: leeg, geen fout.
    let (status, body, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/voorbeelden"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"inloggen": [], "aanvraag": null, "besluit": null})
    );
}

#[tokio::test]
async fn het_aanvraagvoorbeeld_is_in_te_dienen() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (_, v, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/voorbeelden"),
        None,
        None,
    )
    .await;
    let (status, _, cookie) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": v["inloggen"][0]["kvk"], "persoon": v["inloggen"][0]["persoon"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/aanvraag"),
        cookie.as_deref(),
        Some(json!({"external": v["aanvraag"]})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[test]
fn voorbeelden_controle_bij_het_opstarten() {
    let weg = |t: String| t.replace("voorbeeld-besluit.json", "weg.json");
    let alle = [
        ("afnemer", &zo as &dyn Fn(String) -> String),
        ("register", &zo),
        ("gebieden", &zo),
    ];
    let opstelling = eigen_opstelling(&alle, &[("afnemer", &weg)]);
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert_eq!(fouten.len(), 1, "{fouten:?}");
    assert!(
        fouten[0].starts_with("proces 'test_afnemer_proces': voorbeeld weg.json"),
        "{fouten:?}"
    );

    let verkeerd = |t: String| t.replace("voorbeeld-besluit.json", "voorbeeld-login.json");
    let opstelling = eigen_opstelling(&alle, &[("afnemer", &verkeerd)]);
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert!(
        fouten[0].contains("voorbeeld-login.json: verwacht een object met 'formulier'"),
        "{fouten:?}"
    );
}

// --- De cel legt vast en reduceert op proef, op verzoek van een proces ---

/// Een verzoek aan de instantie-cel voor het portaal-event.
fn verzoek(actor: &str) -> Value {
    json!({
        "actor": actor,
        "stroom": "test_aanvragen",
        "event": "aanvraag_ontvangen",
        "intake": {"kanaal": "portaal", "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"}},
        "external": volledig()["external"],
    })
}

#[tokio::test]
async fn de_cel_legt_een_gram_vast_voor_haar_actor() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let grammen = format!("{INSTANTIE_CEL}/api/grammen");
    let (status, body) = als_runtime(&rt, "POST", &grammen, verzoek("test_instantie")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    schema::valideer(Soort::Gram, &body["gram"]).unwrap();
    // De cel bouwt het gram uit haar stroom: zij geeft het zaakkenmerk en het
    // moment.
    assert_eq!(body["gram"]["zaak"], "opent");
    assert!(body["gram"]["zaakkenmerk"].is_string());
    assert_eq!(body["gram"]["op_moment"], "2025-03-12T10:14:03+01:00");
    assert_eq!(body["gram"]["recording_actor"], "test_instantie");
    assert!(body["yaml"]
        .as_str()
        .unwrap()
        .starts_with("kind: chronolexogram\n"));
    let pad = dir.path().join("test_instantie/test_kroniek.jsonl");
    assert_eq!(std::fs::read_to_string(&pad).unwrap().lines().count(), 1);

    // Een actor die niet de recording_actor van de stroom is: geen gram.
    let (status, body) = als_runtime(&rt, "POST", &grammen, verzoek("test_afnemer")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        body["fout"],
        "actor 'test_afnemer' legt niet vast in stroom 'test_aanvragen': de recording_actor is 'test_instantie'"
    );
    // Een event dat de cel niet heeft, en een veld dat de stroom niet kent.
    let mut onbekend = verzoek("test_instantie");
    onbekend["event"] = json!("bestaat_niet");
    let (status, _) = als_runtime(&rt, "POST", &grammen, onbekend).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut veld = verzoek("test_instantie");
    veld["external"]["schoenmaat"] = json!(44);
    let (status, body) = als_runtime(&rt, "POST", &grammen, veld).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["fout"].as_str().unwrap().contains("schoenmaat"));
    assert_eq!(std::fs::read_to_string(&pad).unwrap().lines().count(), 1);
}

#[tokio::test]
async fn de_cel_reduceert_op_proef_zonder_vast_te_leggen() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let app = rt.router.clone();
    let proef = format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud/proef");
    let (status, body) = als_runtime(
        &rt,
        "POST",
        &proef,
        json!({"concept": verzoek("test_instantie"), "inputs": {}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Het gram van het concept, en de reductie met dat gram: het zaakkenmerk
    // van het concept is de input.
    let zaak = body["gram"]["zaakkenmerk"].as_str().unwrap();
    assert_eq!(body["lexostatus"]["zaakkenmerk"], zaak);
    assert_eq!(body["lexostatus"]["parameters"]["bevat_naam"], json!(true));
    assert_eq!(
        body["lexostatus"]["parameters"]["aanvraagdatum"],
        "2025-03-12"
    );
    // Niets vastgelegd.
    let (_, kroniek, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/kroniek"),
        None,
        None,
    )
    .await;
    assert_eq!(kroniek, json!([]));
    // Ook op proef legt alleen de actor van de stroom iets voor.
    let (status, _) = als_runtime(
        &rt,
        "POST",
        &proef,
        json!({"concept": verzoek("iemand_anders")}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn een_proefreductie_reduceert_de_kroniek_met_het_concept() {
    // De werkvoorraad van de afnemer: een lijst over de hele kroniek. Op
    // proef telt het concept mee naast wat er al ligt, en er blijft niets van
    // over.
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = rt.router.clone();
    afnemer_indienen(&app, "12345678").await;
    let concept = json!({
        "actor": "test_afnemer",
        "stroom": "test_afnemer_aanvragen",
        "event": "aanvraag_ontvangen",
        "intake": {"kanaal": "portaal", "eherkenning": {"kvk": "87654321", "persoon": "B. Tester"}},
        "external": afnemer_concept(Some("VOORBEELD"))["external"],
    });
    let (status, body) = als_runtime(
        &rt,
        "POST",
        &format!("{AFNEMER_CEL}/api/lexostatus/werkvoorraad/proef"),
        json!({"concept": concept}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lexostatus"]["lijst"].as_array().unwrap().len(), 2);
    let (_, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/lexostatus/werkvoorraad"),
        None,
        None,
    )
    .await;
    assert_eq!(w["lijst"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn alleen_de_runtime_legt_vast_en_lezen_blijft_open() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let grammen = format!("{INSTANTIE_CEL}/api/grammen");
    let proef = format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud/proef");
    // Zonder token: 401, met een ander token: 403, voor vastleggen en proef.
    for (pad, body) in [
        (&grammen, verzoek("test_instantie")),
        (&proef, json!({"concept": verzoek("test_instantie")})),
    ] {
        let (status, fout, _) = vraag(&rt.router, "POST", pad, None, Some(body.clone())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{pad}: {fout}");
        assert!(fout["fout"]
            .as_str()
            .unwrap()
            .contains("runtime-token ontbreekt"));
        let vals = [(RUNTIME_TOKEN_HEADER, "0".repeat(64))];
        let vals: Vec<(&str, &str)> = vals.iter().map(|(n, w)| (*n, w.as_str())).collect();
        let (status, _, _) = vraag_met(&rt.router, "POST", pad, &vals, Some(body)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{pad}");
    }
    // Het token van een andere runtime telt ook niet.
    let ander = runtime_op(&fixtures(), tempfile::tempdir().unwrap().path()).unwrap();
    let vreemd = [(RUNTIME_TOKEN_HEADER, ander.runtime_token.als_str())];
    let (status, _, _) = vraag_met(
        &rt.router,
        "POST",
        &grammen,
        &vreemd,
        Some(verzoek("test_instantie")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Er is niets vastgelegd, en lezen vraagt geen token.
    for pad in ["kroniek", "stroom"] {
        let (status, _, _) = vraag(
            &rt.router,
            "GET",
            &format!("{INSTANTIE_CEL}/api/{pad}"),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pad}");
    }
    assert!(!dir
        .path()
        .join("test_instantie/test_kroniek.jsonl")
        .exists());
    // Het proces zelf legt wel vast: het interne transport draagt het token.
    let c = inloggen(&rt.router, "12345678").await;
    let (status, body, _) = vraag(
        &rt.router,
        "POST",
        &format!("{INSTANTIE}/api/aanvraag"),
        Some(&c),
        Some(volledig()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

// --- Het aanbod toetst alleen wat vooraf vaststaat ---

#[test]
fn een_aanbod_op_een_aanvraagfeit_houdt_de_runtime_tegen() {
    let met_aanbod = |t: String| {
        t.replace(
            "  formulier:",
            "  aanbod: {regeling: testregeling_aanvraag, uitkomst: aanvraag_volledig}\n  formulier:",
        )
    };
    let opstelling = eigen_opstelling(&[("instantie", &zo)], &[("instantie", &met_aanbod)]);
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert!(
        fouten.contains(&"proces 'test_instantie_proces': aanbod: voorwaarde leunt op 'aanvraagdatum' (BELANGHEBBENDE, grondslag testregeling_aanvraag#1 lid 1), dat vooraf niet bekend is".to_string()),
        "{fouten:?}"
    );
}

// --- Het tijdvak van het aanbod ---

/// Het aanbod draait per tijdvak uit `aanbod.keuzes`; het tijdvak is de
/// parameter met origin BELANGHEBBENDE en grondslag Awb 4:2 lid 1, niet een
/// vaste naam.
#[tokio::test]
async fn het_aanbod_draait_per_gekozen_tijdvak() {
    let met_aanbod = |t: String| {
        t.replace(
            "    uitkomst: aanvraag_toelaatbaar\n",
            "    uitkomst: aanvraag_toelaatbaar\n  aanbod:\n    regeling: testregeling_afnemer\n    uitkomst: aanvraag_aangeboden\n    termijn: aanvraagtermijn\n    keuzes: {jaren_vanaf_nu: [0, 1]}\n",
        )
    };
    let opstelling = eigen_opstelling(
        &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
        &[("afnemer", &met_aanbod)],
    );
    let data = tempfile::tempdir().unwrap();
    let app = runtime_op(opstelling.path(), data.path()).unwrap().router;
    let (_, _, cookie) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    let (status, body, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/mogelijkheden"),
        cookie.as_deref(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let m = body["mogelijkheden"].as_array().unwrap();
    assert_eq!(m.len(), 2, "{body}");
    for (i, jaar) in [(0, 2025), (1, 2026)] {
        assert_eq!(
            m[i]["mogelijkheid"]["tijdvak"],
            json!({"parameter": "aanvraagjaar", "waarde": jaar}),
            "{body}"
        );
        assert_eq!(m[i]["parameters"]["aanvraagjaar"], json!(jaar));
        assert_eq!(m[i]["herkomst"]["aanvraagjaar"], json!({"bron": "keuze"}));
        assert_eq!(
            m[i]["mogelijkheid"]["termijn"],
            json!(format!("{jaar}-04-01"))
        );
    }
}

/// Een aanbod dat een tijdvak vraagt, zonder keuzes: de runtime start niet.
#[test]
fn een_tijdvak_zonder_keuzes_houdt_de_runtime_tegen() {
    let met_aanbod = |t: String| {
        t.replace(
            "    uitkomst: aanvraag_toelaatbaar\n",
            "    uitkomst: aanvraag_toelaatbaar\n  aanbod: {regeling: testregeling_afnemer, uitkomst: aanvraag_aangeboden}\n",
        )
    };
    let opstelling = eigen_opstelling(
        &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
        &[("afnemer", &met_aanbod)],
    );
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert_eq!(
        fouten,
        ["proces 'test_afnemer_proces': aanbod: het tijdvak 'aanvraagjaar' (algemene_wet_bestuursrecht#4:2 lid 1) vraagt aanbod.keuzes: welke tijdvakken het portaal aanbiedt"]
    );
}

// --- De toets bouwt een tabel per regel op, zoals het besluit ---

/// De rijen van de toets: de gebiedstabel uit de proefreductie van het
/// concept, met een kolom die per regel uit de registercel komt.
fn met_toets_rijen(t: String) -> String {
    t.replace(
        "    uitkomst: aanvraag_toelaatbaar\n",
        "    uitkomst: aanvraag_toelaatbaar
    rijen:
      - parameter: gebiedstabel
        tabel: {lexostatus: aanvraag_inhoud, veld: gebieden}
        kolommen: {gebied: gebied}
        bronnen:
          - cel: test_register
            lexostatus: registratie_per_gebied
            invoer:
              aanduiding: {lexostatus: aanvraag_inhoud, veld: aanduiding}
              gebied: {kolom: gebied}
            kolommen: {ingeschreven: ingeschreven}
",
    )
}

#[tokio::test]
async fn de_toets_bouwt_een_tabel_per_regel_op() {
    let opstelling = eigen_opstelling(
        &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
        &[("afnemer", &met_toets_rijen)],
    );
    let data = tempfile::tempdir().unwrap();
    let a = runtime_op(opstelling.path(), data.path()).unwrap();
    let body = afnemer_toets(&a.router, Some("VOORBEELD")).await;
    // De regels komen uit het concept, de kolom `ingeschreven` per regel uit
    // de registercel; de tabel gaat met de andere parameters naar de engine.
    assert_eq!(
        body["parameters"]["gebiedstabel"],
        json!([
            {"gebied": "Voorbeeldstad", "ingeschreven": true},
            {"gebied": "Buurdorp", "ingeschreven": false},
        ]),
        "{body}"
    );
    assert_eq!(
        body["herkomst"]["gebiedstabel"],
        json!({"bron": "per_regel", "lexostatus": "aanvraag_inhoud", "veld": "gebieden"})
    );
    assert_eq!(body["rijen"][0]["bronnen"][0]["bevraagd"], json!(2));
    assert!(body["rijen"][0].get("mist").is_none(), "{body}");
    assert_eq!(body["uitslag"]["waarde"], json!(true), "{body}");
    // Niets vastgelegd.
    assert!(!data.path().join("test_afnemer/test_afnemer.jsonl").exists());

    // Zonder aanduiding wordt de bron per regel niet bevraagd: de kolom
    // blijft weg, er wordt niets aangevuld.
    let body = afnemer_toets(&a.router, None).await;
    assert_eq!(
        body["parameters"]["gebiedstabel"],
        json!([
            {"gebied": "Voorbeeldstad"},
            {"gebied": "Buurdorp"},
        ])
    );
    assert_eq!(body["rijen"][0]["mist"], json!(["ingeschreven"]));
    assert_eq!(body["rijen"][0]["bronnen"][0]["status"], "niet_bevraagd");
}

#[tokio::test]
async fn de_toets_zonder_rijen_blijft_gelijk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let body = afnemer_toets(&app, Some("VOORBEELD")).await;
    assert_eq!(body["rijen"], json!([]));
    assert!(body["parameters"].get("gebiedstabel").is_none(), "{body}");
    assert_eq!(body["uitslag"]["waarde"], json!(true), "{body}");
}

#[test]
fn toets_rijen_controle_bij_het_opstarten() {
    let uit_ander = |t: String| {
        met_toets_rijen(t).replacen(
            "tabel: {lexostatus: aanvraag_inhoud, veld: gebieden}",
            "tabel: {lexostatus: zaakverloop, veld: gebieden}",
            1,
        )
    };
    let opstelling = eigen_opstelling(
        &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
        &[("afnemer", &uit_ander)],
    );
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert!(
        fouten.contains(&"proces 'test_afnemer_proces': toets, rijen 'gebiedstabel': de tabel komt uit lexostatus 'zaakverloop', en die is niet de toets-lexostatus".to_string()),
        "{fouten:?}"
    );
}

// --- Tijd: twee tijden per gram, en een peil op de reductie ---

/// Een aanvraag die langs een andere weg binnenkwam: het loket geeft de dag
/// van ontvangst op (`$intake.ontvangen_op`). Rechtens telt die dag (de
/// aanvraagdatum), vastgelegd wordt op de klok van de cel.
#[tokio::test]
async fn een_eerdere_ontvangst_is_de_aanvraagdatum() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let app = rt.router.clone();
    let mut v = verzoek("test_instantie");
    v["intake"] = json!({"kanaal": "loket", "ontvangen_op": "2025-03-05",
                         "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"}});
    let (status, body) = als_runtime(&rt, "POST", &format!("{INSTANTIE_CEL}/api/grammen"), v).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let g = &body["gram"];
    schema::valideer(Soort::Gram, g).unwrap();
    assert_eq!(g["op_moment"], "2025-03-05T00:00:00+01:00");
    assert_eq!(g["vastgelegd_op"], "2025-03-12T10:14:03+01:00");
    assert_eq!(g["op_moment_grondslag"], json!(["testregeling_aanvraag#1"]));
    let zaak = g["zaakkenmerk"].as_str().unwrap();
    let (status, l, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud?zaakkenmerk={zaak}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(l["parameters"]["aanvraagdatum"], "2025-03-05");
    assert_eq!(l["vastgelegd_op"], "2025-03-12T10:14:03+01:00");

    // Een ontvangst na het vastleggen is geen feit.
    let mut v = verzoek("test_instantie");
    v["intake"]["ontvangen_op"] = json!("2025-03-20");
    let (status, body) = als_runtime(&rt, "POST", &format!("{INSTANTIE_CEL}/api/grammen"), v).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["fout"].as_str().unwrap().contains("na het vastleggen"));
}

/// `GET lexostatus` met een peil: dezelfde kroniek geeft op een ander moment
/// een andere lexostatus. De startstand geldt rechtens op haar eigen
/// momenten, maar is pas bekend sinds ze geladen is.
#[tokio::test]
async fn lexostatus_op_een_peilmoment() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let status_op = |query: &'static str| {
        let app = app.clone();
        async move {
            let (s, l, _) = vraag(
                &app,
                "GET",
                &format!("/cellen/test_register/api/lexostatus/registerstatus?aanduiding=VOORBEELD{query}"),
                None,
                None,
            )
            .await;
            (s, l)
        }
    };
    let (s, nu) = status_op("").await;
    assert_eq!(s, StatusCode::OK, "{nu}");
    assert_eq!(nu["parameters"]["zetels_op_lijst"], json!(6));
    assert!(nu.get("peilmoment").is_none());

    // Rechtens op 1 januari 2024: nog niet ingeschreven, geen uitslag.
    let (s, jan) = status_op("&peilmoment=2024-01-01").await;
    assert_eq!(s, StatusCode::OK, "{jan}");
    assert_eq!(jan["peilmoment"], "2024-01-01");
    assert_eq!(jan["parameters"]["is_ingeschreven_raad"], json!(false));
    assert_eq!(jan["parameters"]["zetels_op_lijst"], json!(0));
    // Een moment met tijdzone kan ook (in een query als %2B voor '+').
    let (s, apr) = status_op("&peilmoment=2024-04-01T00:00:00%2B02:00").await;
    assert_eq!(s, StatusCode::OK, "{apr}");
    assert_eq!(apr["parameters"]["zetels_op_lijst"], json!(6));
    assert_eq!(apr["parameters"]["is_ingeschreven_raad"], json!(true));

    // Zoals bekend voor het laden van de startstand (de klok van de test):
    // de cel wist toen nog niets.
    let (s, eerder) = status_op("&bekend_op=2025-03-11").await;
    assert_eq!(s, StatusCode::OK, "{eerder}");
    assert_eq!(eerder["parameters"]["is_ingeschreven_raad"], json!(false));
    let (_, toen) = status_op("&bekend_op=2025-03-12").await;
    assert_eq!(toen["parameters"], nu["parameters"]);

    let (s, f) = status_op("&peilmoment=morgen").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(f["fout"].as_str().unwrap().contains("ongeldig peilmoment"));
}

/// Ook de proefroute peilt: een concept telt als vastgelegd op de klok van
/// nu, dus zoals bekend op een eerdere dag ligt het er niet.
#[tokio::test]
async fn de_proefroute_peilt_ook() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let proef = format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud/proef");
    let (status, body) = als_runtime(
        &rt,
        "POST",
        &proef,
        json!({"concept": verzoek("test_instantie"), "inputs": {"peilmoment": "2025-03-12"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lexostatus"]["peilmoment"], "2025-03-12");
    let (status, _) = als_runtime(
        &rt,
        "POST",
        &proef,
        json!({"concept": verzoek("test_instantie"), "inputs": {"bekend_op": "2025-03-11"}}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Een kroniek van voor `vastgelegd_op` laadt: het gram krijgt zijn
/// `op_moment` als registratietijd.
#[tokio::test]
async fn een_oude_kroniek_zonder_vastgelegd_op_laadt() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let (status, body) = als_runtime(
        &rt,
        "POST",
        &format!("{INSTANTIE_CEL}/api/grammen"),
        verzoek("test_instantie"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    drop(rt);
    // Het gram zoals een eerdere versie van de runtime het schreef.
    let pad = dir.path().join("test_instantie/test_kroniek.jsonl");
    let mut oud: Value =
        serde_json::from_str(std::fs::read_to_string(&pad).unwrap().trim()).unwrap();
    oud.as_object_mut().unwrap().remove("vastgelegd_op");
    oud["op_moment"] = json!("2025-03-01T09:00:00+01:00");
    std::fs::write(&pad, format!("{oud}\n")).unwrap();

    let app = app(dir.path());
    let (status, k, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/kroniek"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{k}");
    assert_eq!(k[0]["gram"]["vastgelegd_op"], "2025-03-01T09:00:00+01:00");
    schema::valideer(Soort::Gram, &k[0]["gram"]).unwrap();
}

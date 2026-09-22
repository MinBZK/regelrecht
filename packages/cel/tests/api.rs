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
use serde_json::{json, Value};
use tower::ServiceExt;

const INSTANTIE: &str = "/cellen/test_instantie";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn klok() -> Klok {
    Arc::new(|| DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap())
}

fn runtime_op(cellen: &Path, data: &Path) -> Result<Runtime, Vec<String>> {
    let config = Config {
        cells_path: cellen.to_path_buf(),
        regulation_path: fixtures().join("regulation"),
        data_dir: data.to_path_buf(),
        port: STANDAARD_POORT,
    };
    Runtime::laad(&config, klok())
}

fn app(data: &Path) -> Router {
    runtime_op(&fixtures().join("cellen"), data).unwrap().router
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
        &format!("{INSTANTIE}/api/eherkenning/login"),
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
        &format!("{INSTANTIE}/api/eherkenning/login"),
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
    let (status, body, _) =
        vraag(&app, "GET", &format!("{INSTANTIE}/api/stroom"), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["event"], "aanvraag_ontvangen");
    assert_eq!(body["velden"][0]["label"], "Naam van de aanvrager");
    schema::valideer(Soort::Stroom, &body["stroom"]).unwrap();
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
        &format!("{INSTANTIE}/api/kroniek"),
        Some(&c),
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
        &format!("{INSTANTIE}/api/kroniek"),
        Some(&c),
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

    let (_, kroniek, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/kroniek"),
        Some(&c),
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
        &format!("{INSTANTIE}/api/lexostatus/aanvraag_inhoud?zaakkenmerk={zaak}"),
        Some(&c),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lexo}");
    assert_eq!(lexo["parameters"]["bevat_naam"], json!(true));

    // Een andere KvK ziet deze grammen en deze zaak niet.
    let ander = inloggen(&app, "87654321").await;
    let (_, kroniek, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/kroniek"),
        Some(&ander),
        None,
    )
    .await;
    assert_eq!(kroniek, json!([]));
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/lexostatus/aanvraag_inhoud?zaakkenmerk={zaak}"),
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
        &format!("{INSTANTIE}/api/lexostatus/bestaat_niet?zaakkenmerk=x"),
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
    assert_eq!(ids, ["test_afnemer", "test_instantie", "test_register"]);
    let register = &body[2];
    assert_eq!(register["portaal"], json!(false));
    assert_eq!(register["lexostatussen"][0]["name"], "registerstatus");
    assert_eq!(
        register["lexostatussen"][0]["inputs"][0]["name"],
        "aanduiding"
    );
    assert_eq!(body[0]["synthese"][0]["transport"], "intern");
    assert_eq!(
        body[0]["lexostatussen"][0]["extra_velden"],
        json!(["aanduiding"])
    );
}

#[tokio::test]
async fn cel_zonder_portaal_heeft_geen_login_of_aanvraag() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    for (methode, pad) in [
        ("POST", "/cellen/test_register/api/eherkenning/login"),
        ("GET", "/cellen/test_register/api/stroom"),
        ("POST", "/cellen/test_register/api/aanvraag/toets"),
        ("POST", "/cellen/test_register/api/aanvraag"),
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
        json!({"is_ingeschreven_raad": true, "is_geschrapt_raad": false,
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
        "/cellen/test_afnemer/api/eherkenning/login",
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester", "machtiging": "volledig"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = vraag(
        app,
        "POST",
        "/cellen/test_afnemer/api/aanvraag/toets",
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
        json!(["datum_mededeling"])
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

/// Kopieer een fixture-cel en alle stromen naar een eigen CELLS_PATH, met
/// een aanpassing aan haar cel.yaml.
fn eigen_cellen(cellen: &[(&str, &dyn Fn(String) -> String)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let f = fixtures();
    std::fs::create_dir_all(dir.path().join("chronicles")).unwrap();
    for e in std::fs::read_dir(f.join("chronicles")).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(
            &p,
            dir.path().join("chronicles").join(p.file_name().unwrap()),
        )
        .unwrap();
    }
    for (cel, pas_aan) in cellen {
        let doel = dir.path().join("cellen").join(cel);
        std::fs::create_dir_all(&doel).unwrap();
        for e in std::fs::read_dir(f.join("cellen").join(cel)).unwrap() {
            let p = e.unwrap().path();
            let tekst = std::fs::read_to_string(&p).unwrap();
            let tekst = if p.file_name().unwrap() == "cel.yaml" {
                pas_aan(tekst)
            } else {
                tekst
            };
            std::fs::write(doel.join(p.file_name().unwrap()), tekst).unwrap();
        }
    }
    dir
}

fn met_url(url: String) -> impl Fn(String) -> String {
    move |t: String| {
        t.replace(
            "  - cel: test_register\n",
            &format!("  - cel: test_register\n    url: {url}\n"),
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
    let cellen = eigen_cellen(&[("afnemer", &aanpassing)]);
    let data_a = tempfile::tempdir().unwrap();
    let a = runtime_op(&cellen.path().join("cellen"), data_a.path()).unwrap();
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
    let cellen = eigen_cellen(&[("afnemer", &aanpassing)]);
    let data = tempfile::tempdir().unwrap();
    // De bron mag later komen: de runtime start wel, met een waarschuwing.
    let a = runtime_op(&cellen.path().join("cellen"), data.path()).unwrap();
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
    let niets = |t: String| t;
    let cellen = eigen_cellen(&[("afnemer", &niets)]);
    let data = tempfile::tempdir().unwrap();
    let a = runtime_op(&cellen.path().join("cellen"), data.path()).unwrap();
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
    let cellen = eigen_cellen(&[("afnemer", &|t: String| t), ("register", &|t: String| t)]);
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
    let a = runtime_op(&cellen.path().join("cellen"), data.path()).unwrap();
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
        let cellen = eigen_cellen(&[("afnemer", aanpassing), ("register", &|t: String| t)]);
        let data = tempfile::tempdir().unwrap();
        let fouten = runtime_op(&cellen.path().join("cellen"), data.path())
            .err()
            .unwrap();
        assert!(
            fouten
                .iter()
                .any(|f| f.starts_with("cel 'test_afnemer': ") && f.contains(verwacht)),
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
    // Synthese zonder portaal.
    geval(
        &|t: String| {
            let (voor, na) = t.split_once("portaal:").unwrap();
            let (_, synthese) = na.split_once("synthese:").unwrap();
            format!("{voor}synthese:{synthese}")
        },
        "synthese zonder portaal",
    );
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
    let cellen = eigen_cellen(&[("instantie", &|t: String| t)]);
    let stroom = cellen.path().join("chronicles/test_aanvragen.yaml");
    let tekst = std::fs::read_to_string(&stroom).unwrap();
    std::fs::write(&stroom, tekst.replace("zaak: opent", "zaak: volgt")).unwrap();
    let app = runtime_op(&cellen.path().join("cellen"), data.path())
        .unwrap()
        .router;
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
    // Een andere KvK kent deze zaak niet.
    let ander = inloggen(&app, "87654321").await;
    let (status, _, _) = vraag(&app, "POST", &aanvraag, Some(&ander), Some(volgt.clone())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
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
    let cellen = eigen_cellen(&[("instantie", &|t: String| t), ("register", &kapot)]);
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(&cellen.path().join("cellen"), data.path())
        .err()
        .unwrap();
    assert_eq!(fouten.len(), 1, "{fouten:?}");
    assert!(fouten[0].starts_with("cel 'test_register': "), "{fouten:?}");
}

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
use regelrecht_cel::config::{Config, Reductiemodus, STANDAARD_POORT};
use regelrecht_cel::runtime::Runtime;
use regelrecht_cel::schema::{self, Soort};
use regelrecht_cel::transport::{LEES_TOKEN_HEADER, RUNTIME_TOKEN_HEADER};
use serde_json::{json, Value};
use tower::ServiceExt;

/// Het proces met een portaal, en de cel waarin het vastlegt.
const INSTANTIE: &str = "/processes/test_instantie_proces";
const INSTANTIE_CEL: &str = "/cells/test_instantie";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn klok() -> Klok {
    Arc::new(|| DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap())
}

/// Een runtime over `<opstelling>/cellen` en `<opstelling>/processes`.
fn runtime_op(opstelling: &Path, data: &Path) -> Result<Runtime, Vec<String>> {
    let config = Config {
        cells_path: opstelling.join("cells"),
        processes_path: Some(opstelling.join("processes")),
        regulation_path: fixtures().join("regulation"),
        data_dir: data.to_path_buf(),
        port: STANDAARD_POORT,
        lees_token: None,
        lees_token_bronnen: Vec::new(),
        reduction: Default::default(),
        registers: None,
    };
    Runtime::laad(&config, klok())
}

/// Een runtime zoals [`runtime_op`], met een leestoken.
fn runtime_met_leestoken(opstelling: &Path, data: &Path, token: &str, sources: &[&str]) -> Runtime {
    let config = Config {
        cells_path: opstelling.join("cells"),
        processes_path: Some(opstelling.join("processes")),
        regulation_path: fixtures().join("regulation"),
        data_dir: data.to_path_buf(),
        port: STANDAARD_POORT,
        lees_token: Some(token.to_string()),
        lees_token_bronnen: sources.iter().map(|b| b.to_string()).collect(),
        reduction: Default::default(),
        registers: None,
    };
    Runtime::laad(&config, klok()).unwrap()
}

fn app(data: &Path) -> Router {
    als_lezer(&runtime_op(&fixtures(), data).unwrap())
}

/// De router van een runtime waarvan de tests de leesroutes van een cel
/// lezen zoals een proces van die runtime: een `GET` onder `/cellen/`
/// krijgt het runtime-token mee. De leesroutes zijn niet open (zie
/// `de_leesroutes_van_een_cel_zijn_niet_open`).
fn als_lezer(rt: &Runtime) -> Router {
    let token = rt.runtime_token.als_str().to_string();
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
    let (status, _, cookie) = vraag(
        app,
        "POST",
        &format!("{INSTANTIE}/api/channels/eherkenning/login"),
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
        &format!("{INSTANTIE}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "123", "persoon": "A"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("acht cijfers"));
}

#[tokio::test]
async fn formulier_geeft_velden_met_labels() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(&app, "GET", &format!("{INSTANTIE}/api/form"), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cell"], "test_instantie");
    assert_eq!(body["event"], "aanvraag_ontvangen");
    assert_eq!(body["fields"][0]["label"], "Naam van de aanvrager");
    schema::valideer(Soort::Stroom, &body["stream"]).unwrap();
}

#[tokio::test]
async fn de_cel_geeft_haar_stromen_met_hun_hash() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(&app, "GET", "/cells/test_afnemer/api/stream", None, None).await;
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
        schema::valideer(Soort::Stroom, &s["stream"]).unwrap();
    }
}

#[tokio::test]
async fn toets_zonder_login_mag_niet() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application/assessment"),
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
    let c = logins(&app, "12345678").await;

    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application/assessment"),
        Some(&c),
        Some(volledig()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["result"]["value"], json!(true), "{body}");
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
        &format!("{INSTANTIE}/api/application/assessment"),
        Some(&c),
        Some(onvolledig),
    )
    .await;
    assert_eq!(body["result"]["value"], json!(false), "{body}");
    assert_eq!(body["result"]["absent"], json!(["bevat_aanduiding"]));

    let mut zonder_jaar = volledig();
    zonder_jaar["external"]
        .as_object_mut()
        .unwrap()
        .remove("aanvraagjaar");
    let (_, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application/assessment"),
        Some(&c),
        Some(zonder_jaar),
    )
    .await;
    assert_eq!(body["result"]["to_assess"], json!(false));
    assert_eq!(
        body["result"]["reason"],
        "niet te beoordelen: mist aanvraagjaar"
    );

    // Een toets legt niets vast.
    let (_, chronicle, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(chronicle, json!([]));
}

#[tokio::test]
async fn onbekend_veld_wordt_geweigerd() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = logins(&app, "12345678").await;
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application"),
        Some(&c),
        Some(json!({"external": {"schoenmaat": 44}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("schoenmaat"));
}

#[tokio::test]
async fn onbekende_tabelkolom_wordt_geweigerd() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = logins(&app, "12345678").await;
    let mut body = volledig();
    body["external"]["organen"][1]["kleur"] = json!("rood");
    let (status, antwoord, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application"),
        Some(&c),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        antwoord["error"]
            .as_str()
            .unwrap()
            .contains("onbekend veld 'organen[1].kleur'"),
        "{antwoord}"
    );
    // Er is niets vastgelegd.
    let (_, chronicle, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(chronicle, json!([]));
}

#[tokio::test]
async fn indienen_legt_een_gram_vast_per_kvk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = logins(&app, "12345678").await;

    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application"),
        Some(&c),
        Some(volledig()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gram = &body["gram"];
    schema::valideer(Soort::Gram, gram).unwrap();
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
    // Velden in de volgorde van de stroom: kern voor inhoud.
    assert!(yaml.find("core:").unwrap() < yaml.find("content:").unwrap());
    let case = gram["id"].as_str().unwrap().to_string();

    // Een onvolledige aanvraag wordt ook vastgelegd, in een nieuwe zaak.
    let mut onvolledig = volledig();
    onvolledig["external"]
        .as_object_mut()
        .unwrap()
        .remove("adres");
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application"),
        Some(&c),
        Some(onvolledig),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        body["gram"]["fields"]["core"]["aanvrager"]["adres"],
        Value::Null
    );
    assert_ne!(body["gram"]["id"], json!(case));

    // De kroniek en de lexostatussen zijn van de cel, zonder login: tussen
    // een afnemer en de cel is geen beveiligingscontext.
    let (_, chronicle, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(chronicle.as_array().unwrap().len(), 2);
    for item in chronicle.as_array().unwrap() {
        schema::valideer(Soort::Gram, &item["gram"]).unwrap();
    }
    let rows =
        std::fs::read_to_string(dir.path().join("test_instantie/test_kroniek.jsonl")).unwrap();
    assert_eq!(rows.lines().count(), 2);

    let (status, lexo, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud?root={case}"),
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
        &format!("{INSTANTIE}/api/chronicle"),
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
    let c = logins(&app, "12345678").await;
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/lexostatus/bestaat_niet?root=x"),
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
        ("cells/instantie/lexostatuses.yaml", Soort::Lexostatus),
        ("cells/register/lexostatuses.yaml", Soort::Lexostatus),
        ("cells/afnemer/lexostatuses.yaml", Soort::Lexostatus),
        ("cells/instantie/cell.yaml", Soort::Cell),
        ("cells/register/cell.yaml", Soort::Cell),
        ("cells/afnemer/cell.yaml", Soort::Cell),
        ("cells/gebieden/cell.yaml", Soort::Cell),
        ("processes/instantie/process.yaml", Soort::Proces),
        ("processes/afnemer/process.yaml", Soort::Proces),
    ] {
        let doc: Value =
            serde_yaml_ng::from_str(&std::fs::read_to_string(f.join(bestand)).unwrap()).unwrap();
        let result = schema::valideer(soort, &doc);
        assert!(result.is_ok(), "{bestand}: {result:?}");
    }
}

// --- De runtime: meer cellen, elk met een eigen kroniek ---

#[tokio::test]
async fn cellen_worden_opgesomd_met_hun_mogelijkheden() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(&app, "GET", "/api/cells", None, None).await;
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
    // Een cel zegt niets over wie er handelt: dat staat bij het proces.
    for sleutel in ["portal", "rollen", "behandeling", "synthese"] {
        assert!(body[0].get(sleutel).is_none(), "{sleutel}");
    }
}

#[tokio::test]
async fn processen_worden_opgesomd_met_hun_cel() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) = vraag(&app, "GET", "/api/processes", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let afnemer = &body[0];
    assert_eq!(afnemer["id"], "test_afnemer_proces");
    assert_eq!(afnemer["actor"], "test_afnemer");
    assert_eq!(afnemer["cell"], "test_afnemer");
    assert_eq!(afnemer["portal"], json!(true));
    // De eigen cel is een bron zoals de andere, voor de zaak.
    assert_eq!(
        afnemer["synthesis"][0],
        json!({"cell": "test_afnemer", "lexostatus": "aanvraag_inhoud", "case": true, "transport": "internal", "parameters": []})
    );
    assert_eq!(afnemer["synthesis"][3]["cell"], "test_register");
    assert_eq!(afnemer["synthesis"][3]["transport"], "internal");
    let instantie = &body[1];
    assert_eq!(instantie["id"], "test_instantie_proces");
    assert_eq!(instantie["handling"], Value::Null);
    assert_eq!(
        instantie["roles"],
        json!({
            "aanvrager": {"channel": "eherkenning", "routes": ["portal"], "label": "aanvrager"},
            "burger": {"channel": "burger", "routes": ["portal"], "label": "burger"},
            "loket": {"channel": "medewerker", "routes": ["counter"], "label": "Loketmedewerker"},
        })
    );
    assert_eq!(instantie["counter"], json!(true));
    assert_eq!(instantie["authority"], Value::Null);
    // De kanalen met hun velden: de frontend bouwt er het inlogscherm uit.
    assert_eq!(
        instantie["channels"]["burger"]["fields"][0]["check"],
        "elfproef"
    );
    assert_eq!(instantie["channels"]["burger"]["owner"], "nummer");
    assert_eq!(afnemer["authority"], "Test afnemer");
    assert_eq!(afnemer["counter"], json!(false));
}

#[tokio::test]
async fn een_cel_heeft_geen_login_of_aanvraag() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    for (methode, path) in [
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
        let (status, _, _) = vraag(&app, methode, path, None, Some(json!({}))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{methode} {path}");
    }
}

#[tokio::test]
async fn startstand_in_een_lege_kroniek_en_niet_nog_eens() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, chronicle, _) = vraag(
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
        // Een registerbesluit hoort bij geen zaak.
        assert!(g["gram"].get("zaakkenmerk").is_none(), "{g}");
        assert!(!g["yaml"].as_str().unwrap().contains("zaakkenmerk"));
        schema::valideer(Soort::Gram, &g["gram"]).unwrap();
    }
    // De kroniek staat in de eigen map van de cel.
    let path = dir.path().join("test_register/test_register.jsonl");
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 4);
    // Een tweede start voegt niets toe: de kroniek is niet meer leeg.
    drop(app);
    let _ = self::app(dir.path());
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 4);
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
        "/cells/test_register/api/lexostatus/registerstatus?aanduiding=VOORBEELD",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    // De registercel spreekt de taal van haar eigen regeling.
    assert_eq!(
        l["parameters"],
        json!({"jaar_van_mededeling": 2024, "zetels_toegewezen": 6,
               "datum_mededeling": "2024-11-01", "geblokkeerd": false})
    );
    let (status, l, _) = vraag(
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
    let (status, l, _) = vraag(
        &app,
        "GET",
        "/cells/test_register/api/lexostatus/registerstatus",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(l["error"], "input 'aanduiding' ontbreekt");
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
        &format!("{AFNEMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/application/assessment"),
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
    assert_eq!(body["result"]["value"], json!(true), "{body}");
    assert_eq!(
        body["provenance"]["bevat_aanduiding"],
        json!({"source": "own", "lexostatus": "aanvraag_inhoud"})
    );
    assert_eq!(
        body["provenance"]["zetels_op_lijst"],
        json!({"source": "cell", "cell": "test_register", "lexostatus": "registerstatus", "transport": "internal"})
    );
    // De afnemer vraagt het register onder zijn eigen naam; de bron levert
    // in de taal van haar regeling, met het orgaan als vaste invoer.
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
    // De aanduiding is geen parameter: ze staat apart in de eigen lexostatus.
    assert_eq!(
        body["lexostatus"]["extra_fields"]["aanduiding"],
        "VOORBEELD"
    );
    assert!(body["provenance"].get("aanduiding").is_none());
    // Niets uit de synthese wordt vastgelegd, niet bij de afnemer en niet bij
    // het register.
    assert!(!dir.path().join("test_afnemer/test_afnemer.jsonl").exists());
    let rows = std::fs::read_to_string(dir.path().join("test_register/test_register.jsonl"));
    assert_eq!(rows.unwrap().lines().count(), 4);
}

#[tokio::test]
async fn synthese_zonder_registratie_en_zonder_invoer() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    // Een onbekende aanduiding: niet ingeschreven en geen zetels, dus niet
    // toelaatbaar. Een mededeling is er niet; die datum vult niemand aan.
    let body = afnemer_toets(&app, Some("ONBEKEND")).await;
    assert_eq!(body["result"]["value"], json!(false), "{body}");
    assert_eq!(body["provenance"]["is_ingeschreven_raad"]["source"], "cell");
    // Wat de bron niet leverde, onder haar eigen naam.
    assert_eq!(
        body["sources"][1]["not_delivered"],
        json!(["datum_mededeling", "jaar_van_mededeling"])
    );
    assert!(body["provenance"].get("datum_mededeling").is_none());
    // Zonder aanduiding wordt de bron niet bevraagd.
    let body = afnemer_toets(&app, None).await;
    assert_eq!(body["sources"][0]["status"], "not_queried");
    assert_eq!(body["result"]["absent"], json!(["bevat_aanduiding"]));
    assert!(body["result"]["reason"]
        .as_str()
        .unwrap()
        .starts_with("niet te beoordelen: bron test_register niet bevraagd"));
}

/// Een aanpassing aan een `cel.yaml` of `proces.yaml`.
type Aanpassing<'a> = (&'a str, &'a dyn Fn(String) -> String);

/// Kopieer fixture-cellen, fixture-processen en alle stromen naar een eigen
/// opstelling (`cellen/`, `processes/`, `chronicles/`), met een aanpassing
/// aan elke `cel.yaml` en `proces.yaml`.
fn eigen_opstelling(cells: &[Aanpassing], processen: &[Aanpassing]) -> tempfile::TempDir {
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
    for (soort, list, bestand) in [
        ("cells", cells, "cell.yaml"),
        ("processes", processen, "process.yaml"),
    ] {
        for (name, pas_aan) in list {
            let doel = dir.path().join(soort).join(name);
            std::fs::create_dir_all(&doel).unwrap();
            for e in std::fs::read_dir(f.join(soort).join(name)).unwrap() {
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
        // Alleen de synthese-bronnen, niet de bron onder rijen.
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
async fn synthese_over_http_naar_een_andere_runtime() {
    // Runtime B: de fixtures, op een echte poort. A leest B met het gedeelde
    // leestoken; zonder leest alleen B zelf.
    let token = "gedeeld-leestoken-van-de-test";
    let data_b = tempfile::tempdir().unwrap();
    let b = runtime_met_leestoken(&fixtures(), data_b.path(), token, &[]).router;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let adres = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, b).await });

    // Runtime A: alleen de afnemer, met een url naar B.
    let aanpassing = met_url(format!("http://{adres}"));
    let opstelling = eigen_opstelling(&[("afnemer", &zo)], &[("afnemer", &aanpassing)]);
    let data_a = tempfile::tempdir().unwrap();
    // Zonder B onder de bronnen van het leestoken krijgt B het niet mee.
    let data_z = tempfile::tempdir().unwrap();
    let without = runtime_met_leestoken(opstelling.path(), data_z.path(), token, &[]);
    let body = afnemer_toets(&without.router, Some("VOORBEELD")).await;
    assert_ne!(body["result"]["value"], json!(true), "{body}");
    let a = runtime_met_leestoken(
        opstelling.path(),
        data_a.path(),
        token,
        &[&format!("http://{adres}")],
    );
    // Wat de runtime van een bron buiten haar niet kan zien, meldt ze (de
    // herkomst, RFC-043); verder niets.
    let w = a.warnings().await;
    assert_eq!(w.len(), 2, "{w:?}");
    for w in &w {
        assert!(
            w.starts_with("proces 'test_afnemer_proces': herkomst van ")
                && w.contains(&format!("draait buiten deze runtime (http://{adres})")),
            "{w:?}"
        );
    }
    let body = afnemer_toets(&a.router, Some("VOORBEELD")).await;
    assert_eq!(body["result"]["value"], json!(true), "{body}");
    assert_eq!(
        body["provenance"]["is_ingeschreven_raad"]["transport"],
        "http"
    );
    assert_eq!(body["sources"][0]["transport"], "http");
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
    let w = a.warnings().await;
    assert!(
        w.iter().any(|w| w.contains("nu niet te controleren")),
        "{w:?}"
    );
    let body = afnemer_toets(&a.router, Some("VOORBEELD")).await;
    assert_eq!(body["result"]["to_assess"], json!(false));
    assert_eq!(
        body["result"]["reason"],
        "niet te beoordelen: bron test_register onbereikbaar"
    );
    assert_eq!(body["sources"][0]["status"], "unreachable");
    // Er is niets aangevuld.
    assert!(body["provenance"].get("is_ingeschreven_raad").is_none());
}

#[tokio::test]
async fn interne_bron_die_niet_draait_is_een_waarschuwing() {
    let opstelling = eigen_opstelling(&[("afnemer", &zo)], &[("afnemer", &zo)]);
    let data = tempfile::tempdir().unwrap();
    let a = runtime_op(opstelling.path(), data.path()).unwrap();
    let w = a.warnings().await;
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
    let cells = eigen_opstelling(&[("afnemer", &zo), ("register", &zo)], &[("afnemer", &zo)]);
    let lexo = cells.path().join("cells/register/lexostatuses.yaml");
    let tekst = std::fs::read_to_string(&lexo).unwrap();
    std::fs::write(
        &lexo,
        tekst.replace(
            "        is_geschrapt:\n          filter: {name: aanduiding_geschrapt, orgaan: $orgaan, aanduiding: $aanduiding}\n          exists: true\n          legal_basis: [testregeling_register#1 lid 1]\n",
            "",
        ),
    )
    .unwrap();
    // `aanduiding_geschrapt` leest dan niemand meer: markeer het event.
    let stream = cells.path().join("chronicles/test_registers.yaml");
    let tekst = std::fs::read_to_string(&stream).unwrap();
    std::fs::write(
        &stream,
        tekst.replace(
            "      orgaan: $external.orgaan\n  - name: uitslag_vastgesteld",
            "      orgaan: $external.orgaan\n    not_reduced:\n      - {field: aanduiding, reason: test}\n      - {field: orgaan, reason: test}\n  - name: uitslag_vastgesteld",
        ),
    )
    .unwrap();
    let data = tempfile::tempdir().unwrap();
    let a = runtime_op(cells.path(), data.path()).unwrap();
    let w = a.warnings().await;
    assert!(
        w.iter()
            .any(|w| w.contains("de bron levert geen parameter 'is_geschrapt'")),
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
                "{is_ingeschreven_in_register: is_ingeschreven_raad,",
                "{uitslag_openbaar: uitslag_openbaar, is_ingeschreven_in_register: is_ingeschreven_raad,",
            )
        },
        "'uitslag_openbaar' is geen parameter van testregeling_afnemer#1",
    );
    // Een parameter uit twee bronnen: de eigen reductie en de synthese.
    geval(
        &|t: String| {
            t.replace(
                "{is_ingeschreven_in_register: is_ingeschreven_raad,",
                "{bevat_aanduiding: bevat_aanduiding, is_ingeschreven_in_register: is_ingeschreven_raad,",
            )
        },
        "parameter 'bevat_aanduiding' komt uit meer dan een bron",
    );
    // Een invoer uit een veld dat de toets-lexostatus niet levert.
    geval(
        &|t: String| t.replace("field: aanduiding}", "field: aanduiding_x}"),
        "levert geen 'aanduiding_x'",
    );
    // Synthese zonder portaal en zonder besluit.
    geval(
        &|t: String| {
            let (voor, after) = t.split_once("roles:").unwrap();
            let (_, synthesis) = after.split_once("synthesis:").unwrap();
            let (synthesis, _) = synthesis.split_once("handling:").unwrap();
            format!("{voor}synthesis:{synthesis}")
        },
        "synthese zonder portaal en zonder handelingen",
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
        &|t: String| t.replace("routes: [handling]", "routes: [counter]"),
        "behandeling zonder rol die het mag",
    );
    geval(
        &|t: String| {
            t.replace(
                "  aanvrager: {channel: eherkenning, routes: [portal]}\n",
                "",
            )
        },
        "portaal zonder rol die het mag",
    );
    geval(
        &|t: String| {
            t.replace(
                "{channel: medewerker, routes: [handling]",
                "{channel: balie, routes: [handling]",
            )
        },
        "rol 'behandelaar': kanaal 'balie' staat niet onder kanalen",
    );
    // Het portaal-event bindt op_moment niet aan $intake: geen loket.
    geval(
        &|t: String| t.replace("routes: [handling]", "routes: [handling, counter]"),
        "loket: event 'aanvraag_ontvangen' bindt op_moment niet aan $intake",
    );
    // Namens een gezag dat de wet niet kent, of geen gezag bij een besluit.
    geval(
        &|t: String| {
            t.replace(
                "on_behalf_of: {regulation: testregeling_afnemer}",
                "on_behalf_of: {authority: test_afnemer}",
            )
        },
        "on_behalf_of: geen geladen regeling noemt 'test_afnemer' als bevoegd gezag",
    );
    geval(
        &|t: String| t.replace("on_behalf_of: {regulation: testregeling_afnemer}\n", ""),
        "behandeling zonder on_behalf_of",
    );
    geval(
        &|t: String| {
            t.replace(
                "on_behalf_of: {regulation: testregeling_afnemer}\n",
                "on_behalf_of: {regulation: testregeling_afnemer}\nmandates:\n  - {authority: Test afnemer, legal_basis: 'testregeling_afnemer#9'}\n",
            )
        },
        "mandaat: 'Test afnemer' is het gezag waarvoor het proces zelf handelt",
    );
    geval(
        &|t: String| t.replace("lexostatus: werkvoorraad}", "lexostatus: aanvraag_inhoud}"),
        "werkvoorraad 'aanvraag_inhoud' is geen lijst",
    );
    geval(
        &|t: String| t.replace("outputs: [vastgesteld_bedrag,", "outputs: [bestaat_niet,"),
        "heeft geen uitkomst 'bestaat_niet'",
    );
    geval(
        &|t: String| {
            t.replace(
                "lexostatus: zaakverloop, case: true}",
                "lexostatus: werkvoorraad, case: true}",
            )
        },
        "lexostatus 'werkvoorraad' is een lijst",
    );
    // Een bron van de zaak in een andere cel dan die van het proces.
    geval(
        &|t: String| {
            t.replace(
                "{cell: test_afnemer, lexostatus: zaakverloop, case: true}",
                "{cell: test_register, lexostatus: zaakverloop, case: true}",
            )
        },
        "cel 'test_register', en het proces legt vast in cel 'test_afnemer'",
    );
    // Een gewone bron uit de eigen cel: dat is een bron van de zaak.
    geval(
        &|t: String| {
            t.replace(
                "  - cell: test_register\n    lexostatus: registerstatus\n",
                "  - cell: test_afnemer\n    lexostatus: registerstatus\n",
            )
        },
        "is een bron van de zaak (zaak: true)",
    );
    // De stand bij besluit staat niet meer in de configuratie: ze volgt uit
    // de procedure van de beschikking. Het schema weigert haar.
    let met_stand = |t: String| {
        t.replacen(
            "      record: {cell: test_afnemer, stream: test_afnemer_zaakverloop, event: besluit_genomen}",
            "      stand_bij_besluit: {bekendgemaakt: false}\n      record: {cell: test_afnemer, stream: test_afnemer_zaakverloop, event: besluit_genomen}",
            1,
        )
    };
    let opstelling = eigen_opstelling(
        &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
        &[("afnemer", &met_stand)],
    );
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert!(
        fouten
            .iter()
            .any(|f| f.contains("'stand_bij_besluit' was unexpected")),
        "{fouten:?}"
    );
    // Synthese per regel.
    geval(
        &|t: String| {
            t.replace(
                "          table: {lexostatus: aanvraag_inhoud, field: gebieden}",
                "          table: {lexostatus: aanvraag_inhoud, field: dorpen}",
            )
        },
        "lexostatus 'aanvraag_inhoud' levert geen 'dorpen'",
    );
    geval(
        &|t: String| t.replace("          table: {lexostatus: aanvraag_inhoud, field: gebieden}", "          table: {lexostatus: werkvoorraad, field: gebieden}"),
        "de tabel komt uit lexostatus 'werkvoorraad', en die is geen lexostatus van de zaak (zaak: true)",
    );
    geval(
        &|t: String| {
            t.replace(
                "                gebied: {column: gebied}\n                peildatum:",
                "                gebied: {column: gebiedje}\n                peildatum:",
            )
        },
        "kolom 'gebiedje' wordt door niets ervoor gevuld",
    );
    geval(
        &|t: String| t.replace("- parameter: gebiedstabel", "- parameter: dorpstabel"),
        "'besluit', rijen: 'dorpstabel' is geen parameter van testregeling_afnemer#3",
    );
    geval(
        &|t: String| {
            t.replace(
                "              columns: {tarief: tarief}",
                "              columns: {tarief: zetels}",
            )
        },
        "kolom 'zetels' komt uit meer dan een plek: de tabel, bron test_gebieden/tarief",
    );
    // Waar het besluit wordt vastgelegd: elk veld van het event is een
    // uitkomst of een oordeel.
    geval(
        &|t: String| t.replace("      outputs: [vastgesteld_bedrag, gebiedsbedrag,", "      outputs: [vastgesteld_bedrag,"),
        "het event legt [gebiedsbedrag] vast, en dat is geen uitkomst en geen oordeel van het besluit",
    );
}

// --- De behandelaar: werkvoorraad, zaak en proefbesluit ---

const AFNEMER: &str = "/processes/test_afnemer_proces";
const AFNEMER_CEL: &str = "/cells/test_afnemer";

async fn afnemer_indienen(app: &Router, kvk: &str) -> String {
    let (_, _, cookie) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": kvk, "persoon": "A. Tester"})),
    )
    .await;
    let (status, body, _) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/application"),
        cookie.as_deref(),
        Some(afnemer_concept(Some("VOORBEELD"))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // De aanvraag opent de zaak in stage AANVRAAG (RFC-008); de werkvoorraad
    // laat alleen een zaak met stage BESLUIT weg.
    assert_eq!(body["gram"]["stage"], "AANVRAAG");
    body["gram"]["id"].as_str().unwrap().to_string()
}

async fn behandelaar(app: &Router) -> String {
    behandelaar_in(app, AFNEMER).await
}

/// Log in als behandelaar van een proces.
async fn behandelaar_in(app: &Router, proces: &str) -> String {
    let (status, body, cookie) = vraag(
        app,
        "POST",
        &format!("{proces}/api/channels/medewerker/login"),
        None,
        Some(json!({"naam": "B. Behandelaar"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["fields"]["naam"], "B. Behandelaar");
    assert_eq!(body["role"], "behandelaar");
    cookie.unwrap()
}

/// Voeg een gram toe aan de kroniek van de afnemer, zoals stap voor stap
/// vastleggen dat later zal doen. Langs de kroniek van de runtime: die houdt
/// de grammen in het geheugen, dus een regel die iemand anders in het bestand
/// schrijft, ziet zij niet.
/// Leg een verloopgram vast op de klok van de test.
fn voeg_gram_toe(rt: &Runtime, name: &str, case: &str, fields: Value) {
    voeg_gram_toe_op(rt, name, case, fields, "2025-03-12T10:14:03+01:00");
}

/// Leg een verloopgram vast dat rechtens geldt op `op_moment`, vastgelegd op
/// de klok van de test.
fn voeg_gram_toe_op(rt: &Runtime, name: &str, case: &str, fields: Value, effective_at: &str) {
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
    schema::valideer(Soort::Gram, &gram).unwrap();
    rt.cells
        .iter()
        .find(|c| c.cell.id() == "test_afnemer")
        .unwrap()
        .chronicle
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
    let (status, _, _) = vraag(&app, "GET", &format!("{AFNEMER}/api/worklist"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // De aanvrager: zijn eigen grammen, geen werkvoorraad en geen zaak.
    let (_, _, aanvrager) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A"})),
    )
    .await;
    let aanvrager = aanvrager.as_deref();
    for (methode, path) in [
        ("GET", format!("{AFNEMER}/api/worklist")),
        ("GET", format!("{AFNEMER}/api/cases/{een}")),
        (
            "POST",
            format!("{AFNEMER}/api/cases/{een}/actions/besluit/trial"),
        ),
        ("GET", format!("{AFNEMER}/api/channels/medewerker/session")),
    ] {
        let (status, body, _) = vraag(&app, methode, &path, aanvrager, Some(json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{methode} {path}: {body}");
    }
    // De kroniek is van de cel, zonder login en zonder rollen: de cel kent
    // geen aanvrager en geen behandelaar.
    let (status, chronicle, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(chronicle.as_array().unwrap().len(), 2);

    // De behandelaar: het portaal niet.
    let b = behandelaar(&app).await;
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/application/assessment"),
        Some(&b),
        Some(afnemer_concept(Some("VOORBEELD"))),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/channels/eherkenning/session"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Een lege naam is geen medewerker; een proces zonder behandeling heeft
    // geen werkvoorraad, en een kanaal dat het niet noemt bestaat niet.
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/channels/medewerker/login"),
        None,
        Some(json!({"naam": " "})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    for path in ["/api/channels/bestaat_niet/login", "/api/worklist"] {
        let (status, _, _) = vraag(
            &app,
            "POST",
            &format!("{INSTANTIE}{path}"),
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
async fn werkvoorraad_is_een_lijst_van_zaken_zonder_besluit() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = als_lezer(&rt);
    let een = afnemer_indienen(&app, "12345678").await;
    let twee = afnemer_indienen(&app, "87654321").await;
    let b = behandelaar(&app).await;

    let (status, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    assert_eq!(w["name"], "werkvoorraad");
    assert_eq!(
        w["parameters"],
        json!({}),
        "een lijst heeft geen parameters"
    );
    let list = w["list"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    let regel = list.iter().find(|r| r["root"] == een.as_str()).unwrap();
    assert_eq!(
        regel["fields"],
        json!({"ontvangen_op": "2025-03-12", "aanvrager": "Vereniging Voorbeeld", "kvk": "12345678"})
    );

    // Een verloopgram laat de zaak staan; een besluit haalt haar eraf.
    voeg_gram_toe(&rt, "termijn_opgeschort", &een, json!({"dagen": 5}));
    let (_, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(w["list"].as_array().unwrap().len(), 2);
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
        &format!("{AFNEMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    let list = w["list"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["root"], twee.as_str());

    // De cellenlijst noemt de werkvoorraad een lijst, met kolommen; de
    // processenlijst zegt wie haar ziet.
    let (_, cells, _) = vraag(&app, "GET", "/api/cells", None, None).await;
    let worklist = cells[0]["lexostatuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "werkvoorraad")
        .unwrap();
    assert_eq!(worklist["list"], json!(true));
    assert_eq!(worklist["parameters"], json!([]));
    let (_, processen, _) = vraag(&app, "GET", "/api/processes", None, None).await;
    assert_eq!(
        processen[0]["roles"]["behandelaar"],
        json!({"channel": "medewerker", "routes": ["handling"], "label": "Behandelaar"})
    );
    assert_eq!(processen[0]["handling"]["worklist"], "werkvoorraad");
}

#[tokio::test]
async fn zaak_met_proefbesluit_zonder_vastleggen() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = als_lezer(&rt);
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let chronicle = data.path().join("test_afnemer/test_afnemer.jsonl");
    let voor = std::fs::read_to_string(&chronicle).unwrap();

    // Zonder oordelen: niet te nemen, en het proefbesluit zegt wat er mist.
    let (status, z, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{z}");
    assert_eq!(z["grams"].as_array().unwrap().len(), 1);
    // De handelingen van het proces, in de volgorde van proces.yaml; het
    // besluit eerst.
    let namen: Vec<&str> = z["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        namen,
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
            .starts_with("niet te nemen: mist "),
        "{p}"
    );
    let niet: Vec<&str> = p["not_delivered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert_eq!(niet, ["besluitdatum", "feiten_vergaard"]);

    // Met oordelen: te nemen, met herkomst per parameter.
    let (status, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit/trial"),
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
    // De bekendmaking komt in een latere stage van de procedure: bij het
    // besluit is ze nog niet gebeurd. De dag leest de lexostatus besluit
    // (geen gram: leeg); de rest volgt uit de procedure.
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
    // De peildatum is de besluitdatum: het op_moment dat het event aan het
    // formulier bindt.
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
    // Geen aanvulling gevraagd: de reductie leest het ontbreken als null.
    assert_eq!(p["parameters"]["datum_uitnodiging_aanvulling"], Value::Null);
    assert_eq!(p["parameters"]["opgeschorte_dagen"], json!(0));
    assert_eq!(p["not_delivered"], json!([]));

    // Een opschorting in de zaak schuift de uiterste datum op.
    voeg_gram_toe(&rt, "termijn_opgeschort", &case, json!({"dagen": 5}));
    let (_, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(json!({"form": {"besluitdatum": "2025-03-12", "feiten_vergaard": true}})),
    )
    .await;
    assert_eq!(p["outputs"]["besluitdeadline"], json!("2025-04-16"));

    // Een opschorting die pas na de peildatum ingaat, telt bij dit besluit
    // niet mee: de cel reduceert op de peildatum van het besluit.
    voeg_gram_toe_op(
        &rt,
        "termijn_opgeschort",
        &case,
        json!({"dagen": 30}),
        "2025-04-01T09:00:00+02:00",
    );
    let (_, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(json!({"form": {"besluitdatum": "2025-03-12", "feiten_vergaard": true}})),
    )
    .await;
    assert_eq!(p["outputs"]["besluitdeadline"], json!("2025-04-16"));
    assert_eq!(p["lexostatuses"][1]["as_of"], json!("2025-03-12"), "{p}");

    // Er is niets vastgelegd, behalve de verloopgrammen van deze test.
    let after = std::fs::read_to_string(&chronicle).unwrap();
    assert_eq!(after.lines().count(), voor.lines().count() + 2);

    // Een oordeel dat het formulier niet kent, en een onbekende zaak.
    let (status, f, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(json!({"form": {"bekendgemaakt": true}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(f["error"].as_str().unwrap().contains("'bekendgemaakt'"));
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/cases/00000000-0000-4000-8000-000000000009"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Een gram zonder verwijzing is zijn eigen wortel; het id geeft de cel.
/// Een event met een verwijzing laat het portaal alleen verwijzen naar een
/// gram waarvan de aanvrager de groep kent, en de cel toetst dat het gram
/// bestaat.
#[tokio::test]
async fn een_wortel_en_een_verwijzing() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let c = logins(&app, "12345678").await;
    let application = format!("{INSTANTIE}/api/application");
    // Een verwijzing die het event niet heeft, weigert de cel.
    let mut met = volledig();
    met["refers_to"] = json!({"vorige": "00000000-0000-4000-8000-000000000009"});
    let (status, body, _) = vraag(&app, "POST", &application, Some(&c), Some(met)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body, _) = vraag(&app, "POST", &application, Some(&c), Some(volledig())).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body["gram"].get("refers_to").is_none());
    let case = body["gram"]["id"].as_str().unwrap().to_string();
    drop(app);

    // Dezelfde kroniek, nu met een stroom waarin de aanvraag mag verwijzen
    // naar een eerdere aanvraag.
    let opstelling = eigen_opstelling(&[("instantie", &zo)], &[("instantie", &zo)]);
    let stream = opstelling.path().join("chronicles/test_aanvragen.yaml");
    let tekst = std::fs::read_to_string(&stream).unwrap();
    std::fs::write(
        &stream,
        tekst.replace(
            "  - name: aanvraag_ontvangen\n",
            "  - name: aanvraag_ontvangen\n    refers_to: {vorige: {to: aanvraag_ontvangen}}\n",
        ),
    )
    .unwrap();
    let app = runtime_op(opstelling.path(), data.path()).unwrap().router;
    let c = logins(&app, "12345678").await;
    let mut onbekend = volledig();
    onbekend["refers_to"] = json!({"vorige": "00000000-0000-4000-8000-000000000009"});
    let (status, body, _) = vraag(&app, "POST", &application, Some(&c), Some(onbekend)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["error"].as_str().unwrap().contains("geen wortel"),
        "{body}"
    );
    let mut volgt = volledig();
    volgt["refers_to"] = json!({"vorige": case});
    // Een andere KvK kent deze groep niet: dat weet het proces, niet de cel.
    let ander = logins(&app, "87654321").await;
    let (status, body, _) = vraag(
        &app,
        "POST",
        &application,
        Some(&ander),
        Some(volgt.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["error"].as_str().unwrap().contains("geen wortel"),
        "{body}"
    );
    let (status, body, _) = vraag(&app, "POST", &application, Some(&c), Some(volgt)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["refers_to"]["vorige"], json!(case));
    assert_ne!(body["gram"]["id"], json!(case));
    schema::valideer(Soort::Gram, &body["gram"]).unwrap();
}

#[test]
fn een_falende_cel_houdt_de_runtime_tegen() {
    let kapot = |t: String| t.replace("lexostatuses: lexostatuses.yaml", "lexostatuses: weg.yaml");
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
        cells_path: fixtures().join("cells"),
        processes_path: None,
        regulation_path: fixtures().join("regulation"),
        data_dir: data.path().to_path_buf(),
        port: STANDAARD_POORT,
        lees_token: None,
        lees_token_bronnen: Vec::new(),
        reduction: Default::default(),
        registers: None,
    };
    let r = Runtime::laad(&config, klok()).unwrap();
    assert_eq!(r.cells.len(), 5);
    assert!(r.processen.is_empty());
}

// --- Synthese per regel en het vastleggen van het besluit ---

/// Het besluitformulier van de afnemer, volledig ingevuld.
fn verdicts() -> Value {
    json!({"form": {"besluitdatum": "2025-03-12", "feiten_vergaard": true}})
}

#[tokio::test]
async fn synthese_per_regel_vult_de_tabel_aan() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let (status, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit/trial"),
        Some(&b),
        Some(verdicts()),
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

    // De peildatum is het jaartal uit de registercel (jaar_van), omgezet
    // naar 1 januari van dat jaar.
    assert_eq!(p["parameters"]["jaar"], json!(2024));
    assert_eq!(p["provenance"]["jaar"]["cell"], "test_register");
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
        &format!("{AFNEMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    let mut concept = afnemer_concept(Some("VOORBEELD"));
    concept["external"]["gebieden"] = json!([{"gebied": "Onbekendstad", "zetels": 1}]);
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/application"),
        cookie.as_deref(),
        Some(concept),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let case = body["gram"]["id"].as_str().unwrap().to_string();
    let b = behandelaar(&app).await;
    let (_, p, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit/trial"),
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

    // En dan legt de cel niets vast.
    let (status, f, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(f["error"].as_str().unwrap().starts_with("niet te nemen"));
    let chronicle =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(chronicle.lines().count(), 1, "alleen de aanvraag");
}

#[tokio::test]
async fn besluit_nemen_legt_een_decretogram_vast() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = afnemer_indienen(&app, "12345678").await;
    let twee = afnemer_indienen(&app, "87654321").await;
    let b = behandelaar(&app).await;

    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gram = &body["gram"];
    schema::valideer(Soort::Gram, gram).unwrap();
    assert_eq!(gram["type"], "decretogram");
    assert_eq!(gram["stage"], "BESLUIT");
    assert_eq!(gram["refers_to"], json!({"on_application": case}));
    assert_ne!(gram["id"], case.as_str());
    assert_eq!(gram["legal_character"], "BESCHIKKING");
    assert_eq!(gram["legal_basis"], json!(["testregeling_afnemer#3 lid 1"]));
    assert_eq!(gram["decision_type"], "TOEKENNING");
    assert_eq!(gram["regulation"], "testregeling_afnemer");
    assert_eq!(gram["regulation_valid_from"], "2025-01-01");
    // Drie assen: wie vastlegt (de actor van het proces), wie de wet bevoegd
    // maakt (letterlijk uit de regeling), en wie handelde, namens dat gezag.
    assert_eq!(gram["recording_actor"], "test_afnemer");
    assert_eq!(gram["competent_authority"], "Test afnemer");
    assert_eq!(
        gram["acting_actor"],
        json!({"role": "behandelaar", "channel": "medewerker",
               "identity": {"naam": "B. Behandelaar"}, "on_behalf_of": "Test afnemer"})
    );
    assert_eq!(body["warnings"], json!([]), "{body}");

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
        json!({"value": "2025-03-12", "provenance": {"source": "handler"}})
    );
    assert_eq!(inputs["zetels_op_lijst"]["provenance"]["source"], "cell");
    assert_eq!(inputs["gebiedstabel"]["provenance"]["source"], "per_row");
    assert_eq!(
        inputs["bekendgemaakt"]["provenance"]["source"],
        "state_at_decision"
    );
    // Het op_moment van het besluit is de besluitdatum, met grondslag.
    assert_eq!(gram["effective_at"], "2025-03-12T00:00:00+01:00");
    assert_eq!(
        gram["effective_at_legal_basis"],
        json!(["testregeling_afnemer#3 lid 1"])
    );
    assert_eq!(
        inputs["aanvraagdatum"]["provenance"],
        json!({"source": "own", "lexostatus": "aanvraag_inhoud"})
    );
    // Het receipt: de geladen regelingen en de stromen, met een hash erover.
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

    // De zaak verdwijnt uit de werkvoorraad, en het gram staat in de zaak.
    let (_, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    let list = w["list"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["root"], twee.as_str());
    let (_, z, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(z["grams"].as_array().unwrap().len(), 2);

    // Een tweede besluit in dezelfde zaak: dat is een wijziging.
    let (status, f, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("ligt al in de zaak; een ander besluit hierover vraagt een eigen grondslag"),
        "{f}"
    );
    let chronicle =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(
        chronicle.lines().count(),
        3,
        "twee aanvragen en een besluit"
    );

    // De aanvrager mag niet besluiten.
    let (_, _, aanvrager) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A"})),
    )
    .await;
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{twee}/actions/besluit"),
        aanvrager.as_deref(),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // De cel geeft een proces alleen de zaak die het vraagt, en kent een
    // onbekende zaak niet.
    let (status, g, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/cases/{case}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{g}");
    let g = g.as_array().unwrap();
    assert_eq!(g.len(), 2);
    assert_eq!(g[0]["gram"]["id"], case.as_str());
    assert_eq!(g[1]["gram"]["refers_to"]["on_application"], case.as_str());
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/cases/00000000-0000-4000-8000-000000000009"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Twee gelijktijdige besluiten op de andere zaak: de cel legt er één
    // vast, want de toets op de stage en het schrijven delen één slot.
    let path = format!("{AFNEMER}/api/cases/{twee}/actions/besluit");
    let (een, ander) = tokio::join!(
        vraag(&app, "POST", &path, Some(&b), Some(verdicts())),
        vraag(&app, "POST", &path, Some(&b), Some(verdicts())),
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
    let chronicle =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(
        chronicle.lines().count(),
        4,
        "twee aanvragen en twee besluiten"
    );
}

/// Een opstelling waarin artikel 3 (de beschikking) een ander gezag noemt
/// dan het gezag waarvoor het proces handelt ('Test afnemer'), met een
/// aanpassing aan het proces.
fn met_ander_gezag(
    proces: &dyn Fn(String) -> String,
) -> (tempfile::TempDir, tempfile::TempDir, Router) {
    let cells = eigen_opstelling(
        &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
        &[("afnemer", proces)],
    );
    for e in std::fs::read_dir(fixtures().join("regulation")).unwrap() {
        let van = e.unwrap().path();
        let to = cells
            .path()
            .join("regulation")
            .join(van.file_name().unwrap());
        std::fs::create_dir_all(&to).unwrap();
        let tekst = std::fs::read_to_string(van.join("2025-01-01.yaml")).unwrap();
        let tekst = if van.ends_with("testregeling_afnemer") {
            // Alleen artikel 3 is een BESCHIKKING; daar komt het gezag bij.
            let met_gezag = tekst.replace(
                "    machine_readable:\n      execution:\n        produces:\n          legal_character: BESCHIKKING",
                "    machine_readable:\n      competent_authority:\n        name: Een andere instantie\n      execution:\n        produces:\n          legal_character: BESCHIKKING",
            );
            assert_ne!(met_gezag, tekst, "het gezag is niet in de regeling gezet");
            met_gezag
        } else {
            tekst
        };
        std::fs::write(to.join("2025-01-01.yaml"), tekst).unwrap();
    }
    let data = tempfile::tempdir().unwrap();
    let config = Config {
        cells_path: cells.path().join("cells"),
        processes_path: Some(cells.path().join("processes")),
        regulation_path: cells.path().join("regulation"),
        data_dir: data.path().to_path_buf(),
        port: 0,
        lees_token: None,
        lees_token_bronnen: Vec::new(),
        reduction: Default::default(),
        registers: None,
    };
    let app = Runtime::laad(&config, klok()).unwrap().router;
    (cells, data, app)
}

#[tokio::test]
async fn een_ander_bevoegd_gezag_weigert_het_besluit() {
    // De wet wijst een ander gezag aan dan dat waarvoor het proces handelt,
    // en het proces heeft geen mandaat: geen gram. Namen worden letterlijk
    // vergeleken, niet genormaliseerd.
    let (_cellen, data, app) = met_ander_gezag(&zo);
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let (status, f, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit"),
        Some(&b),
        Some(verdicts()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("de wet wijst 'Een andere instantie' aan als bevoegd gezag, en het proces handelt namens 'Test afnemer' zonder mandaat"),
        "{f}"
    );
    let chronicle =
        std::fs::read_to_string(data.path().join("test_afnemer/test_afnemer.jsonl")).unwrap();
    assert_eq!(chronicle.lines().count(), 1, "alleen de aanvraag");
}

/// Met een mandaat van dat gezag (Awb 10:1) legt de cel het besluit wel vast:
/// `competent_authority` is het gezag van de wet, `recording_actor` de actor
/// van het proces, en de handelende actor noemt de behandelaar, namens wie
/// en op welk mandaat.
#[tokio::test]
async fn een_mandaat_laat_besluiten_namens_een_ander_gezag() {
    let met_mandaat = |t: String| {
        t.replace(
            "on_behalf_of: {regulation: testregeling_afnemer}\n",
            "on_behalf_of: {regulation: testregeling_afnemer}\nmandates:\n  - {authority: Een andere instantie, legal_basis: 'testregeling_afnemer#7'}\n",
        )
    };
    let (_cellen, _data, app) = met_ander_gezag(&met_mandaat);
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/besluit"),
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

// --- Voorbeelden per handeling ---

#[tokio::test]
async fn voorbeelden_zonder_login() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (status, body, _) =
        vraag(&app, "GET", &format!("{AFNEMER}/api/examples"), None, None).await;
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
        afnemer_concept(Some("VOORBEELD"))["external"]
    );
    assert_eq!(body["actions"]["besluit"], verdicts()["form"]);
    // "$vandaag" is bij het opvragen de datum van de klok.
    assert_eq!(
        body["actions"]["bekendmaken"]["datum_bekendmaking"],
        "2025-03-12"
    );

    // Een proces zonder voorbeelden: leeg, geen fout.
    let (status, body, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/examples"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"logins": [], "application": null, "actions": {}})
    );
}

#[tokio::test]
async fn het_aanvraagvoorbeeld_is_in_te_dienen() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (_, v, _) = vraag(&app, "GET", &format!("{AFNEMER}/api/examples"), None, None).await;
    let (status, _, cookie) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/channels/eherkenning/login"),
        None,
        Some(v["logins"][0]["fields"].clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/application"),
        cookie.as_deref(),
        Some(json!({"external": v["application"]})),
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
        fouten[0].contains("voorbeeld-login.json: verwacht een object met 'form'"),
        "{fouten:?}"
    );
}

// --- De cel legt vast en reduceert op proef, op verzoek van een proces ---

/// Een verzoek aan de instantie-cel voor het portaal-event.
fn verzoek(actor: &str) -> Value {
    json!({
        "actor": actor,
        "stream": "test_aanvragen",
        "event": "aanvraag_ontvangen",
        "intake": {"channel": "portaal", "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"}, "burger": {"nummer": null}},
        "external": volledig()["external"],
    })
}

#[tokio::test]
async fn de_cel_legt_een_gram_vast_voor_haar_actor() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let grams = format!("{INSTANTIE_CEL}/api/grams");
    let (status, body) = als_runtime(&rt, "POST", &grams, verzoek("test_instantie")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    schema::valideer(Soort::Gram, &body["gram"]).unwrap();
    // De cel bouwt het gram uit haar stroom: zij geeft het id en het
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

    // Een actor die niet de recording_actor van de stroom is: geen gram.
    let (status, body) = als_runtime(&rt, "POST", &grams, verzoek("test_afnemer")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        body["error"],
        "actor 'test_afnemer' legt niet vast in stroom 'test_aanvragen': de recording_actor is 'test_instantie'"
    );
    // Een event dat de cel niet heeft, en een veld dat de stroom niet kent.
    let mut onbekend = verzoek("test_instantie");
    onbekend["event"] = json!("bestaat_niet");
    let (status, _) = als_runtime(&rt, "POST", &grams, onbekend).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut field = verzoek("test_instantie");
    field["external"]["schoenmaat"] = json!(44);
    let (status, body) = als_runtime(&rt, "POST", &grams, field).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("schoenmaat"));
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[tokio::test]
async fn de_cel_reduceert_op_proef_zonder_vast_te_leggen() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let app = als_lezer(&rt);
    let trial = format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud/trial");
    let (status, body) = als_runtime(
        &rt,
        "POST",
        &trial,
        json!({"draft": verzoek("test_instantie"), "inputs": {}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Het gram van het concept, en de reductie met dat gram: het zaakkenmerk
    // van het concept is de input.
    let case = body["gram"]["id"].as_str().unwrap();
    assert_eq!(body["lexostatus"]["root"], case);
    assert_eq!(body["lexostatus"]["parameters"]["bevat_naam"], json!(true));
    assert_eq!(
        body["lexostatus"]["parameters"]["aanvraagdatum"],
        "2025-03-12"
    );
    // Niets vastgelegd.
    let (_, chronicle, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/chronicle"),
        None,
        None,
    )
    .await;
    assert_eq!(chronicle, json!([]));
    // Ook op proef legt alleen de actor van de stroom iets voor.
    let (status, _) = als_runtime(
        &rt,
        "POST",
        &trial,
        json!({"draft": verzoek("iemand_anders")}),
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
    let app = als_lezer(&rt);
    afnemer_indienen(&app, "12345678").await;
    let concept = json!({
        "actor": "test_afnemer",
        "stream": "test_afnemer_aanvragen",
        "event": "aanvraag_ontvangen",
        "intake": {"channel": "portaal", "eherkenning": {"kvk": "87654321", "persoon": "B. Tester"}, "burger": {"nummer": null}},
        "external": afnemer_concept(Some("VOORBEELD"))["external"],
    });
    let (status, body) = als_runtime(
        &rt,
        "POST",
        &format!("{AFNEMER_CEL}/api/lexostatus/werkvoorraad/trial"),
        json!({"draft": concept}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lexostatus"]["list"].as_array().unwrap().len(), 2);
    let (_, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/lexostatus/werkvoorraad"),
        None,
        None,
    )
    .await;
    assert_eq!(w["list"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn alleen_de_runtime_legt_vast_en_leest() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let grams = format!("{INSTANTIE_CEL}/api/grams");
    let trial = format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud/trial");
    // Zonder token: 401, met een ander token: 403, voor vastleggen en proef.
    for (path, body) in [
        (&grams, verzoek("test_instantie")),
        (&trial, json!({"draft": verzoek("test_instantie")})),
    ] {
        let (status, error, _) = vraag(&rt.router, "POST", path, None, Some(body.clone())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: {error}");
        assert!(error["error"]
            .as_str()
            .unwrap()
            .contains("runtime-token ontbreekt"));
        let vals = [(RUNTIME_TOKEN_HEADER, "0".repeat(64))];
        let vals: Vec<(&str, &str)> = vals.iter().map(|(n, w)| (*n, w.as_str())).collect();
        let (status, _, _) = vraag_met(&rt.router, "POST", path, &vals, Some(body)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    // Het token van een andere runtime telt ook niet.
    let ander = runtime_op(&fixtures(), tempfile::tempdir().unwrap().path()).unwrap();
    let vreemd = [(RUNTIME_TOKEN_HEADER, ander.runtime_token.als_str())];
    let (status, _, _) = vraag_met(
        &rt.router,
        "POST",
        &grams,
        &vreemd,
        Some(verzoek("test_instantie")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Er is niets vastgelegd. Lezen vraagt een token: de grammen dragen de
    // identiteit en de intake van wie indiende. Alleen de stroomdefinities
    // zijn open.
    for path in ["chronicle", "cases/z", "lexostatus/aanvraag_inhoud?root=z"] {
        let uri = format!("{INSTANTIE_CEL}/api/{path}");
        let (status, error, _) = vraag(&rt.router, "GET", &uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: {error}");
        let (status, _, _) = vraag_met(&rt.router, "GET", &uri, &vreemd, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        // Zonder leestoken in de runtime telt een leestoken niet.
        let lees = [(LEES_TOKEN_HEADER, "gedeeld-leestoken-van-de-test")];
        let (status, _, _) = vraag_met(&rt.router, "GET", &uri, &lees, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    let (status, _, _) = vraag(
        &rt.router,
        "GET",
        &format!("{INSTANTIE_CEL}/api/stream"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!dir
        .path()
        .join("test_instantie/test_kroniek.jsonl")
        .exists());
    // Het proces zelf legt wel vast: het interne transport draagt het token.
    let c = logins(&rt.router, "12345678").await;
    let (status, body, _) = vraag(
        &rt.router,
        "POST",
        &format!("{INSTANTIE}/api/application"),
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
            "  form:",
            "  offer: {regulation: testregeling_aanvraag, output: aanvraag_volledig}\n  form:",
        )
    };
    let opstelling = eigen_opstelling(&[("instantie", &zo)], &[("instantie", &met_aanbod)]);
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert!(
        fouten.contains(&"proces 'test_instantie_proces': offer: voorwaarde leunt op 'aanvraagdatum' (BELANGHEBBENDE, grondslag testregeling_aanvraag#1 lid 1), dat vooraf niet bekend is".to_string()),
        "{fouten:?}"
    );
}

// --- Het tijdvak van het aanbod ---

/// Het aanbod draait per tijdvak dat het beleid aanbiedt: de uitkomst
/// `aanbod.tijdvakken` van de regeling, uit een run op de datum van vandaag.
/// Het tijdvak is de parameter met origin BELANGHEBBENDE en `rol: TIJDVAK`,
/// niet een vaste naam.
#[tokio::test]
async fn het_aanbod_draait_per_gekozen_tijdvak() {
    let met_aanbod = |t: String| {
        t.replace(
            "    output: aanvraag_toelaatbaar\n",
            "    output: aanvraag_toelaatbaar\n  offer:\n    regulation: testregeling_afnemer\n    output: aanvraag_aangeboden\n    deadline: aanvraagtermijn\n    windows: aangeboden_jaren\n    start: begin_aanvraagjaar\n",
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
        &format!("{AFNEMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    let (status, body, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/possibilities"),
        cookie.as_deref(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let m = body["possibilities"].as_array().unwrap();
    assert_eq!(m.len(), 2, "{body}");
    for (i, jaar) in [(0, 2025), (1, 2026)] {
        assert_eq!(
            m[i]["possibility"]["window"],
            json!({"parameter": "aanvraagjaar", "value": jaar}),
            "{body}"
        );
        assert_eq!(m[i]["parameters"]["aanvraagjaar"], json!(jaar));
        assert_eq!(
            m[i]["provenance"]["aanvraagjaar"],
            json!({"source": "choice"})
        );
        assert_eq!(
            m[i]["possibility"]["deadline"],
            json!(format!("{jaar}-04-01"))
        );
    }
    // Het lopende jaar peilt op vandaag, een komend jaar op het begin dat
    // de regeling zegt (art. 6), niet op een jaar dat de code aanneemt.
    assert_eq!(m[0]["as_of"], json!("2025-03-12"), "{body}");
    assert_eq!(m[1]["as_of"], json!("2026-01-01"), "{body}");
}

/// Een aanbod dat een tijdvak vraagt, zonder tijdvakken: de runtime start niet.
#[test]
fn een_tijdvak_zonder_tijdvakken_houdt_de_runtime_tegen() {
    let met_aanbod = |t: String| {
        t.replace(
            "    output: aanvraag_toelaatbaar\n",
            "    output: aanvraag_toelaatbaar\n  offer: {regulation: testregeling_afnemer, output: aanvraag_aangeboden}\n",
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
        ["proces 'test_afnemer_proces': offer: het tijdvak 'aanvraagjaar' (rol TIJDVAK) vraagt aanbod.tijdvakken: de uitkomst van het beleid met de tijdvakken die het portaal aanbiedt"]
    );
}

/// De tijdvakken komen uit de regeling van het aanbod, in een run zonder
/// parameters: een uitkomst die niet bestaat, of uit een artikel dat een
/// parameter vraagt, houdt de runtime tegen.
#[test]
fn de_tijdvakken_komen_uit_het_beleid() {
    for (windows, verwacht) in [
        (
            "bestaat_niet",
            "portaal.aanbod: regeling 'testregeling_afnemer' heeft geen tijdvakken-uitkomst 'bestaat_niet'",
        ),
        (
            "gebiedsbedrag",
            "portaal.aanbod: tijdvakken 'gebiedsbedrag' komt uit testregeling_afnemer#3, en dat artikel vraagt een parameter",
        ),
    ] {
        let met_aanbod = move |t: String| {
            t.replace(
                "    output: aanvraag_toelaatbaar\n",
                &format!("    output: aanvraag_toelaatbaar\n  offer: {{regulation: testregeling_afnemer, output: aanvraag_aangeboden, windows: {windows}}}\n"),
            )
        };
        let opstelling = eigen_opstelling(
            &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
            &[("afnemer", &met_aanbod)],
        );
        let data = tempfile::tempdir().unwrap();
        let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
        assert!(
            fouten.iter().any(|f| f.contains(verwacht)),
            "verwacht '{verwacht}' in {fouten:?}"
        );
    }
}

/// Een vastleg-event met een stage die de procedure van de beschikking niet
/// kent: de stand bij besluit is dan niet af te leiden, en de runtime start
/// niet.
#[test]
fn een_stage_buiten_de_procedure_houdt_de_runtime_tegen() {
    let opstelling = eigen_opstelling(
        &[("afnemer", &zo), ("register", &zo), ("gebieden", &zo)],
        &[("afnemer", &zo)],
    );
    let stream = opstelling
        .path()
        .join("chronicles/test_afnemer_zaakverloop.yaml");
    let tekst = std::fs::read_to_string(&stream).unwrap();
    std::fs::write(
        &stream,
        tekst.replace("stage: BESLUIT", "stage: BESLISSING"),
    )
    .unwrap();
    let lexo = opstelling.path().join("cells/afnemer/lexostatuses.yaml");
    let tekst = std::fs::read_to_string(&lexo).unwrap();
    std::fs::write(&lexo, tekst.replace("stage: BESLUIT", "stage: BESLISSING")).unwrap();
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
    assert!(
        fouten.iter().any(|f| f.contains(
            "handeling 'besluit', vastleggen test_afnemer_zaakverloop/besluit_genomen: stage 'BESLISSING' staat niet in procedure 'beschikking' van testregeling_afnemer#3 (AANVRAAG, BESLUIT, BEKENDMAKING, BEZWAAR)"
        )),
        "{fouten:?}"
    );
}

// --- De toets bouwt een tabel per regel op, zoals het besluit ---

/// De rijen van de toets: de gebiedstabel uit de proefreductie van het
/// concept, met een kolom die per regel uit de registercel komt.
fn met_toets_rijen(t: String) -> String {
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
        body["provenance"]["gebiedstabel"],
        json!({"source": "per_row", "lexostatus": "aanvraag_inhoud", "field": "gebieden"})
    );
    assert_eq!(body["rows"][0]["sources"][0]["queried"], json!(2));
    assert!(body["rows"][0].get("missing").is_none(), "{body}");
    assert_eq!(body["result"]["value"], json!(true), "{body}");
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
    assert_eq!(body["rows"][0]["missing"], json!(["ingeschreven"]));
    assert_eq!(body["rows"][0]["sources"][0]["status"], "not_queried");
}

#[tokio::test]
async fn de_toets_zonder_rijen_blijft_gelijk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let body = afnemer_toets(&app, Some("VOORBEELD")).await;
    assert_eq!(body["rows"], json!([]));
    assert!(body["parameters"].get("gebiedstabel").is_none(), "{body}");
    assert_eq!(body["result"]["value"], json!(true), "{body}");
}

#[test]
fn toets_rijen_controle_bij_het_opstarten() {
    let uit_ander = |t: String| {
        met_toets_rijen(t).replacen(
            "table: {lexostatus: aanvraag_inhoud, field: gebieden}",
            "table: {lexostatus: zaakverloop, field: gebieden}",
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
        fouten.contains(&"proces 'test_afnemer_proces': assessment, rijen 'gebiedstabel': de tabel komt uit lexostatus 'zaakverloop', en die is niet de toets-lexostatus".to_string()),
        "{fouten:?}"
    );
}

/// Een grondslag in het formulier die geen artikel van een geladen regeling
/// aanwijst, of een lid dat het artikel niet heeft: de runtime start niet.
#[test]
fn een_grondslag_in_het_formulier_wordt_gecontroleerd() {
    for (legal_basis, verwacht) in [
        (
            "testregeling_aanvraag#9",
            "formulier, veld 'aanvraagjaar': grondslag 'testregeling_aanvraag#9': regeling 'testregeling_aanvraag' heeft geen artikel 9",
        ),
        (
            "testregeling_aanvraag#1 lid 8",
            "formulier, veld 'aanvraagjaar': grondslag 'testregeling_aanvraag#1 lid 8': artikel 1 heeft geen lid 8",
        ),
    ] {
        let opstelling = eigen_opstelling(&[("instantie", &zo)], &[("instantie", &zo)]);
        let path = opstelling.path().join("processes/instantie/formulier.yaml");
        let tekst = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            tekst.replace("grondslag: testregeling_aanvraag#1 lid 1}", &format!("grondslag: '{legal_basis}'}}")),
        )
        .unwrap();
        let data = tempfile::tempdir().unwrap();
        let fouten = runtime_op(opstelling.path(), data.path()).err().unwrap();
        assert!(
            fouten.iter().any(|f| f.contains(verwacht)),
            "verwacht '{verwacht}' in {fouten:?}"
        );
    }
}

// --- Tijd: twee tijden per gram, en een peil op de reductie ---

/// Een aanvraag die langs een andere weg binnenkwam: het loket geeft de dag
/// van ontvangst op (`$intake.ontvangen_op`). Rechtens telt die dag (de
/// aanvraagdatum), vastgelegd wordt op de klok van de cel.
#[tokio::test]
async fn een_eerdere_ontvangst_is_de_aanvraagdatum() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let app = als_lezer(&rt);
    let mut v = verzoek("test_instantie");
    v["intake"] = json!({"channel": "counter", "received_at": "2025-03-05",
                         "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"},
                         "burger": {"nummer": null}});
    let (status, body) = als_runtime(&rt, "POST", &format!("{INSTANTIE_CEL}/api/grams"), v).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let g = &body["gram"];
    schema::valideer(Soort::Gram, g).unwrap();
    assert_eq!(g["effective_at"], "2025-03-05T00:00:00+01:00");
    assert_eq!(g["recorded_at"], "2025-03-12T10:14:03+01:00");
    assert_eq!(
        g["effective_at_legal_basis"],
        json!(["testregeling_aanvraag#1"])
    );
    let case = g["id"].as_str().unwrap();
    let (status, l, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(l["parameters"]["aanvraagdatum"], "2025-03-05");
    assert_eq!(l["recorded_at"], "2025-03-12T10:14:03+01:00");

    // Een ontvangst na het vastleggen is geen feit.
    let mut v = verzoek("test_instantie");
    v["intake"]["received_at"] = json!("2025-03-20");
    let (status, body) = als_runtime(&rt, "POST", &format!("{INSTANTIE_CEL}/api/grams"), v).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("na het vastleggen"));
}

/// `GET lexostatus` met een peil: dezelfde kroniek geeft op een ander moment
/// een andere lexostatus. De startstand geldt rechtens op haar eigen
/// momenten, maar is pas bekend sinds ze geladen is.
#[tokio::test]
async fn lexostatus_op_een_peilmoment() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    // De zetels uit `registerstatus`, de inschrijving uit het register van
    // de raad; samen in een antwoord.
    let status_op = |query: &'static str| {
        let app = app.clone();
        async move {
            let (s, mut l, _) = vraag(
                &app,
                "GET",
                &format!("/cells/test_register/api/lexostatus/registerstatus?aanduiding=VOORBEELD{query}"),
                None,
                None,
            )
            .await;
            if s == StatusCode::OK {
                let (_, r, _) = vraag(
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
    let (s, nu) = status_op("").await;
    assert_eq!(s, StatusCode::OK, "{nu}");
    assert_eq!(nu["parameters"]["zetels_toegewezen"], json!(6));
    assert!(nu.get("as_of").is_none());

    // Rechtens op 1 januari 2024: nog niet ingeschreven, geen uitslag.
    let (s, jan) = status_op("&as_of=2024-01-01").await;
    assert_eq!(s, StatusCode::OK, "{jan}");
    assert_eq!(jan["as_of"], "2024-01-01");
    assert_eq!(jan["parameters"]["is_ingeschreven_raad"], json!(false));
    assert_eq!(jan["parameters"]["zetels_toegewezen"], json!(0));
    // Een moment met tijdzone kan ook (in een query als %2B voor '+').
    let (s, apr) = status_op("&as_of=2024-04-01T00:00:00%2B02:00").await;
    assert_eq!(s, StatusCode::OK, "{apr}");
    assert_eq!(apr["parameters"]["zetels_toegewezen"], json!(6));
    assert_eq!(apr["parameters"]["is_ingeschreven_raad"], json!(true));

    // Zoals bekend voor het laden van de startstand (de klok van de test):
    // de cel wist toen nog niets.
    let (s, eerder) = status_op("&known_at=2025-03-11").await;
    assert_eq!(s, StatusCode::OK, "{eerder}");
    assert_eq!(eerder["parameters"]["is_ingeschreven_raad"], json!(false));
    let (_, toen) = status_op("&known_at=2025-03-12").await;
    assert_eq!(toen["parameters"], nu["parameters"]);

    let (s, f) = status_op("&as_of=morgen").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(f["error"].as_str().unwrap().contains("ongeldig as_of"));
}

/// Ook de proefroute peilt: een concept telt als vastgelegd op de klok van
/// nu, dus zoals bekend op een eerdere dag ligt het er niet.
#[tokio::test]
async fn de_proefroute_peilt_ook() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), dir.path()).unwrap();
    let trial = format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud/trial");
    let (status, body) = als_runtime(
        &rt,
        "POST",
        &trial,
        json!({"draft": verzoek("test_instantie"), "inputs": {"as_of": "2025-03-12"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lexostatus"]["as_of"], "2025-03-12");
    let (status, _) = als_runtime(
        &rt,
        "POST",
        &trial,
        json!({"draft": verzoek("test_instantie"), "inputs": {"known_at": "2025-03-11"}}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Een kroniek van voor chronolex v0.2.0 (zonder id, met zaakkenmerk) wordt
/// niet omgezet: de runtime start niet, en zegt waarom.
#[tokio::test]
async fn een_oude_kroniek_start_niet() {
    let data = tempfile::tempdir().unwrap();
    let case = {
        let app = app(data.path());
        afnemer_indienen(&app, "12345678").await
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
    let f = runtime_op(&fixtures(), data.path())
        .err()
        .unwrap()
        .join("\n");
    assert!(f.contains("van voor chronolex v0.2.0"), "{f}");
    assert!(f.contains("lege DATA_DIR"), "{f}");
}

// --- Kanalen en rollen als configuratie ---

/// Een tweede kanaal van hetzelfde portaal: een burger logt in met een
/// nummer van negen cijfers dat de elfproef doorstaat. Zijn nummer komt onder
/// het intake-pad van zijn kanaal in het gram; wat het andere kanaal levert,
/// blijft leeg.
#[tokio::test]
async fn een_tweede_kanaal_met_de_elfproef() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let login = format!("{INSTANTIE}/api/channels/burger/login");
    for error in ["123456789", "12345678", "1234567890"] {
        let (status, body, _) =
            vraag(&app, "POST", &login, None, Some(json!({"nummer": error}))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}: {body}");
        assert_eq!(body["error"], "dit is geen geldig burgernummer");
    }
    let (status, session, cookie) = vraag(
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
    // De sessie is van dit kanaal, niet van het andere.
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/channels/eherkenning/session"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, s, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE}/api/session"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["channel"], "burger");
    // Het portaal is van beide kanalen.
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application"),
        Some(&cookie),
        Some(volledig()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let o = &body["gram"]["fields"]["core"]["signed_via"];
    assert_eq!(
        o,
        &json!({"kanaal": "portaal", "kvk_nummer": null, "gemachtigde": null, "burgernummer": "123456782"})
    );
    // Een onbekend kanaal bestaat niet.
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/channels/digid/login"),
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Een rol noemt haar kanaal en haar routes. Langs het medewerkerskanaal
/// van de instantie logt alleen de rol loket in; die mag het loket en niet
/// het portaal, en de aanvrager het portaal en niet het loket.
#[tokio::test]
async fn een_rol_mag_alleen_haar_routes() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let medewerker = format!("{INSTANTIE}/api/channels/medewerker/login");
    let (status, body, _) = vraag(
        &app,
        "POST",
        &medewerker,
        None,
        Some(json!({"naam": "L. Loket", "role": "aanvrager"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, session, counter) = vraag(
        &app,
        "POST",
        &medewerker,
        None,
        Some(json!({"naam": "L. Loket"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(session["role"], "loket");
    let counter = counter.unwrap();
    let (status, body, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application"),
        Some(&counter),
        Some(volledig()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("alleen voor de rol aanvrager of burger"),
        "{body}"
    );
    let aanvrager = logins(&app, "12345678").await;
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/counter/application"),
        Some(&aanvrager),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/counter/application"),
        Some(&aanvrager),
        Some(loketinvoer("2025-03-05")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Het afnemerproces heeft geen loket.
    let (status, _, _) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/counter/application"),
        None,
        Some(loketinvoer("2025-03-05")),
    )
    .await;
    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED);
}

fn loketinvoer(received_at: &str) -> Value {
    json!({
        "applicant": {"channel": "eherkenning", "kvk": "12345678", "persoon": "A. Tester"},
        "received_at": received_at,
        "external": volledig()["external"],
    })
}

async fn counter(app: &Router) -> String {
    let (status, body, cookie) = vraag(
        app,
        "POST",
        &format!("{INSTANTIE}/api/channels/medewerker/login"),
        None,
        Some(json!({"naam": "L. Loket"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    cookie.unwrap()
}

/// Het loket voert een aanvraag in die langs een andere weg binnenkwam,
/// namens de aanvrager, met de dag van ontvangst (Awb 4:1, 4:13). Die dag is
/// het `op_moment`, het invoeren `vastgelegd_op`. Een dag na vandaag weigert
/// het loket; de aanvrager duidt het loket aan met de velden van een
/// portaalkanaal.
#[tokio::test]
async fn het_loket_voert_een_eerdere_ontvangst_in() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let l = counter(&app).await;
    let path = format!("{INSTANTIE}/api/counter/application");
    let (status, body, _) = vraag(
        &app,
        "POST",
        &path,
        Some(&l),
        Some(loketinvoer("2025-03-05")),
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
    // De aanvrager volgt zijn papieren aanvraag: het nummer is van hem.
    let case = g["id"].as_str().unwrap();
    let (status, l2, _) = vraag(
        &app,
        "GET",
        &format!("{INSTANTIE_CEL}/api/lexostatus/aanvraag_inhoud?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(l2["parameters"]["aanvraagdatum"], "2025-03-05");

    for (input, error) in [
        (loketinvoer("2025-03-13"), "ligt na vandaag"),
        (loketinvoer("vorige week"), "ongeldig ontvangen_op"),
        (
            json!({"applicant": {"kvk": "12345678", "persoon": "A"}, "received_at": "2025-03-05"}),
            "aanvrager: noem het kanaal",
        ),
        (
            json!({"applicant": {"channel": "eherkenning", "kvk": "1", "persoon": "A"}, "received_at": "2025-03-05"}),
            "aanvrager: een organisatienummer heeft acht cijfers",
        ),
    ] {
        let (status, body, _) = vraag(&app, "POST", &path, Some(&l), Some(input)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"].as_str().unwrap().contains(error),
            "{error}: {body}"
        );
    }
}

/// Noemt het beleid een openstelling van het tijdvak
/// (`aanbod.openstelling`), dan voert het loket geen ontvangst in van vóór
/// die dag. Het tijdvak komt uit het veld van de aanvraag.
#[tokio::test]
async fn het_loket_weigert_een_ontvangst_voor_de_openstelling() {
    let met_aanbod = |t: String| {
        t.replace(
            "  form:",
            "  offer:\n    regulation: testregeling_aanvraag\n    output: aanvraag_aangeboden\n    windows: aangeboden_jaren\n    opening: openstelling_aanvraagjaar\n  form:",
        )
    };
    let opstelling = eigen_opstelling(&[("instantie", &zo)], &[("instantie", &met_aanbod)]);
    let data = tempfile::tempdir().unwrap();
    let app = runtime_op(opstelling.path(), data.path()).unwrap().router;
    let l = counter(&app).await;
    let path = format!("{INSTANTIE}/api/counter/application");
    // Aanvraagjaar 2025 is open vanaf 1 januari 2025.
    let (status, body, _) = vraag(
        &app,
        "POST",
        &path,
        Some(&l),
        Some(loketinvoer("2024-12-20")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("vóór de openstelling van het tijdvak (2025-01-01)"),
        "{body}"
    );
    let (status, body, _) = vraag(
        &app,
        "POST",
        &path,
        Some(&l),
        Some(loketinvoer("2025-01-02")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // Zonder tijdvak is de openstelling niet te toetsen.
    let mut without = loketinvoer("2025-01-02");
    without["external"]["aanvraagjaar"] = Value::Null;
    let (status, body, _) = vraag(&app, "POST", &path, Some(&l), Some(without)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("het tijdvak (aanvraagjaar) ontbreekt"),
        "{body}"
    );
}

// --- Handelingen na het besluit: bekendmaken, betalen, en het zaakverloop ---

async fn action(
    app: &Router,
    b: &str,
    case: &str,
    name: &str,
    trial: bool,
    form: Value,
) -> (StatusCode, Value) {
    handeling_in(app, AFNEMER, b, case, name, trial, json!({ "form": form })).await
}

/// Een handeling in een zaak van een proces, op proef of genomen, met de
/// body zoals de route hem leest (`formulier`, en zo nodig `gebeurd`).
async fn handeling_in(
    app: &Router,
    proces: &str,
    b: &str,
    case: &str,
    name: &str,
    trial: bool,
    body: Value,
) -> (StatusCode, Value) {
    let path = if trial {
        format!("{proces}/api/cases/{case}/actions/{name}/trial")
    } else {
        format!("{proces}/api/cases/{case}/actions/{name}")
    };
    let (status, body, _) = vraag(app, "POST", &path, Some(b), Some(body)).await;
    (status, body)
}

/// Na het besluit: de bekendmaking is een vervolg op het besluit (de
/// engine voert stage BEKENDMAKING uit op de invoer van het vastgelegde
/// besluit, en de haak van die stage rekent de bezwaartermijn uit); de
/// betaling is een executogram dat alleen vastligt als de toets van zijn
/// grondslag waar is. Een tweede betaling boven het bedrag weigert de wet.
#[tokio::test]
async fn besluit_bekendmaken_en_betalen() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let betaling = |bedrag: i64| json!({"bedrag": bedrag, "datum_betaling": "2025-03-12"});

    // Voor het besluit: bekendmaken en betalen wachten op het besluit dat ze
    // volgen.
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
            .contains("wacht op het besluit"),
        "{f}"
    );
    let (status, f) = action(&app, &b, &case, "betalen", false, betaling(6000)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("wacht op het besluit"),
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

    // Na het besluit, voor de bekendmaking: het besluit is niet in werking.
    let (status, f) = action(&app, &b, &case, "betalen", false, betaling(6000)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"].as_str().unwrap().contains("betaling_conform"),
        "{f}"
    );

    // Het formulier van de bekendmaking is wat de stage vraagt.
    let (_, z, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    let bekend = &z["actions"][1];
    assert_eq!(bekend["available"], json!(true), "{bekend}");
    let fields: Vec<&str> = bekend["form"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(fields, ["datum_bekendmaking", "bekendgemaakt"]);
    assert_eq!(z["actions"][0]["available"], json!(false));

    // Een bekendmaking die niet op de voorgeschreven wijze is gedaan, geeft
    // geen bezwaartermijn: niet te nemen.
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
    // Een conclusie over de inhoud: gebeurde het toch, dan is het te melden.
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
    // Art. 3 in de stage BEKENDMAKING: nu is het besluit bekendgemaakt.
    assert_eq!(gram["fields"]["besluit_tijdig"], json!(true));
    assert_eq!(gram["effective_at"], "2025-03-12T00:00:00+01:00");

    // Betalen, in twee delen; de reductie telt de betalingen op.
    let (status, body) = action(&app, &b, &case, "betalen", false, betaling(4000)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["type"], "executogram");
    assert_eq!(body["trial"]["outputs"]["nog_te_betalen"], json!(2000));
    let (status, f) = action(&app, &b, &case, "betalen", false, betaling(2001)).await;
    assert_eq!(status, StatusCode::CONFLICT, "boven het bedrag: {f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("meld het dan als gebeurd"),
        "{f}"
    );
    let (status, body) = action(&app, &b, &case, "betalen", false, betaling(2000)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["trial"]["outputs"]["nog_te_betalen"], json!(0));
    let (status, _) = action(&app, &b, &case, "betalen", false, betaling(1)).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // De zaak: nog te betalen 0, en de bezwaartermijn uit de procedure.
    let (_, z, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    let betalen = z["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "betalen")
        .unwrap();
    assert_eq!(
        betalen["trial"]["outputs"]["nog_te_betalen"],
        json!(0),
        "{betalen}"
    );
    assert_eq!(betalen["recorded"], json!(2));
    assert_eq!(betalen["decision"], z["decisions"][0]["id"]);
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
    // De lexostatus van het besluit bevat de route.
    let (_, l, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/lexostatus/besluit?root={case}"),
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

/// Twee gelijktijdige betalingen die samen boven het vastgestelde bedrag
/// komen: elk rekent uit wat er nog te betalen is op de zaak zoals het proces
/// haar las, en de cel legt alleen vast als de zaak sindsdien niet veranderde
/// (`zaak_grammen`, onder het schrijfslot). Er komt er een door.
#[tokio::test]
async fn twee_gelijktijdige_betalingen() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
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
    let betaling = json!({"bedrag": 4000, "datum_betaling": "2025-03-12"});
    let (een, ander) = tokio::join!(
        action(&app, &b, &case, "betalen", false, betaling.clone()),
        action(&app, &b, &case, "betalen", false, betaling.clone()),
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
    // Welke weigering de tweede krijgt, hangt af van de volgorde. Lazen
    // beide proeven de zaak voor de eerste vastlag, dan weigert de cel op de
    // optimistische toets. Las de tweede haar erna, dan zegt de wet nee (er
    // is al betaald) en neemt het proces de handeling niet. Twee keer
    // vastleggen gebeurt in geen van beide gevallen.
    let geweigerd = if een.0 == StatusCode::CONFLICT {
        &een.1
    } else {
        &ander.1
    };
    let error = geweigerd["error"].as_str().unwrap();
    assert!(
        error.contains("veranderde sinds het proces haar las") || error.contains("zegt nee"),
        "{geweigerd}"
    );
    let (_, l, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER_CEL}/api/lexostatus/besluit?root={case}"),
        None,
        None,
    )
    .await;
    assert_eq!(l["parameters"]["betaald_bedrag"], json!(4000), "{l}");
}

/// Een tweede handeling in het zaakverloop: een verzoek om aanvulling telt
/// op proef mee, en na het vastleggen leest het besluit het.
#[tokio::test]
async fn een_aanvulling_vragen_werkt_door_in_het_besluit() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let (_, p) = action(&app, &b, &case, "aanvulling_vragen", true, json!({})).await;
    // Zonder de datum telt het feit niet: niet te nemen.
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

/// Een handeling nemen met `gebeurd`: de behandelaar meldt een feit dat
/// gebeurde terwijl de proef om de inhoud nee zei.
async fn melden(app: &Router, b: &str, case: &str, name: &str, form: Value) -> (StatusCode, Value) {
    let (status, body, _) = vraag(
        app,
        "POST",
        &format!("{AFNEMER}/api/cases/{case}/actions/{name}"),
        Some(b),
        Some(json!({"form": form, "happened": true})),
    )
    .await;
    (status, body)
}

/// Het proces concludeert voor het handelt; wat gebeurde, legt de cel toch
/// vast. Een betaling boven het bedrag doet het proces niet uit zichzelf,
/// maar gemeld als gebeurd ligt zij vast, en dan is het meerdere
/// onverschuldigd betaald. Een bekendmaking die niet aan de wet voldoet,
/// ligt gemeld vast zonder bezwaartermijn. Een besluit wordt niet gemeld, en
/// een onvolledig formulier niet vastgelegd.
#[tokio::test]
async fn een_gemeld_feit_legt_de_cel_vast() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let b = behandelaar(&app).await;
    let betaling = |bedrag: i64| json!({"bedrag": bedrag, "datum_betaling": "2025-03-12"});

    // Een besluit meldt men niet.
    let case = afnemer_indienen(&app, "12345678").await;
    let (status, f) = melden(&app, &b, &case, "besluit", verdicts()["form"].clone()).await;
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

    // Een bekendmaking die niet op de voorgeschreven wijze is gedaan: gemeld
    // ligt zij vast, zonder bezwaartermijn.
    let (status, body) = melden(
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
            .contains("gemeld als gebeurd"),
        "{body}"
    );
    let (_, l, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    let r = &l["decisions"][0]["legal_protection"];
    assert_eq!(r["stage"], "BEZWAAR", "{l}");
    assert_eq!(r["outputs"]["einde_bezwaartermijn"], Value::Null, "{l}");

    // Betalen: het bedrag, en dan een cent te veel.
    let (status, body) = action(&app, &b, &case, "betalen", false, betaling(6000)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, p) = action(&app, &b, &case, "betalen", true, betaling(1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["takeable"], json!(false), "{p}");
    assert_eq!(p["reportable"], json!(true), "{p}");
    assert_eq!(p["assessments"]["betaling_conform"], json!(false), "{p}");
    let (status, body) = melden(&app, &b, &case, "betalen", betaling(1)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["gram"]["type"], "executogram");
    assert_eq!(body["gram"]["fields"]["bedrag"], json!(1));
    // De gevolgen: de cel telt de betalingen op, de wet zegt wat
    // onverschuldigd is betaald.
    let (_, p) = action(&app, &b, &case, "betalen", true, betaling(0)).await;
    assert_eq!(p["parameters"]["betaald_bedrag"], json!(6001), "{p}");
    assert_eq!(p["outputs"]["onverschuldigd_betaald"], json!(1), "{p}");

    // Een feit zonder ingevuld formulier legt niemand vast, ook gemeld niet.
    let (status, f) = melden(&app, &b, &case, "aanvulling_vragen", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(f["error"].as_str().unwrap().contains("vul in"), "{f}");
}

/// Het moment van een handeling: niet in de toekomst en niet voor de zaak.
/// Het proces zegt het op proef; de cel weigert zo'n gram ook zelf.
#[tokio::test]
async fn een_moment_ligt_niet_voor_de_zaak_of_in_de_toekomst() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = als_lezer(&rt);
    let b = behandelaar(&app).await;
    let case = afnemer_indienen(&app, "12345678").await;
    let decision = |date: &str| json!({"besluitdatum": date, "feiten_vergaard": true});

    let (_, p) = action(&app, &b, &case, "besluit", true, decision("2025-03-11")).await;
    assert_eq!(p["takeable"], json!(false), "{p}");
    assert_eq!(p["reportable"], json!(false), "{p}");
    assert!(
        p["reason"].as_str().unwrap().contains("ligt voor de zaak"),
        "{p}"
    );
    let (status, f) = action(&app, &b, &case, "besluit", false, decision("2025-03-11")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    let (_, p) = action(&app, &b, &case, "besluit", true, decision("2025-03-13")).await;
    assert!(
        p["reason"].as_str().unwrap().contains("ligt na vandaag"),
        "{p}"
    );
    // Dezelfde dag als de aanvraag mag: het gaat om de dag.
    let (_, p) = action(&app, &b, &case, "besluit", true, decision("2025-03-12")).await;
    assert_eq!(p["takeable"], json!(true), "{p}");

    // De cel zelf: een besluit dat naar de aanvraag verwijst, met een
    // eerdere dag dan de aanvraag.
    let (status, f) = als_runtime(
        &rt,
        "POST",
        &format!("{AFNEMER_CEL}/api/grams"),
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
            .contains("ligt voor het gram waarnaar het verwijst"),
        "{f}"
    );
}

/// De stand van een zaak is een lexostatus van de cel, die de runtime
/// aanbiedt: de stages met wat hun gram vastlegde, het aantal per event, en
/// op vraag of iemand de zaak kent.
#[tokio::test]
async fn de_cel_geeft_de_stand_van_een_zaak() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = als_lezer(&rt);
    let b = behandelaar(&app).await;
    let case = afnemer_indienen(&app, "12345678").await;
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

    let lees = |q: String| {
        let rt = &rt;
        async move {
            let headers = [(RUNTIME_TOKEN_HEADER, rt.runtime_token.als_str())];
            let (status, body, _) = vraag_met(
                &rt.router,
                "GET",
                &format!("{AFNEMER_CEL}/api/lexostatus/case_state?{q}"),
                &headers,
                None,
            )
            .await;
            (status, body)
        }
    };
    let (status, l) = lees(format!("root={case}")).await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(l["name"], "case_state");
    assert_eq!(l["parameters"], json!({}), "gaat nooit naar de engine");
    let z = &l["extra_fields"];
    assert_eq!(z["grams"], json!(2), "{z}");
    assert_eq!(
        z["events"]["test_afnemer_zaakverloop/besluit_genomen"],
        json!(1)
    );
    // Het besluit is een eigen gram dat naar de aanvraag verwijst; de
    // aanvraag is de wortel.
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

    let (_, l) = lees(format!(
        "root={case}&owner_path=eherkenning.kvk&owner=12345678"
    ))
    .await;
    assert_eq!(l["extra_fields"]["owner"], json!(true), "{l}");
    let (_, l) = lees(format!(
        "root={case}&owner_path=eherkenning.kvk&owner=87654321"
    ))
    .await;
    assert_eq!(l["extra_fields"]["owner"], json!(false), "{l}");
    let (status, _) = lees("root=bestaat-niet".into()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = lees(format!("root={case}&owner=12345678")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // De cel noemt haar bij haar lexostatussen, als lexostatus van de runtime.
    let (_, cells, _) = vraag(&app, "GET", "/api/cells", None, None).await;
    let afnemer = cells
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "test_afnemer")
        .unwrap();
    assert!(
        afnemer["lexostatuses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["name"] == "case_state" && l["runtime"] == json!(true)),
        "{afnemer}"
    );
}

/// Het leestoken geeft lezen, geen vastleggen.
#[tokio::test]
async fn het_leestoken_geeft_alleen_lezen() {
    let data = tempfile::tempdir().unwrap();
    let token = "gedeeld-leestoken-van-de-test";
    let rt = runtime_met_leestoken(&fixtures(), data.path(), token, &[]);
    let lees = [(LEES_TOKEN_HEADER, token)];
    let (status, body, _) = vraag_met(
        &rt.router,
        "GET",
        &format!("{INSTANTIE_CEL}/api/chronicle"),
        &lees,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, _) = vraag_met(
        &rt.router,
        "POST",
        &format!("{INSTANTIE_CEL}/api/grams"),
        &lees,
        Some(verzoek("test_instantie")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Een behandelaar ziet de kroniek en de lexostatussen van de cellen die zijn
/// proces leest, via het proces; een aanvrager en wie niet inlogde niet.
#[tokio::test]
async fn inzage_in_de_cellen_via_het_proces() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = rt.router.clone();
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let chronicle = format!("{AFNEMER}/api/inspection/test_afnemer/chronicle");

    let (status, grams, _) = vraag(&app, "GET", &chronicle, Some(&b), None).await;
    assert_eq!(status, StatusCode::OK, "{grams}");
    assert_eq!(grams.as_array().unwrap().len(), 1);
    let (status, l, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/inspection/test_afnemer/lexostatus/aanvraag_inhoud?root={case}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{l}");
    assert_eq!(l["root"], json!(case));
    // Een bron van het proces in deze runtime mag ook; een andere cel niet.
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/inspection/test_register/chronicle"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/inspection/test_instantie/chronicle"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Zonder login 401, als aanvrager 403.
    let (status, _, _) = vraag(&app, "GET", &chronicle, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, _, a) = vraag(
        &app,
        "POST",
        &format!("{AFNEMER}/api/channels/eherkenning/login"),
        None,
        Some(json!({"kvk": "12345678", "persoon": "A. Tester"})),
    )
    .await;
    let (status, _, _) = vraag(&app, "GET", &chronicle, a.as_deref(), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Het proces noemt de cellen die een behandelaar kan inzien.
    let (_, processen, _) = vraag(&app, "GET", "/api/processes", None, None).await;
    let p = processen
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

// --- Een tweede casus: een maandtoeslag, met meer besluiten in een zaak ---
//
// Hetzelfde binaire programma, dezelfde routes: alleen de configuratie en de
// regelingen verschillen (processes/toeslag, cellen/toeslag,
// regulation/testregeling_toeslag en testbeleid_toeslag).

const TOESLAG: &str = "/processes/test_toeslag_proces";
const TOESLAG_CEL: &str = "/cells/test_toeslag";

async fn toeslag_inloggen(app: &Router) -> String {
    let (status, body, cookie) = vraag(
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

/// Dien een aanvraag voor een maandtoeslag in; de zaak die zij opent.
async fn toeslag_indienen(app: &Router, month: &str, geschat_inkomen: i64) -> String {
    let a = toeslag_inloggen(app).await;
    let (status, body, _) = vraag(
        app,
        "POST",
        &format!("{TOESLAG}/api/application"),
        Some(&a),
        Some(json!({"external": {
            "naam": "A. Voorbeeld",
            "adres": "Voorbeeldstraat 1, 1234 AB Voorbeeld",
            "dagtekening": "2025-03-12",
            "maand": month,
            "geschat_inkomen": geschat_inkomen,
        }})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["gram"]["id"].as_str().unwrap().to_string()
}

/// Een handeling in een toeslagzaak; `gebeurd` meldt een feit dat toch
/// gebeurde.
async fn toeslag(
    app: &Router,
    b: &str,
    case: &str,
    name: &str,
    form: Value,
    happened: bool,
) -> (StatusCode, Value) {
    let body = json!({"form": form, "happened": happened});
    handeling_in(app, TOESLAG, b, case, name, false, body).await
}

fn bekendmaking() -> Value {
    json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": true})
}

/// Het tijdvak is een maand: het beleid biedt maanden aan (als de eerste dag
/// ervan), een maand die nog moet beginnen peilt op haar begin, en de cel
/// leidt de maand van de aanvraag af met periode_van, met de periode die de
/// regeling noemt (temporal.period_type: month).
#[tokio::test]
async fn een_maand_als_tijdvak() {
    let data = tempfile::tempdir().unwrap();
    let app = app(data.path());
    let a = toeslag_inloggen(&app).await;
    let (status, body, _) = vraag(
        &app,
        "GET",
        &format!("{TOESLAG}/api/possibilities"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let m = body["possibilities"].as_array().unwrap();
    let maanden: Vec<&Value> = m
        .iter()
        .map(|m| &m["possibility"]["window"]["value"])
        .collect();
    assert_eq!(
        maanden,
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
    // De lopende maand peilt op vandaag, een komende op haar eerste dag.
    assert_eq!(m[0]["as_of"], "2025-03-12", "{}", m[0]);
    assert!(
        m[1]["as_of"].as_str().unwrap().starts_with("2025-04-01"),
        "{}",
        m[1]
    );
    // De cel leidt de maand af uit het formulier: een dag in de maand is de
    // maand, als haar eerste dag.
    assert_eq!(m[1]["parameters"]["maand"], "2025-04-01");

    let case = toeslag_indienen(&app, "2025-03-17", 90000).await;
    let b = behandelaar_in(&app, TOESLAG).await;
    let (status, p) = handeling_in(
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

/// Een zaak met vier besluiten: een voorschot, een vaststelling, een
/// wijziging van die vaststelling en een terugvordering, elk bekendgemaakt
/// met een eigen bezwaartermijn, met de betaling van het voorschot en de
/// terugbetaling van wat is teruggevorderd. Een tweede vaststelling zonder
/// wijzigingsgrond weigert de cel. Geen regel code verschilt van de afnemer.
#[tokio::test]
async fn meer_besluiten_in_een_zaak() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_op(&fixtures(), data.path()).unwrap();
    let app = als_lezer(&rt);
    let case = toeslag_indienen(&app, "2025-03-01", 90000).await;
    let b = behandelaar_in(&app, TOESLAG).await;

    // Voor het voorschot: bekendmaken en betalen wachten op het besluit.
    let (status, f) = toeslag(
        &app,
        &b,
        &case,
        "voorschot_bekendmaken",
        bekendmaking(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("wacht op het besluit (Voorschot verlenen)"),
        "{f}"
    );

    // 1. Het voorschot: het eerste besluit in de zaak.
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
    // Een tweede voorschot in dezelfde zaak: geen eigen grondslag.
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
        bekendmaking(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bm}");
    assert_eq!(bm["gram"]["refers_to"]["decision"], k1.as_str());
    assert_eq!(bm["gram"]["fields"]["einde_bezwaartermijn"], "2025-04-23");
    // Een besluit wordt een keer bekendgemaakt.
    let (status, f) = toeslag(
        &app,
        &b,
        &case,
        "voorschot_bekendmaken",
        bekendmaking(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"].as_str().unwrap().contains("ligt al in besluit"),
        "{f}"
    );
    let betaling = json!({"bedrag": 12000, "datum_betaling": "2025-03-12"});
    let (status, bt) = toeslag(&app, &b, &case, "voorschot_betalen", betaling, false).await;
    assert_eq!(status, StatusCode::CREATED, "{bt}");
    assert_eq!(bt["gram"]["refers_to"]["decision"], k1.as_str());
    assert_eq!(bt["trial"]["outputs"]["nog_te_betalen_voorschot"], json!(0));

    // 2. De vaststelling: een tweede besluit, van een eigen artikel.
    let vaststelling = json!({"vastgesteld_inkomen": 150000, "vaststellingsdatum": "2025-03-12"});
    let (status, vs) = toeslag(&app, &b, &case, "vaststellen", vaststelling.clone(), false).await;
    assert_eq!(status, StatusCode::CREATED, "{vs}");
    let k2 = vs["gram"]["id"].as_str().unwrap().to_string();
    assert_eq!(vs["gram"]["refers_to"]["on_application"], case.as_str());
    assert_eq!(vs["gram"]["fields"]["vastgestelde_toeslag"], json!(6000));
    // Een tweede vaststelling zonder wijzigingsgrond: de proef zegt het, en
    // de cel weigert zo'n gram zelf ook.
    let (status, f) = toeslag(&app, &b, &case, "vaststellen", vaststelling, false).await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"].as_str().unwrap().contains("eigen grondslag"),
        "{f}"
    );
    let (status, f) = als_runtime(
        &rt,
        "POST",
        &format!("{TOESLAG_CEL}/api/grams"),
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
        f["error"].as_str().unwrap().contains("verwijzing wijzigt"),
        "{f}"
    );
    // Een gram dat een besluit volgt, noemt een besluit dat in de zaak ligt.
    let bekendmaking_van = |decision: Option<String>| {
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
            "geen gram",
        ),
        (None, "verplicht met 'decision'"),
    ] {
        let (status, f) = als_runtime(
            &rt,
            "POST",
            &format!("{TOESLAG_CEL}/api/grams"),
            bekendmaking_van(decision),
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
        bekendmaking(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bm}");
    assert_eq!(bm["gram"]["refers_to"]["decision"], k2.as_str());

    // 3. De wijziging van de vaststelling: een eigen besluit met een eigen
    // grondslag. Zonder nieuwe feiten is er niets te wijzigen.
    let wijziging = |nieuw: bool| json!({"gecorrigeerd_inkomen": 250000, "nieuwe_feiten": nieuw, "wijzigingsdatum": "2025-03-12"});
    let (status, f) = toeslag(
        &app,
        &b,
        &case,
        "vaststelling_wijzigen",
        wijziging(false),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{f}");
    assert!(
        f["error"]
            .as_str()
            .unwrap()
            .contains("geeft geen waarde voor gewijzigde_toeslag"),
        "{f}"
    );
    let (status, w) = toeslag(
        &app,
        &b,
        &case,
        "vaststelling_wijzigen",
        wijziging(true),
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
        bekendmaking(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bm}");
    assert_eq!(bm["gram"]["refers_to"]["decision"], k3.as_str());

    // Een handeling handelt op het laatste besluit, tenzij de behandelaar er
    // een noemt: een tweede wijziging kan de oorspronkelijke vaststelling
    // wijzigen. Een besluit waarop zij niet handelt, weigert het proces.
    let wijzigen = |id: &str| json!({"form": wijziging(true), "decision": id});
    let (status, p) = handeling_in(
        &app,
        TOESLAG,
        &b,
        &case,
        "vaststelling_wijzigen",
        true,
        json!({"form": wijziging(true)}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["decision"]["id"], k3.as_str());
    let (status, p) = handeling_in(
        &app,
        TOESLAG,
        &b,
        &case,
        "vaststelling_wijzigen",
        true,
        wijzigen(&k2),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["decision"]["id"], k2.as_str());
    let (status, p) = handeling_in(
        &app,
        TOESLAG,
        &b,
        &case,
        "vaststelling_wijzigen",
        true,
        wijzigen(&k1),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["takeable"], json!(false), "{p}");
    assert!(
        p["reason"]
            .as_str()
            .unwrap()
            .contains("is geen besluit waarop handeling 'vaststelling_wijzigen' handelt"),
        "{p}"
    );

    // 4. De terugvordering: het voorschot min de toeslag zoals die nu is
    // vastgesteld.
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
        bekendmaking(),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bm}");

    // De terugbetaling voert de terugvordering uit (naar Awb 4:57).
    let terug = |bedrag: i64| json!({"bedrag": bedrag, "datum_terugbetaling": "2025-03-12"});
    let (status, tb) = toeslag(&app, &b, &case, "terugbetalen", terug(5000), false).await;
    assert_eq!(status, StatusCode::CREATED, "{tb}");
    assert_eq!(tb["gram"]["refers_to"]["decision"], k4.as_str());
    assert_eq!(tb["trial"]["outputs"]["nog_terug_te_betalen"], json!(7000));
    // Het type en de eenheid komen uit de regeling, niet uit de naam.
    assert_eq!(
        tb["trial"]["types"]["nog_terug_te_betalen"],
        json!({"type": "amount", "unit": "eurocent"})
    );
    assert_eq!(
        tb["trial"]["types"]["terugbetaling_conform"],
        json!({"type": "boolean"})
    );
    let (status, f) = toeslag(&app, &b, &case, "terugbetalen", terug(7001), false).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "boven het teruggevorderde: {f}"
    );
    let (status, tb) = toeslag(&app, &b, &case, "terugbetalen", terug(7001), true).await;
    assert_eq!(status, StatusCode::CREATED, "gemeld als gebeurd: {tb}");

    // Het zaakscherm: vier besluiten, elk met zijn stages, zijn route en
    // zijn handelingen.
    let (status, z, _) = vraag(
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
    let terugbetalen = z["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "terugbetalen")
        .unwrap();
    assert_eq!(terugbetalen["decision"], k4.as_str());
    assert_eq!(terugbetalen["recorded"], json!(2));
    let bedrag = terugbetalen["form"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "bedrag")
        .unwrap();
    assert_eq!(bedrag["type"], "amount", "{bedrag}");
    assert_eq!(bedrag["unit"], "eurocent", "{bedrag}");
    assert_eq!(
        terugbetalen["trial"]["outputs"]["nog_terug_te_betalen"],
        json!(0),
        "{terugbetalen}"
    );
    let vaststellen = z["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "vaststellen")
        .unwrap();
    assert_eq!(vaststellen["available"], json!(false));

    // De zaakstand van de cel: de besluiten, elk met zijn stages.
    let (status, l, _) = vraag(
        &app,
        "GET",
        &format!("{TOESLAG_CEL}/api/lexostatus/case_state?root={case}"),
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

// --- Experiment A: de engine-route (CEL_REDUCTIE) ---

/// Een runtime over de fixtures met deze reductiemodus.
fn runtime_met_reductie(data: &Path, reduction: Reductiemodus) -> Result<Runtime, Vec<String>> {
    let config = Config {
        cells_path: fixtures().join("cells"),
        processes_path: Some(fixtures().join("processes")),
        regulation_path: fixtures().join("regulation"),
        data_dir: data.to_path_buf(),
        port: STANDAARD_POORT,
        lees_token: None,
        lees_token_bronnen: Vec::new(),
        reduction,
        registers: None,
    };
    Runtime::laad(&config, klok())
}

fn koppeling() -> PathBuf {
    fixtures().join("experiment/engine/koppeling.yaml")
}

/// Wat een antwoord zegt, zonder wat per run verschilt (tijdstippen,
/// hashes) en zonder de route van de reductie.
fn zonder_run(w: &Value) -> Value {
    const WEG: &[&str] = &[
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
                .filter(|(k, _)| !WEG.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), zonder_run(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(zonder_run).collect()),
        Value::String(t) => Value::String(zonder_uuid(t)),
        _ => w.clone(),
    }
}

/// Een tekst met elk kenmerk (een uuid van 36 tekens) als `<kenmerk>`.
fn zonder_uuid(t: &str) -> String {
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
    let mut uit = String::new();
    let mut i = 0;
    while i < t.len() {
        if t.is_char_boundary(i)
            && t.len() - i >= 36
            && t.is_char_boundary(i + 36)
            && is_uuid(&t[i..i + 36])
        {
            uit.push_str("<kenmerk>");
            i += 36;
        } else {
            let c = t[i..].chars().next().unwrap();
            uit.push(c);
            i += c.len_utf8();
        }
    }
    uit
}

/// De hele weg door de fixtures: toets en indienen bij de instantie en de
/// afnemer, en bij de afnemer het besluit (met de synthese per regel), de
/// bekendmaking, de betaling en de zaak. Elk antwoord, zonder wat per run
/// verschilt.
async fn fixtureflow(rt: &Runtime) -> Vec<(String, StatusCode, Value)> {
    let app = als_lezer(rt);
    let mut uit = Vec::new();
    let mut noteer = |stap: &str, status: StatusCode, w: &Value| {
        uit.push((stap.to_string(), status, zonder_run(w)));
    };
    let c = logins(&app, "12345678").await;
    let (s, w, _) = vraag(
        &app,
        "POST",
        &format!("{INSTANTIE}/api/application/assessment"),
        Some(&c),
        Some(volledig()),
    )
    .await;
    noteer("toets instantie", s, &w);
    for a in [Some("VOORBEELD"), None] {
        let w = afnemer_toets(&app, a).await;
        noteer("toets afnemer", StatusCode::OK, &w);
    }
    let case = afnemer_indienen(&app, "12345678").await;
    let b = behandelaar(&app).await;
    let (s, w) = action(&app, &b, &case, "aanvulling_vragen", true, json!({})).await;
    noteer("aanvulling op proef", s, &w);
    let (s, w) = action(&app, &b, &case, "besluit", true, verdicts()["form"].clone()).await;
    noteer("besluit op proef", s, &w);
    let (s, w) = action(
        &app,
        &b,
        &case,
        "besluit",
        false,
        verdicts()["form"].clone(),
    )
    .await;
    noteer("besluit", s, &w);
    let (s, w) = action(
        &app,
        &b,
        &case,
        "bekendmaken",
        false,
        json!({"datum_bekendmaking": "2025-03-12", "bekendgemaakt": true}),
    )
    .await;
    noteer("bekendmaken", s, &w);
    let (s, w) = action(
        &app,
        &b,
        &case,
        "betalen",
        false,
        json!({"bedrag": 6000, "datum_betaling": "2025-03-12"}),
    )
    .await;
    noteer("betalen", s, &w);
    let (s, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/cases/{case}"),
        Some(&b),
        None,
    )
    .await;
    noteer("zaak", s, &w);
    let (s, w, _) = vraag(
        &app,
        "GET",
        &format!("{AFNEMER}/api/worklist"),
        Some(&b),
        None,
    )
    .await;
    noteer("werkvoorraad", s, &w);
    uit
}

/// De engine-route geeft langs de hele weg dezelfde antwoorden als de
/// reductie-DSL. In `vergelijk` reduceert elke cel ook langs de DSL en is
/// elk verschil een fout, dus een verschil in een bron die de synthese stil
/// als fout zou tonen, valt hier ook op.
#[tokio::test]
async fn de_engine_route_geeft_dezelfde_uitkomsten_als_de_dsl() {
    let (d1, d2, d3) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let dsl = fixtureflow(&runtime_met_reductie(d1.path(), Reductiemodus::Dsl).unwrap()).await;
    for (d, vergelijk) in [(&d2, false), (&d3, true)] {
        let rt = runtime_met_reductie(
            d.path(),
            Reductiemodus::Engine {
                koppeling: koppeling(),
                vergelijk,
            },
        )
        .unwrap();
        let engine = fixtureflow(&rt).await;
        for ((stap, s1, w1), (_, s2, w2)) in dsl.iter().zip(&engine) {
            assert_eq!((s1, w1), (s2, w2), "{stap} (vergelijk: {vergelijk})");
        }
    }
    // Het besluit van de afnemer: 4 x 1000 + 2 x 500 en de rest, ook hier.
    let decision = &dsl
        .iter()
        .find(|(s, _, _)| s == "besluit op proef")
        .unwrap()
        .2;
    assert_eq!(
        decision["outputs"]["gebiedsbedrag"],
        json!(5000),
        "{decision}"
    );
}

/// In een runtime met de engine-route zegt elke lexostatus langs welke route
/// zij kwam, met de regeling; de beschrijving van de cel zegt het per
/// lexostatus, ook voor een lexostatus die bewust langs de DSL gaat; op
/// verzoek komt de trace van de engine-run mee.
#[tokio::test]
async fn de_engine_route_is_zichtbaar() {
    let data = tempfile::tempdir().unwrap();
    let rt = runtime_met_reductie(
        data.path(),
        Reductiemodus::Engine {
            koppeling: koppeling(),
            vergelijk: false,
        },
    )
    .unwrap();
    let app = als_lezer(&rt);
    let (_, l, _) = vraag(
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
    let (_, cells, _) = vraag(&app, "GET", "/api/cells", None, None).await;
    let afnemer = cells
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "test_afnemer")
        .unwrap();
    assert_eq!(afnemer["reduction"], "engine");
    let worklist = afnemer["lexostatuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "werkvoorraad")
        .unwrap();
    assert_eq!(worklist["reduction"]["route"], "dsl");
    // De zaakstand is code van de runtime, geen reductie: ook dat staat er.
    let zaakstand = afnemer["lexostatuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["runtime"] == json!(true))
        .unwrap();
    assert_eq!(zaakstand["reduction"]["route"], "runtime");
    // Zonder de engine-route zegt de beschrijving er niets over.
    let d = tempfile::tempdir().unwrap();
    let (_, cells, _) = vraag(&app_dsl(d.path()), "GET", "/api/cells", None, None).await;
    assert!(cells[0].get("reduction").is_none(), "{cells}");
}

fn app_dsl(data: &Path) -> Router {
    als_lezer(&runtime_met_reductie(data, Reductiemodus::Dsl).unwrap())
}

/// Geen stille terugval: een lexostatus zonder koppeling, een regeling
/// zonder de uitkomst van een afleiding, of een lijst via de engine houdt de
/// runtime tegen, met elke fout.
#[test]
fn een_onvolledige_koppeling_houdt_de_runtime_tegen() {
    let map = tempfile::tempdir().unwrap();
    let source = std::fs::read_to_string(koppeling()).unwrap();
    let kapot = source
        .replace("    tarief: gebieden_tarief.yaml\n", "    {}\n")
        .replace(
            "registratie_per_gebied: register_registratie_per_gebied.yaml",
            "registratie_per_gebied: register_register.yaml",
        )
        .replace(
            "    werkvoorraad:\n      dsl: een lijst met een regel per zaak (groepeer); de engine kent geen groeperen\n",
            "    werkvoorraad: afnemer_besluit.yaml\n",
        );
    // De regelingen blijven waar ze staan: hun paden worden absoluut.
    let bij = koppeling().parent().unwrap().display().to_string();
    let kapot = kapot
        .replace("  test_gebieden:\n    {}\n", "  test_gebieden: {}\n")
        .replace(": ../", &format!(": {bij}/../"))
        .lines()
        .map(|r| match r.split_once(": ") {
            Some((k, v)) if v.ends_with(".yaml") && !v.starts_with('/') => {
                format!("{k}: {bij}/{v}")
            }
            _ => r.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let path = map.path().join("koppeling.yaml");
    std::fs::write(&path, kapot).unwrap();
    let data = tempfile::tempdir().unwrap();
    let fouten = runtime_met_reductie(
        data.path(),
        Reductiemodus::Engine {
            koppeling: path,
            vergelijk: false,
        },
    )
    .err()
    .unwrap()
    .join("\n");
    assert!(
        fouten.contains("cel 'test_gebieden', lexostatus 'tarief': geen koppeling"),
        "{fouten}"
    );
    assert!(
        fouten.contains("heeft geen uitkomst 'ingeschreven'"),
        "{fouten}"
    );
    assert!(
        fouten.contains("lexostatus 'werkvoorraad': een lijst-lexostatus"),
        "{fouten}"
    );
}

#[test]
fn de_reductiemodus_komt_uit_de_omgeving() {
    assert_eq!(Reductiemodus::uit(None, None), Ok(Reductiemodus::Dsl));
    assert_eq!(
        Reductiemodus::uit(Some("dsl"), None),
        Ok(Reductiemodus::Dsl)
    );
    assert_eq!(
        Reductiemodus::uit(Some("engine"), Some("k.yaml")),
        Ok(Reductiemodus::Engine {
            koppeling: PathBuf::from("k.yaml"),
            vergelijk: false
        })
    );
    assert!(Reductiemodus::uit(Some("engine"), None).is_err());
    assert!(Reductiemodus::uit(None, Some("k.yaml")).is_err());
    assert!(Reductiemodus::uit(Some("anders"), Some("k.yaml")).is_err());
}

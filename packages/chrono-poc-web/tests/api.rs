//! De API over de publieke wereld, met de login uit.
//!
//! Deze tests draaien de **echte** router over het **echte** wereldbestand
//! (`packages/simulator/worlds/publieke_wereld.yaml`) en het echte corpus. Dat is
//! met opzet geen fixture-wereld: wat hier langs HTTP gaat, is precies wat een
//! frontend en een deployment te zien krijgen, en een wereldbestand dat stilletjes
//! onleesbaar wordt hoort hier rood te worden en niet in een container.
//!
//! De login staat uit (geen `OIDC_CLIENT_ID` in de configuratie), zoals lokaal.
//! Dat de rol-gate dán doorlaat is gedrag van `regelrecht-auth` en daar getest;
//! wat hier getoetst wordt is de laag erboven.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use regelrecht_chrono_poc_web::config::{AppConfig, DEFAULT_REQUIRED_ROLE};
use regelrecht_chrono_poc_web::router;
use regelrecht_chrono_poc_web::sources::Source;
use regelrecht_chrono_poc_web::state::AppState;
use regelrecht_chrono_poc_web::worlds::WorldRegistry;
use serde_json::{json, Value};
use tower::ServiceExt;

/// Het wereldbestand waarmee de app lokaal draait.
fn world_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("simulator")
        .join("worlds")
        .join("publieke_wereld.yaml")
}

/// De app zoals `just chrono-poc` hem start: login uit, publieke wereld, corpus
/// uit deze checkout.
async fn app() -> Router {
    let config = AppConfig {
        oidc: None,
        base_url: None,
        required_role: DEFAULT_REQUIRED_ROLE,
        world: Source::Local(world_file()),
        corpus: None,
        auth_ref: None,
        static_dir: "static".to_string(),
        port: 8000,
    };
    let resolved = regelrecht_chrono_poc_web::sources::resolve(&config.world, None, None)
        .await
        .expect("de publieke wereld moet te lezen zijn");
    let worlds = WorldRegistry::new(
        resolved.definition,
        resolved.regulation_root,
        Duration::from_secs(3600),
    );
    router(AppState {
        config: Arc::new(config),
        worlds: Arc::new(worlds),
        oidc_client: None,
        end_session_url: None,
        http_client: regelrecht_auth::http_client(),
    })
}

/// Eén browser: houdt zijn sessiecookie vast tussen verzoeken.
struct Browser {
    app: Router,
    cookie: Option<String>,
}

impl Browser {
    async fn new() -> Self {
        Self {
            app: app().await,
            cookie: None,
        }
    }

    /// Een tweede browser op dezelfde server: dezelfde router, geen cookie. Zo
    /// zijn twee sessies twee sessies en niet twee servers — anders zou de test
    /// "twee werelden" ook slagen als de state per router zou leven.
    fn tweede(&self) -> Self {
        Self {
            app: self.app.clone(),
            cookie: None,
        }
    }

    async fn send(
        &mut self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(cookie) = &self.cookie {
            request = request.header(COOKIE, cookie);
        }
        let request = match body {
            Some(json) => request
                .header("content-type", "application/json")
                .body(Body::from(json.to_string())),
            None => request.body(Body::empty()),
        }
        .expect("verzoek moet te bouwen zijn");

        let response = self
            .app
            .clone()
            .oneshot(request)
            .await
            .expect("de app moet antwoorden");
        let status = response.status();
        if let Some(set) = response.headers().get(SET_COOKIE) {
            let raw = set.to_str().expect("cookie is tekst");
            self.cookie = Some(
                raw.split(';')
                    .next()
                    .expect("een cookie heeft een naam=waarde")
                    .to_string(),
            );
        }
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body moet te lezen zijn")
            .to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()))
        };
        (status, json)
    }

    async fn get(&mut self, uri: &str) -> (StatusCode, Value) {
        self.send(Method::GET, uri, None).await
    }

    async fn post(&mut self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, uri, Some(body)).await
    }

    async fn put(&mut self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.send(Method::PUT, uri, Some(body)).await
    }

    /// Het beeld van de wereld, of een panic met de status erin.
    async fn world(&mut self) -> Value {
        let (status, body) = self.get("/api/world").await;
        assert_eq!(status, StatusCode::OK, "GET /api/world: {body}");
        body
    }
}

/// Alle grammen in één kroniek van één cel, uit een beeld.
fn grams(world: &Value, cell: &str, chronicle: &str) -> Vec<Value> {
    world["cells"]
        .as_array()
        .expect("cells is een lijst")
        .iter()
        .find(|candidate| candidate["id"] == cell)
        .unwrap_or_else(|| panic!("cel '{cell}' hoort in het beeld te staan"))["chronicles"]
        .as_array()
        .expect("chronicles is een lijst")
        .iter()
        .find(|candidate| candidate["stream"] == chronicle)
        .unwrap_or_else(|| panic!("kroniek '{chronicle}' hoort bij cel '{cell}' te staan"))["grams"]
        .as_array()
        .expect("grams is een lijst")
        .clone()
}

/// Of een actie nu kan, volgens het beeld.
fn available(world: &Value, action: &str) -> bool {
    world["actions"]
        .as_array()
        .expect("actions is een lijst")
        .iter()
        .find(|candidate| candidate["id"] == action)
        .unwrap_or_else(|| panic!("actie '{action}' hoort in het beeld te staan"))["available"]
        == json!(true)
}

/// De aanvraag, met de velden die haar formulier documenteert.
fn aanvraag() -> Value {
    json!({ "bsn": "999993653", "jaar": 2024 })
}

/// `/health` is de enige route zonder login, en hij zegt niets over een wereld:
/// een healthcheck hoort geen regelingen in te lezen.
#[tokio::test]
async fn health_staat_open_en_maakt_geen_wereld() {
    let mut browser = Browser::new().await;
    let (status, body) = browser.get("/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!("OK"));
}

/// Het beeld van een verse wereld: de klok op de startdatum, de vier cellen van
/// het wereldbestand, en precies één actie die nu kan.
#[tokio::test]
async fn een_verse_wereld_staat_op_de_startdatum() {
    let mut browser = Browser::new().await;
    let world = browser.world().await;

    assert_eq!(world["clock"], json!("2024-01-01"));
    let cells: Vec<&str> = world["cells"]
        .as_array()
        .expect("cells is een lijst")
        .iter()
        .filter_map(|cell| cell["id"].as_str())
        .collect();
    assert_eq!(cells, vec!["belastingdienst", "brp", "burger", "toeslagen"]);
    assert_eq!(world["settings"]["betalingsritme"], json!("kwartaal"));

    assert!(
        available(&world, "burger.aanvraag"),
        "een aanvraag indienen kan altijd"
    );
    assert!(
        !available(&world, "toeslagen.toekenning"),
        "beslissen kan pas als er een aanvraag ligt"
    );
}

/// Eén aanvraag is twee grammen: de aanvrager legt vast wat zij indiende, de
/// uitvoerder wat hem geleverd is. Beide staan in het beeld, en de stap zelf
/// vertelt in welke volgorde ze ontstonden.
#[tokio::test]
async fn een_actie_legt_vast_in_twee_kronieken() {
    let mut browser = Browser::new().await;

    let (status, step) = browser
        .post("/api/actions/burger.aanvraag", aanvraag())
        .await;
    assert_eq!(status, StatusCode::OK, "{step}");

    let recordings = step["events"]["recordings"]
        .as_array()
        .expect("recordings is een lijst");
    assert_eq!(recordings.len(), 2, "{step}");
    assert_eq!(recordings[0]["cell"], json!("burger"));
    assert_eq!(recordings[0]["gram"], json!("aanvraag_ingediend"));
    assert_eq!(recordings[1]["cell"], json!("toeslagen"));
    assert_eq!(recordings[1]["gram"], json!("aanvraag_ontvangen"));

    let world = &step["snapshot"];
    assert_eq!(grams(world, "burger", "aanvragen").len(), 1);
    assert_eq!(grams(world, "toeslagen", "aanvragen").len(), 1);
    assert!(
        available(world, "toeslagen.toekenning"),
        "nu er een aanvraag ligt, kan beslissen"
    );
}

/// Een actie die nu niet kan is geen fout in het verzoek en geen defect: het
/// verhaal is nog niet zover. 409, met de uitleg van de wereld in het lichaam.
#[tokio::test]
async fn een_actie_die_nu_niet_kan_is_een_conflict() {
    let mut browser = Browser::new().await;

    let (status, body) = browser
        .post(
            "/api/actions/toeslagen.toekenning",
            json!({ "bsn": "999993653" }),
        )
        .await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let error = body["error"].as_str().expect("een fout heeft een melding");
    assert!(error.contains("toeslagen.toekenning"), "{error}");
}

/// De klok doet het werk: een termijn die verstrijkt zonder dat het feit er ligt,
/// levert een waarschuwing op — geen fout, want de uitvoerder mag alsnog
/// besluiten en de wet zegt alleen wat de termijn was.
#[tokio::test]
async fn de_klok_vooruit_laat_een_termijn_verstrijken() {
    let mut browser = Browser::new().await;

    let (status, step) = browser
        .post("/api/advance", json!({ "until": "2024-04-01" }))
        .await;
    assert_eq!(status, StatusCode::OK, "{step}");
    assert_eq!(step["snapshot"]["clock"], json!("2024-04-01"));

    let warnings = step["events"]["warnings"]
        .as_array()
        .expect("warnings is een lijst");
    assert_eq!(warnings.len(), 1, "{step}");
    assert_eq!(warnings[0]["cell"], json!("toeslagen"));
    assert_eq!(warnings[0]["name"], json!("aanvraag_ontvangen"));

    // Achteruit kan de logische klok niet: dan zou een feit kunnen ontstaan vóór
    // het feit waarop het rust.
    let (status, body) = browser
        .post("/api/advance", json!({ "until": "2024-01-01" }))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

/// Het volledige verhaal over HTTP: aanvraag, klok vooruit, besluit. Het besluit
/// haalt het toetsingsinkomen **over een celgrens** bij de cel die het vaststelde
/// en rekent het niet na — dat contact staat in het beeld, en de stap zegt dat het
/// er één was.
#[tokio::test]
async fn een_besluit_accepteert_een_waarde_over_een_celgrens() {
    let mut browser = Browser::new().await;

    browser
        .post("/api/actions/burger.aanvraag", aanvraag())
        .await;
    browser
        .post("/api/advance", json!({ "until": "2024-04-01" }))
        .await;
    let (status, step) = browser
        .post(
            "/api/actions/toeslagen.toekenning",
            json!({ "bsn": "999993653" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{step}");

    let decisions = step["events"]["decisions"]
        .as_array()
        .expect("decisions is een lijst");
    assert_eq!(decisions.len(), 1, "{step}");
    assert_eq!(decisions[0]["cell"], json!("toeslagen"));
    assert_eq!(decisions[0]["besluit"], json!("zorgtoeslag_toekenning"));
    assert_eq!(decisions[0]["zaakkenmerk"], json!("zorgtoeslag/999993653"));
    assert_eq!(
        decisions[0]["crossings"],
        json!(1),
        "het toetsingsinkomen komt van een andere cel"
    );

    let crossings = step["snapshot"]["crossings"]
        .as_array()
        .expect("crossings is een lijst");
    assert_eq!(crossings.len(), 1);
    assert_eq!(crossings[0]["answer"]["cell"], json!("belastingdienst"));
    assert_eq!(crossings[0]["answer"]["name"], json!("toetsingsinkomen"));

    // De beschikking ligt nu in de kroniek van de cel die besloot, en nergens
    // anders — op te vragen met een gewone reductie.
    let (status, answer) = browser
        .get("/api/cells/toeslagen/lexostatus/zorgtoeslagbeschikking?zaakkenmerk=zorgtoeslag/999993653")
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer["outcome"]["established"]["heeft_recht_op_zorgtoeslag"],
        json!(true),
        "{answer}"
    );
}

/// Een bron-cel zonder engine antwoordt net zo goed, en "niets vastgesteld" is
/// een **antwoord** met een reden: 200, en te onderscheiden van een antwoord met
/// waarden.
#[tokio::test]
async fn niets_vastgesteld_is_een_gewoon_antwoord() {
    let mut browser = Browser::new().await;
    // De klok moet voorbij de vastlegging staan: een vraag over een moment ná de
    // klok is geen voorspelling maar een fout.
    browser
        .post("/api/advance", json!({ "until": "2024-06-01" }))
        .await;

    let (status, answer) = browser
        .get("/api/cells/brp/lexostatus/partnerschap?bsn=999993653")
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["cell"], json!("brp"));
    assert_eq!(
        answer["outcome"]["established"]["partnerschap_type"],
        json!("GEEN"),
        "{answer}"
    );

    let (status, answer) = browser
        .get("/api/cells/brp/lexostatus/partnerschap?bsn=999993653&op_moment=2023-01-01")
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert!(
        answer["outcome"]["not_established"]["reason"].is_string(),
        "vóór de vastlegging hoort er een reden te staan: {answer}"
    );
}

/// Een BSN is tekst, ook al bestaat hij uit cijfers. Zonder de omzetting naar het
/// gedocumenteerde type zou dezelfde vraag hier "niets vastgesteld" opleveren
/// terwijl de vastlegging er ligt — het stilste soort verkeerd antwoord.
#[tokio::test]
async fn een_parameter_van_het_verkeerde_type_wordt_geweigerd() {
    let mut browser = Browser::new().await;

    let (status, body) = browser
        .get("/api/cells/brp/lexostatus/partnerschap?burgerservicenummer=999993653")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("bsn")),
        "{body}"
    );

    let (status, body) = browser
        .get("/api/cells/kiesraad/lexostatus/registratie?bsn=999993653")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

/// Een instelling is beleid en geen wet, dus ze mag om — tot een besluit erop
/// geleund heeft. Daarna staat ze vast, en dat is wat een decretogram betekent.
#[tokio::test]
async fn een_instelling_mag_om_tot_een_besluit_haar_gebruikt() {
    let mut browser = Browser::new().await;

    let (status, world) = browser
        .put("/api/settings", json!({ "betalingsritme": "maand" }))
        .await;
    assert_eq!(status, StatusCode::OK, "{world}");
    assert_eq!(world["settings"]["betalingsritme"], json!("maand"));

    let (status, body) = browser
        .put("/api/settings", json!({ "betalingsritmen": "maand" }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // Neem het besluit dat het ritme gebruikt; daarna staat de instelling vast.
    browser
        .post("/api/actions/burger.aanvraag", aanvraag())
        .await;
    browser
        .post("/api/advance", json!({ "until": "2024-04-01" }))
        .await;
    let (status, step) = browser
        .post(
            "/api/actions/toeslagen.toekenning",
            json!({ "bsn": "999993653" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{step}");
    assert_eq!(
        step["snapshot"]["locked_settings"]["betalingsritme"]["besluit"],
        json!("zorgtoeslag_toekenning"),
        "{step}"
    );

    let (status, body) = browser
        .put("/api/settings", json!({ "betalingsritme": "kwartaal" }))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

/// Opnieuw beginnen is opnieuw uit het wereldbestand: de klok terug, de kronieken
/// terug naar de startstand.
#[tokio::test]
async fn reset_begint_opnieuw_uit_het_wereldbestand() {
    let mut browser = Browser::new().await;

    browser
        .post("/api/actions/burger.aanvraag", aanvraag())
        .await;
    browser
        .post("/api/advance", json!({ "until": "2024-06-01" }))
        .await;

    let (status, world) = browser.post("/api/reset", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{world}");
    assert_eq!(world["clock"], json!("2024-01-01"));
    assert!(
        grams(&world, "burger", "aanvragen").is_empty(),
        "de aanvraag hoort weg te zijn: {world}"
    );
    assert_eq!(
        grams(&world, "belastingdienst", "aanslagen").len(),
        2,
        "de startstand uit het wereldbestand hoort er weer te staan: {world}"
    );
}

/// De kern van "een wereld per sessie": wat de ene browser doet, ziet de andere
/// niet. Zelfde server, zelfde wereldbestand, twee werelden.
#[tokio::test]
async fn twee_sessies_zijn_twee_werelden() {
    let mut een = Browser::new().await;
    let mut twee = een.tweede();

    een.post("/api/actions/burger.aanvraag", aanvraag()).await;
    een.post("/api/advance", json!({ "until": "2024-06-01" }))
        .await;

    let van_een = een.world().await;
    let van_twee = twee.world().await;

    assert_eq!(van_een["clock"], json!("2024-06-01"));
    assert_eq!(van_twee["clock"], json!("2024-01-01"));
    assert_eq!(grams(&van_een, "burger", "aanvragen").len(), 1);
    assert!(
        grams(&van_twee, "burger", "aanvragen").is_empty(),
        "de tweede sessie hoort de aanvraag van de eerste niet te zien: {van_twee}"
    );

    // En de eerste sessie blijft bij haar eigen wereld: de tweede heeft er niets
    // aan veranderd.
    assert_eq!(een.world().await["clock"], json!("2024-06-01"));
}

/// Wat er niet is, is een 404 met de namen die er wél zijn; een body die geen
/// JSON is, is een 400. Beide als JSON met een Nederlandse melding, want een
/// client die per status een andere vorm moet uitpakken, pakt er één verkeerd uit.
#[tokio::test]
async fn fouten_zijn_json_en_nederlands() {
    let mut browser = Browser::new().await;

    let (status, body) = browser
        .post("/api/actions/burger.aanvraagje", aanvraag())
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("burger.aanvraag")),
        "de melding hoort de acties te noemen die er wél zijn: {body}"
    );

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/advance")
        .header("content-type", "application/json")
        .body(Body::from("{ tot:"))
        .expect("verzoek");
    let response = app()
        .await
        .oneshot(request)
        .await
        .expect("de app moet antwoorden");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body: Value = serde_json::from_slice(&bytes).expect("een fout is JSON");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("geldige JSON")),
        "{body}"
    );
}

/// Een actie met een veld dat haar formulier niet documenteert, en een veld van
/// het verkeerde type: beide geweigerd door de wereld zelf, en beide een 400.
#[tokio::test]
async fn een_formulier_dat_niet_klopt_wordt_geweigerd() {
    let mut browser = Browser::new().await;

    let mut te_veel: BTreeMap<&str, Value> = BTreeMap::new();
    te_veel.insert("bsn", json!("999993653"));
    te_veel.insert("jaar", json!(2024));
    te_veel.insert("toeslagjaar", json!(2024));
    let (status, body) = browser
        .post("/api/actions/burger.aanvraag", json!(te_veel))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = browser
        .post(
            "/api/actions/burger.aanvraag",
            json!({ "bsn": 999_993_653_i64, "jaar": 2024 }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

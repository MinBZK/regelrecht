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
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
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
    app_with_static("static").await
}

/// Dezelfde app, met een eigen map voor de statische bestanden. Waarmee te
/// toetsen is wat de server met die map doet zonder een bundel te bouwen.
async fn app_with_static(static_dir: &str) -> Router {
    let config = AppConfig {
        oidc: None,
        base_url: None,
        required_role: DEFAULT_REQUIRED_ROLE,
        world: Source::Local(world_file()),
        corpus: None,
        auth_ref: None,
        static_dir: static_dir.to_string(),
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
///
/// De datum staat er als ISO in, want dat is wat er op de draad hoort te staan.
/// Wat een mens ziet en typt (dd-mm-jjjj) is een zaak van het veld in de
/// frontend en komt hier nooit langs.
fn aanvraag() -> Value {
    json!({ "bsn": "999993653", "jaar": 2024, "ondertekend_op": "2024-01-09" })
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

/// Eén cel uit een beeld.
fn cell<'a>(world: &'a Value, id: &str) -> &'a Value {
    world["cells"]
        .as_array()
        .expect("cells is een lijst")
        .iter()
        .find(|candidate| candidate["id"] == id)
        .unwrap_or_else(|| panic!("cel '{id}' hoort in het beeld te staan"))
}

/// Wat een cel belooft, staat in het beeld: de toelichting bij een naam, de
/// parameters die ze verlangt, en waar een zaakkenmerk vandaan komt.
///
/// Dit is de reden dat het erin staat. Een consument moet precies de
/// gedocumenteerde parameters meegeven, en één daarvan — de sleutel van de
/// kroniek met beschikkingen — bestaat nergens voordat er besloten is: ze ontstaat
/// uit het sjabloon van een besluit-definitie. Wie alleen het beeld heeft en niet
/// het wereldbestand, moest dat kenmerk dus raden.
#[tokio::test]
async fn het_beeld_draagt_wat_een_cel_belooft() {
    let mut browser = Browser::new().await;
    let world = browser.world().await;
    let toeslagen = cell(&world, "toeslagen");

    let beschikking = toeslagen["lexostatussen"]
        .as_array()
        .expect("lexostatussen is een lijst")
        .iter()
        .find(|candidate| candidate["name"] == "zorgtoeslagbeschikking")
        .expect("deze cel publiceert wat ze besloten heeft");
    assert!(
        beschikking["doc"]
            .as_str()
            .is_some_and(|doc| !doc.is_empty()),
        "de toelichting van de definitie hoort mee te komen: {beschikking}"
    );
    assert_eq!(
        beschikking["inputs"],
        json!([{"name": "zaakkenmerk", "type": "string"}]),
        "de parameters met hun type zijn wat de cel accepteert"
    );
    assert_eq!(
        beschikking["key"],
        json!({"chronicle": "beschikkingen", "parameter": "zaakkenmerk"}),
        "en de sleutel zegt over welke kroniek de reductie gaat"
    );

    let besluit = toeslagen["besluiten"]
        .as_array()
        .expect("besluiten is een lijst")
        .iter()
        .find(|candidate| candidate["name"] == "zorgtoeslag_toekenning")
        .expect("deze cel kan toekennen");
    assert_eq!(
        besluit["zaakkenmerk"],
        json!("zorgtoeslag/{bsn}"),
        "het sjabloon zegt welke vorm de sleutel krijgt"
    );
    assert_eq!(
        besluit["chronicle"],
        json!("beschikkingen"),
        "en in welke kroniek de decretogrammen ervan landen"
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

/// Een besluit dat de cel **weigert** is geen serverfout. De aanvraag ligt er,
/// dus de actie kan; wat er niet ligt is het toetsingsinkomen dat het besluit
/// bij een andere cel moet ophalen. De cel rekent dan niet door met een gat en
/// legt niets vast — en dat is precies wat er hoort te gebeuren, dus komt het
/// terug als 409 met haar eigen reden erin.
///
/// De 500 die dit ooit was, maakte de weigering niet te onderscheiden van een
/// kapotte server: een frontend kon er niets anders van maken dan "er ging iets
/// mis", terwijl het antwoord juist te lezen valt.
#[tokio::test]
async fn een_besluit_dat_de_cel_weigert_is_een_conflict() {
    let mut browser = Browser::new().await;

    let (status, step) = browser
        .post("/api/actions/burger.aanvraag", aanvraag())
        .await;
    assert_eq!(status, StatusCode::OK, "{step}");
    assert!(
        available(&step["snapshot"], "toeslagen.toekenning"),
        "de actie hoort te kunnen: het gaat hier om de weigering van het besluit \
         zelf en niet om een dichte poort: {step}"
    );

    // De klok staat nog op de startdag: de cel die het toetsingsinkomen
    // vaststelt, heeft dat dan nog niet gedaan.
    let (status, body) = browser
        .post(
            "/api/actions/toeslagen.toekenning",
            json!({ "bsn": "999993653" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let error = body["error"].as_str().expect("een fout heeft een melding");
    assert!(
        error.contains("niets vast"),
        "de reden van de cel hoort door te komen: {error}"
    );

    // En er is niets vastgelegd: een besluit dat niet genomen is, laat geen gram
    // achter.
    let world = browser.world().await;
    assert!(
        grams(&world, "toeslagen", "beschikkingen").is_empty(),
        "een weigering legt niets vast: {world}"
    );
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

/// Het receipt van een decretogram is op verzoek op te vragen, terwijl het beeld
/// receipt-loos blijft.
///
/// Dat onderscheid is het punt: een decretogram *is* het RFC-013 Execution
/// Receipt van het besluit (RFC-022 §1.2), maar dat receipt draagt wandkloktijd
/// en hoort dus niet in een contract dat per run hetzelfde moet zijn. Deze route
/// is de weg ernaartoe zonder het beeld te vervuilen.
#[tokio::test]
async fn het_receipt_van_een_decretogram_is_op_te_vragen_naast_het_beeld() {
    let mut browser = Browser::new().await;

    browser
        .post("/api/actions/burger.aanvraag", aanvraag())
        .await;
    browser
        .post("/api/advance", json!({ "until": "2024-04-01" }))
        .await;
    browser
        .post(
            "/api/actions/toeslagen.toekenning",
            json!({ "bsn": "999993653" }),
        )
        .await;

    let world = browser.world().await;
    let beschikkingen = grams(&world, "toeslagen", "beschikkingen");
    assert_eq!(beschikkingen.len(), 1, "{world}");
    assert!(
        // De **sleutel** `"receipt"`, niet de naam: het schema van een
        // decretogram noemt elk veld dat zo'n gram draagt, en `receipt` is er
        // daar een van. Die staat er als waarde (`"name": "receipt"`) en niet
        // als sleutel met een receipt erachter. Dezelfde toets als in
        // `packages/simulator/tests/snapshot.rs`.
        !serde_json::to_string(&world)
            .expect("het beeld moet naar JSON te schrijven zijn")
            .contains("\"receipt\":"),
        "het beeld van de wereld hoort receipt-loos te blijven"
    );

    let (status, receipt) = browser
        .get("/api/cells/toeslagen/chronicles/beschikkingen/grams/0/receipt")
        .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");

    // Het gram waar dit receipt bij hoort, met de zaak erbij: een receipt dat los
    // in handen komt, hoort zichzelf te kunnen plaatsen.
    assert_eq!(receipt["gram"]["cell"], json!("toeslagen"));
    assert_eq!(receipt["gram"]["chronicle"], json!("beschikkingen"));
    assert_eq!(receipt["gram"]["index"], json!(0));
    assert_eq!(
        receipt["gram"]["zaakkenmerk"],
        json!("zorgtoeslag/999993653")
    );

    for sectie in [
        "provenance",
        "engine_config",
        "scope",
        "execution",
        "results",
        "accepted_values",
        "timestamp",
    ] {
        assert!(
            receipt.get(sectie).is_some(),
            "sectie '{sectie}' hoort in het receipt te staan: {receipt}"
        );
    }
    assert_eq!(
        receipt["provenance"]["regulation_id"],
        json!("wet_op_de_zorgtoeslag")
    );
    assert!(
        receipt["scope"]["loaded_regulations"]
            .as_array()
            .expect("loaded_regulations is een lijst")
            .iter()
            .all(|regulation| regulation["hash"].is_string()),
        "elke geladen regeling draagt haar hash: {receipt}"
    );

    // De uitvoeringstrace komt mee, op de plek die RFC-013 ervoor heeft. Zonder
    // haar staat er wel wát eruit kwam, maar niet langs welke artikelen — en dat
    // is waar een lezer een besluit op naslaat. Looptijden horen er juist níet
    // in: die verschillen per run, en een gram dat per run verschilt is geen
    // gram.
    let trace = &receipt["results"]["trace"];
    assert!(
        trace.is_object(),
        "de route hoort de uitvoeringstrace mee te leveren: {receipt}"
    );
    let stappen = serde_json::to_string(trace).expect("een trace is naar JSON te schrijven");
    assert!(
        stappen.contains("\"article\""),
        "een rekenstap hoort het artikel te noemen waar ze vandaan komt: {trace}"
    );
    assert!(
        !stappen.contains("duration_us"),
        "een looptijd verschilt per run en hoort niet in een gram: {trace}"
    );

    // De geaccepteerde waarde noemt de bron-cel én het bevoegd gezag dat die cel
    // erbij noemde. Het adres alleen zou niet zeggen wiens vaststelling dit is.
    let accepted = receipt["accepted_values"]
        .as_array()
        .expect("accepted_values is een lijst")
        .iter()
        .find(|value| value["output"] == json!("toetsingsinkomen"))
        .unwrap_or_else(|| panic!("het toetsingsinkomen is geaccepteerd: {receipt}"));
    assert_eq!(accepted["cell"], json!("belastingdienst"));
    assert_eq!(accepted["authority"], json!("Belastingdienst"));
    assert_eq!(accepted["value"], json!(81000));
    assert_eq!(accepted["zaakkenmerk"], json!("zorgtoeslag/999993653"));

    // De tijdstempel staat er mét het label dat zegt wat voor tijd het is.
    assert!(receipt["timestamp"]["wall_clock"].is_string(), "{receipt}");
    assert!(
        receipt["timestamp"]["note"]
            .as_str()
            .is_some_and(|note| note.contains("wandkloktijd")),
        "{receipt}"
    );
}

/// Wijzen naar iets dat geen receipt heeft, is een 404 met de uitleg erin.
///
/// Drie keer "bestaat niet": een gram dat nooit een uitvoering was, een
/// kroniekstroom die de cel niet houdt, en een plek waar niets ligt. Geen van
/// drieën is een 500 en geen van drieën is een leeg receipt.
#[tokio::test]
async fn een_gram_zonder_receipt_is_een_404() {
    let mut browser = Browser::new().await;
    browser
        .post("/api/actions/burger.aanvraag", aanvraag())
        .await;

    let (status, body) = browser
        .get("/api/cells/toeslagen/chronicles/aanvragen/grams/0/receipt")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("receipt")),
        "{body}"
    );

    let (status, body) = browser
        .get("/api/cells/toeslagen/chronicles/bestaat-niet/grams/0/receipt")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("beschikkingen")),
        "de melding hoort te noemen welke stromen er wél zijn: {body}"
    );

    let (status, body) = browser
        .get("/api/cells/toeslagen/chronicles/beschikkingen/grams/0/receipt")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("geen gram op plek 0")),
        "{body}"
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

/// Het moment van de vraag is de weg terug: dezelfde cel, dezelfde naam, een
/// eerder moment, een ander antwoord. Vooruit is geen moment maar een
/// voorspelling, en dat weigert de wereld — met de klok in de melding, zodat de
/// vrager weet waar de grens ligt.
#[tokio::test]
async fn een_moment_voor_de_klok_kijkt_terug_en_erna_niet() {
    let mut browser = Browser::new().await;
    browser
        .post("/api/advance", json!({ "until": "2024-06-01" }))
        .await;

    // Vóór de vastlegging van 2023-03-01 wist deze cel nog niets; erna wel. Eén
    // vraag, twee momenten, twee antwoorden.
    let (status, eerder) = browser
        .get("/api/cells/brp/lexostatus/partnerschap?bsn=999993653&op_moment=2023-02-28")
        .await;
    assert_eq!(status, StatusCode::OK, "{eerder}");
    assert!(
        eerder["outcome"]["not_established"]["reason"].is_string(),
        "{eerder}"
    );

    let (status, later) = browser
        .get("/api/cells/brp/lexostatus/partnerschap?bsn=999993653&op_moment=2023-03-01")
        .await;
    assert_eq!(status, StatusCode::OK, "{later}");
    assert_eq!(later["op_moment"], json!("2023-03-01"), "{later}");
    assert_eq!(
        later["outcome"]["established"]["partnerschap_type"],
        json!("GEEN"),
        "{later}"
    );

    let (status, body) = browser
        .get("/api/cells/brp/lexostatus/partnerschap?bsn=999993653&op_moment=2024-06-02")
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("2024-06-01")),
        "de melding hoort te zeggen waar de klok staat: {body}"
    );

    // En een moment dat geen datum is, is een verzoekfout met de notatie erin.
    let (status, body) = browser
        .get("/api/cells/brp/lexostatus/partnerschap?bsn=999993653&op_moment=gisteren")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("jjjj-mm-dd")),
        "{body}"
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
            json!({ "bsn": 999_993_653_i64, "jaar": 2024, "ondertekend_op": "2024-01-09" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // En de melding noemt de actie zoals ze heet. Het actie-id draagt de actor
    // al, dus wie er nog een keer een actor voor zet, stuurt de lezer op zoek
    // naar een actie 'burger.burger.aanvraag' die niet bestaat.
    let error = body["error"].as_str().expect("een fout heeft een melding");
    assert!(error.contains("actie 'burger.aanvraag'"), "{error}");
    assert!(!error.contains("burger.burger"), "{error}");
}

/// De server geeft een icoon op `/favicon.ico`, het pad waar een browser uit
/// zichzelf om vraagt. Zonder dat kreeg hij de SPA-fallback: `index.html` met
/// een 404 eronder, als plaatje aangeboden.
#[tokio::test]
async fn favicon_ico_geeft_het_icoon_en_niet_de_pagina() {
    const ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"></svg>"#;
    const PAGE: &str = "<!doctype html><title>Testopstelling</title>";

    let bundle = tempfile::tempdir().expect("een tijdelijke map moet te maken zijn");
    std::fs::write(bundle.path().join("index.html"), PAGE)
        .expect("index.html moet te schrijven zijn");
    std::fs::write(bundle.path().join("favicon.svg"), ICON)
        .expect("het icoon moet te schrijven zijn");
    let app = app_with_static(bundle.path().to_str().expect("een pad in UTF-8")).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon.ico")
                .body(Body::empty())
                .expect("verzoek moet te bouwen zijn"),
        )
        .await
        .expect("de app moet antwoorden");
    assert_eq!(response.status(), StatusCode::OK);
    // Het mediatype is dat van het bestand zelf en niet van de extensie in de
    // URL: daar gaat een browser op af.
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("image/svg+xml")
    );
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body moet te lezen zijn")
        .to_bytes();
    assert_eq!(String::from_utf8_lossy(&body), ICON);

    // En de fallback zelf blijft staan: een diepe link is geen bestand en krijgt
    // de app terug — met de 404 die `not_found_service` eraan geeft, want die
    // link bestaat als bestand inderdaad niet.
    let response = app
        .oneshot(
            Request::builder()
                .uri("/een/diepe/link")
                .body(Body::empty())
                .expect("verzoek moet te bouwen zijn"),
        )
        .await
        .expect("de app moet antwoorden");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body moet te lezen zijn")
        .to_bytes();
    assert_eq!(String::from_utf8_lossy(&body), PAGE);
}

/// De Nederlandse notatie in een datumveld: een 400 met een melding waar een
/// mens iets aan heeft, niet een 500 met de engine-tekst erin.
///
/// Dit is de fout die iemand maakt omdat de rest van het scherm datums als
/// dd-mm-jjjj toont. Dat ze hier stukloopt en niet drie lagen dieper is het punt
/// van `type: date`: de cel weigert wat ze niet beloofd heeft, en de laag
/// eromheen noemt dat een verzoekfout.
#[tokio::test]
async fn een_datum_in_de_nederlandse_notatie_is_een_verzoekfout() {
    let mut browser = Browser::new().await;

    let (status, body) = browser
        .post(
            "/api/actions/burger.aanvraag",
            json!({ "bsn": "999993653", "jaar": 2024, "ondertekend_op": "09-01-2024" }),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error = body["error"].as_str().expect("een fout heeft een melding");
    assert!(error.contains("ondertekend_op"), "{error}");
    assert!(error.contains("09-01-2024"), "{error}");
    assert!(
        error.contains("jjjj-mm-dd"),
        "de melding hoort de notatie te noemen die wél gelezen wordt: {error}"
    );

    // Een dag die niet bestaat heeft de juiste vorm en is evengoed geen datum.
    let (status, body) = browser
        .post(
            "/api/actions/burger.aanvraag",
            json!({ "bsn": "999993653", "jaar": 2024, "ondertekend_op": "2024-02-30" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // En een datum als getal is geen leesfout maar een waarde van een ander
    // soort; ook dat hoort de aanvrager terug te krijgen en niet de server.
    let (status, body) = browser
        .post(
            "/api/actions/burger.aanvraag",
            json!({ "bsn": "999993653", "jaar": 2024, "ondertekend_op": 20_240_109_i64 }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

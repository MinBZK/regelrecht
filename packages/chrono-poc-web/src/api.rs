//! De JSON-API en de router eromheen.
//!
//! Alles wat een client kan doen, doet hij met de wereld van zijn eigen sessie:
//! kijken (`GET /api/world`), er iets in doen (`POST /api/actions/{id}`), de tijd
//! laten lopen (`POST /api/advance`), een cel bevragen
//! (`GET /api/cells/{cel}/lexostatus/{naam}`), een instelling wijzigen
//! (`PUT /api/settings`) of opnieuw beginnen (`POST /api/reset`).
//!
//! **Veldnamen Engels, meldingen Nederlands.** De vorm van het antwoord is het
//! contract dat [`regelrecht_simulator::Snapshot`] al vastlegt, en dat is Engels;
//! elke fout die een mens leest komt uit de simulator, en die praat Nederlands.
//! Eén van de twee omzetten zou betekenen dat er ergens een woordenlijst
//! bijgehouden moet worden die niemand bijhoudt.
//!
//! **Wat geen fout is.** "Niets vastgesteld" is een antwoord met een reden en
//! komt als 200 terug — een cel die op het gevraagde moment geen feit had, is
//! niet stuk. Een actie die nu niet kan is een 409 met de uitleg van de wereld
//! erin: het verhaal is nog niet zover, en dat is een stand en geen vergissing.

use std::collections::BTreeMap;

use axum::extract::{Path, Query, Request, State};
use axum::middleware as axum_middleware;
use axum::routing::{get, post, put, MethodRouter};
use axum::{Json, Router};
use chrono::NaiveDate;
use regelrecht_simulator::{
    Events, Lexostatus, ParameterType, Snapshot, Value, Warning, World, WorldDefinition,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tower_sessions::{Expiry, Session, SessionManagerLayer};
use tower_sessions_memory_store::MemoryStore;

use crate::config::SESSION_TTL;
use crate::error::ApiError;
use crate::state::AppState;

/// Onder welke sleutel de sessie haar wereld-id bewaart.
///
/// Een eigen id en niet [`tower_sessions::Session::id`]: die is `None` tot de
/// sessie voor het eerst opgeslagen is, dus het eerste verzoek van een bezoeker
/// zou geen sleutel hebben — precies het verzoek waarin zijn wereld ontstaat.
const SESSION_KEY_WORLD: &str = "chrono_poc.world";

/// Bouw de hele applicatie: de routes, de sessies, de statische frontend.
///
/// Eén functie, gebruikt door `main` én door de tests. Een tweede plek die
/// routers samenstelt is een tweede plek waar een route buiten de rol-gate kan
/// vallen.
pub fn router(state: AppState) -> Router {
    let role = state.config.required_role;
    let secure_cookies = state.config.is_auth_enabled();
    let static_dir = state.config.static_dir.clone();

    // Elke route hieronder raakt de wereld van een sessie; er is er geen die
    // open hoort te staan. `/health` en `/auth/*` staan er daarom buiten, en
    // niet in deze lijst met een uitzondering erbij.
    let api = Router::new()
        .route("/api/world", get(world))
        .route("/api/actions/{action}", post(act))
        .route("/api/advance", post(advance))
        .route("/api/cells/{cell}/lexostatus/{name}", get(lexostatus))
        .route("/api/settings", put(settings))
        .route("/api/reset", post(reset))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            regelrecht_auth::require_role::<AppState>(role),
        ));

    // Geen `with_secure(true)` in de auth-uit-modus: die draait lokaal over
    // plain HTTP, en een secure cookie komt daar nooit terug. In productie staat
    // OIDC altijd aan.
    let sessions = SessionManagerLayer::new(MemoryStore::default())
        .with_expiry(Expiry::OnInactivity(time::Duration::seconds(
            i64::try_from(SESSION_TTL.as_secs()).unwrap_or(i64::MAX),
        )))
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_http_only(true)
        .with_secure(secure_cookies);

    let refresh_state = state.clone();
    let auth_enabled = state.config.is_auth_enabled();

    let app = Router::new()
        .route("/health", get(health))
        .merge(regelrecht_auth::auth_routes::<AppState>())
        .merge(api)
        .with_state(state);

    // Binnen de sessielaag (de sessie is geladen) en buiten de rol-gate (een
    // verse rollenlijst of een weggevallen login wordt door de gate gezien).
    // Alleen zinvol als er een IdP is om bij te vragen.
    let app = if auth_enabled {
        app.layer(axum_middleware::from_fn_with_state(
            refresh_state,
            regelrecht_auth::refresh_session_token::<AppState>,
        ))
    } else {
        app
    };

    app.layer(sessions)
        // Ná de sessielaag, dus de statische bestanden gaan er niet door: een
        // plaatje hoort geen sessie aan te maken. Zelfde reden als in editor-api.
        .fallback_service(static_service(&static_dir))
        .layer(axum_middleware::from_fn(
            regelrecht_auth::security_headers::security_headers(regelrecht_auth::EDITOR_CSP),
        ))
        .layer(TraceLayer::new_for_http())
}

/// Statische bestanden met SPA-fallback, zoals editor-api ze serveert.
///
/// `index.html` als not-found-service: een diepe link in de frontend is geen
/// bestand en hoort de app te openen, niet een 404. Zonder gebouwde frontend
/// bestaat de map niet en antwoordt dit een gewone 404 — een server zonder
/// frontend is nog steeds een werkende API.
///
/// Geen precompressie zoals in editor-api: die leunt op `.br`/`.gz`-varianten
/// die de bouwstap van díe frontend schrijft. Zodra hier hetzelfde gebeurt,
/// horen de vlaggen erbij.
fn static_service(static_dir: &str) -> MethodRouter {
    let index = ServeFile::new(std::path::Path::new(static_dir).join("index.html"));
    let files = ServeDir::new(static_dir).not_found_service(index);
    get(move |request: Request| {
        let files = files.clone();
        async move {
            // Het foutype van `ServeDir` is `Infallible` zodra er een
            // not-found-service staat, dus dit kan niet mislukken.
            files
                .oneshot(request)
                .await
                .unwrap_or_else(|e| match e {})
                .map(axum::body::Body::new)
        }
    })
}

/// Leeft dit proces? Zonder login, want een healthcheck heeft er geen.
///
/// Zegt niets over de werelden: die worden per sessie opgetuigd. Dat het over
/// die werelden niets hoeft te zeggen, komt doordat het opstarten er al één
/// gebouwd heeft (zie [`crate::sources::resolve`] en
/// [`crate::worlds::WorldRegistry::check_buildable`]) — een proces dat hier
/// antwoordt, heeft een wereld die te bouwen is.
async fn health() -> &'static str {
    "OK"
}

/// `GET /api/world` — het beeld van de wereld van deze sessie.
async fn world(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Snapshot>, ApiError> {
    let key = world_key(&session).await?;
    let snapshot = state
        .worlds
        .with_world(&key, |world| Ok(world.snapshot()))
        .await?;
    Ok(Json(snapshot))
}

/// `POST /api/actions/{action}` — voer een actie uit met de velden uit de body.
async fn act(
    State(state): State<AppState>,
    session: Session,
    Path(action): Path<String>,
    JsonBody(form): JsonBody<BTreeMap<String, Value>>,
) -> Result<Json<Step>, ApiError> {
    let key = world_key(&session).await?;
    let step = state
        .worlds
        .with_world(&key, move |world| {
            let events = world.act(&action, &form)?;
            Ok(Step::new(world, &events))
        })
        .await?;
    Ok(Json(step))
}

/// De body van `POST /api/advance`.
#[derive(Debug, Deserialize)]
pub struct AdvanceRequest {
    /// Tot welke dag de klok gezet wordt. Achteruit is een 409: een logische
    /// klok die terugloopt zou een feit kunnen laten ontstaan vóór het feit
    /// waarop het rust.
    pub until: NaiveDate,
}

/// `POST /api/advance` — zet de klok vooruit en laat de triggers onderweg afgaan.
async fn advance(
    State(state): State<AppState>,
    session: Session,
    JsonBody(body): JsonBody<AdvanceRequest>,
) -> Result<Json<Step>, ApiError> {
    let key = world_key(&session).await?;
    let step = state
        .worlds
        .with_world(&key, move |world| {
            let events = world.advance(body.until)?;
            Ok(Step::new(world, &events))
        })
        .await?;
    Ok(Json(step))
}

/// `GET /api/cells/{cell}/lexostatus/{name}` — één reductie, alleen lezen.
///
/// De parameters komen als gewone queryparameters binnen en worden omgezet naar
/// het type dat de lexostatus documenteert: een BSN is een string ook al ziet hij
/// uit als een getal, en een jaar is een getal ook al staat het in een URL. Zonder
/// die stap zou elke waarde tekst zijn en zou een kroniekfilter op een getal
/// nooit iets vinden.
///
/// `op_moment` is gereserveerd: geen waarde betekent "op de stand van de klok".
async fn lexostatus(
    State(state): State<AppState>,
    session: Session,
    Path((cell, name)): Path<(String, String)>,
    QueryParams(query): QueryParams,
) -> Result<Json<Lexostatus>, ApiError> {
    let key = world_key(&session).await?;
    let Question { params, op_moment } =
        Question::read(state.worlds.definition(), &cell, &name, &query)?;
    let answer = state
        .worlds
        .with_world(&key, move |world| {
            let moment = op_moment.unwrap_or_else(|| world.now());
            Ok(world.reduce(&cell, &name, &params, moment)?)
        })
        .await?;
    Ok(Json(answer))
}

/// `PUT /api/settings` — wijzig instellingen die nog niet vast staan.
async fn settings(
    State(state): State<AppState>,
    session: Session,
    JsonBody(changes): JsonBody<BTreeMap<String, Value>>,
) -> Result<Json<Snapshot>, ApiError> {
    let key = world_key(&session).await?;
    let snapshot = state
        .worlds
        .with_world(&key, move |world| {
            world.update_settings(&changes)?;
            Ok(world.snapshot())
        })
        .await?;
    Ok(Json(snapshot))
}

/// `POST /api/reset` — terug naar de startstand uit het wereldbestand.
async fn reset(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Snapshot>, ApiError> {
    let key = world_key(&session).await?;
    let snapshot = state
        .worlds
        .with_world(&key, |world| {
            world.reset()?;
            Ok(world.snapshot())
        })
        .await?;
    Ok(Json(snapshot))
}

/// De wereld-sleutel van deze sessie; maak hem aan als hij er nog niet is.
async fn world_key(session: &Session) -> Result<String, ApiError> {
    let existing: Option<String> = session
        .get(SESSION_KEY_WORLD)
        .await
        .map_err(|e| ApiError::internal(format!("kon de sessie niet lezen: {e}")))?;
    if let Some(key) = existing {
        return Ok(key);
    }
    let key = uuid::Uuid::new_v4().to_string();
    session
        .insert(SESSION_KEY_WORLD, &key)
        .await
        .map_err(|e| ApiError::internal(format!("kon de sessie niet opslaan: {e}")))?;
    Ok(key)
}

/// Wat één stap opleverde: de nieuwe stand, en wat er gebeurde.
#[derive(Debug, Serialize)]
pub struct Step {
    /// Het beeld van de wereld ná de stap.
    pub snapshot: Snapshot,
    /// Wat er tijdens de stap gebeurde.
    pub events: EventsView,
}

impl Step {
    fn new(world: &World, events: &Events) -> Self {
        Self {
            snapshot: world.snapshot(),
            events: EventsView::new(events),
        }
    }
}

/// Wat er tijdens één stap gebeurde.
///
/// Een **verwijzing** naar wat er in het beeld staat en geen tweede uitgave
/// ervan: elk gram dat hier genoemd wordt, ligt met al zijn velden in
/// `snapshot.cells`. Wat deze lijst toevoegt, is de volgorde en het feit dat het
/// nú gebeurde — zonder dat zou een client twee beelden moeten vergelijken om te
/// zien wat zijn eigen klik deed.
#[derive(Debug, Serialize)]
pub struct EventsView {
    /// De feiten die vastgelegd zijn, in volgorde.
    pub recordings: Vec<RecordingView>,
    /// De besluiten die genomen zijn, in volgorde.
    pub decisions: Vec<DecisionView>,
    /// De termijnen die onderweg verstreken zonder dat het feit er lag.
    pub warnings: Vec<Warning>,
}

impl EventsView {
    fn new(events: &Events) -> Self {
        Self {
            recordings: events
                .recordings
                .iter()
                .map(|fact| RecordingView {
                    cell: fact.cell.clone(),
                    chronicle: fact.chronicle.clone(),
                    gram: fact.event.name.clone(),
                    op_moment: fact.event.op_moment,
                })
                .collect(),
            decisions: events
                .decisions
                .iter()
                .map(|decision| DecisionView {
                    cell: decision.decretogram.cell.clone(),
                    besluit: decision.decretogram.besluit.clone(),
                    zaakkenmerk: decision.decretogram.zaakkenmerk.clone(),
                    op_moment: decision.decretogram.op_moment,
                    legal_character: decision.decretogram.legal_character.clone(),
                    crossings: decision.crossings.len(),
                })
                .collect(),
            warnings: events.warnings.clone(),
        }
    }
}

/// Eén vastgelegd feit, als verwijzing naar het gram in het beeld.
#[derive(Debug, Serialize)]
pub struct RecordingView {
    /// De cel waarin het gram landde.
    pub cell: String,
    /// De kroniekstroom.
    pub chronicle: String,
    /// Hoe het gram heet.
    pub gram: String,
    /// Het moment waarop dit feit in die cel feit werd.
    pub op_moment: NaiveDate,
}

/// Eén genomen besluit, als verwijzing naar het decretogram in het beeld.
#[derive(Debug, Serialize)]
pub struct DecisionView {
    /// De cel die besloot.
    pub cell: String,
    /// De besluit-definitie die uitgevoerd is.
    pub besluit: String,
    /// Waaronder deze zaak terug te vinden is.
    pub zaakkenmerk: String,
    /// Het moment waarop besloten is.
    pub op_moment: NaiveDate,
    /// Het rechtskarakter dat de regeling aan de uitkomst geeft.
    pub legal_character: Option<String>,
    /// Hoeveel contacten dit besluit over een celgrens nodig had. De contacten
    /// zelf staan in `snapshot.crossings`.
    pub crossings: usize,
}

/// Een vraag aan een cel, na omzetting naar wat de definitie documenteert.
#[derive(Debug)]
struct Question {
    params: BTreeMap<String, Value>,
    op_moment: Option<NaiveDate>,
}

impl Question {
    /// Lees de queryparameters tegen de lexostatus-definitie uit het
    /// wereldbestand.
    ///
    /// Dat de definitie hier gelezen wordt en niet de cel zelf, is met opzet: de
    /// cel geeft geen inkijk in haar definities (RFC-022 §4.1 — een consument
    /// vraagt een naam, hij inspecteert geen cel), terwijl het wereldbestand
    /// gewoon configuratie is die dit proces zelf inlas.
    fn read(
        definition: &WorldDefinition,
        cell: &str,
        name: &str,
        query: &BTreeMap<String, String>,
    ) -> Result<Self, ApiError> {
        let cell_config = definition
            .cells
            .iter()
            .find(|candidate| candidate.id == cell)
            .ok_or_else(|| {
                ApiError::not_found(format!(
                    "de wereld kent geen cel '{cell}' (wel: {})",
                    names(definition.cells.iter().map(|c| c.id.as_str()))
                ))
            })?;
        let lexostatus = cell_config
            .lexostatus_definitions
            .iter()
            .find(|candidate| candidate.name == name)
            .ok_or_else(|| {
                ApiError::not_found(format!(
                    "cel '{cell}' publiceert geen lexostatus '{name}' (wel: {})",
                    names(
                        cell_config
                            .lexostatus_definitions
                            .iter()
                            .map(|l| l.name.as_str())
                    )
                ))
            })?;

        if lexostatus
            .inputs
            .iter()
            .any(|input| input.name == OP_MOMENT)
        {
            return Err(ApiError::internal(format!(
                "lexostatus '{cell}.{name}' documenteert een parameter '{OP_MOMENT}', en die naam \
                 is in deze API het moment van de vraag. Noem de parameter anders in het \
                 wereldbestand."
            )));
        }

        let mut params = BTreeMap::new();
        let mut op_moment = None;
        for (key, raw) in query {
            if key == OP_MOMENT {
                op_moment = Some(raw.parse::<NaiveDate>().map_err(|e| {
                    ApiError::bad_request(format!(
                        "'{OP_MOMENT}={raw}' is geen datum (verwacht jjjj-mm-dd): {e}"
                    ))
                })?);
                continue;
            }
            let documented = lexostatus
                .inputs
                .iter()
                .find(|input| &input.name == key)
                .ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "lexostatus '{cell}.{name}' kent geen parameter '{key}' \
                         (gedocumenteerd: {})",
                        names(lexostatus.inputs.iter().map(|i| i.name.as_str()))
                    ))
                })?;
            params.insert(
                key.clone(),
                coerce(cell, name, key, raw, documented.value_type)?,
            );
        }

        Ok(Self { params, op_moment })
    }
}

/// De queryparameter die het moment van de vraag draagt.
const OP_MOMENT: &str = "op_moment";

/// Zet één queryparameter om naar het gedocumenteerde type.
///
/// Getallen en booleans gaan langs de JSON-lezer van [`Value`], zodat een `1` en
/// een `1.5` hier precies dezelfde waarden opleveren als in een wereldbestand of
/// in een body — één plek die getallen leest, en niet drie.
fn coerce(
    cell: &str,
    name: &str,
    parameter: &str,
    raw: &str,
    expected: ParameterType,
) -> Result<Value, ApiError> {
    let wrong = |wanted: &str| {
        ApiError::bad_request(format!(
            "parameter '{parameter}' van lexostatus '{cell}.{name}' is {wanted}, en '{raw}' is \
             dat niet"
        ))
    };
    match expected {
        // Altijd tekst, ook als het een getal lijkt: een BSN met een nul vooraan
        // is geen getal, en een BSN zonder nul vooraan is dat ook niet.
        ParameterType::String => Ok(Value::String(raw.to_string())),
        ParameterType::Number => serde_json::from_str::<Value>(raw)
            .ok()
            .filter(|value| matches!(value, Value::Int(_) | Value::Decimal(_)))
            .ok_or_else(|| wrong("een getal")),
        ParameterType::Boolean => serde_json::from_str::<bool>(raw)
            .map(Value::Bool)
            .map_err(|_| wrong("waar of niet waar")),
    }
}

/// Een komma-gescheiden opsomming, zoals de simulator ze in zijn meldingen zet.
fn names<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let joined = items.collect::<Vec<_>>().join(", ");
    if joined.is_empty() {
        "geen".to_string()
    } else {
        joined
    }
}

/// Een JSON-body die zijn afwijzing als [`ApiError`] geeft.
///
/// Zonder dit antwoordt axum op een onleesbare body met platte tekst, en dan is
/// er één soort fout die niet op de andere lijkt — precies de fout die een client
/// niet verwacht en dus niet uitpakt. Een **lege** body wordt gelezen als `{}`,
/// zodat een actie zonder velden geen body nodig heeft.
pub struct JsonBody<T>(pub T);

impl<S, T> axum::extract::FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::Bytes::from_request(request, state)
            .await
            .map_err(|e| {
                ApiError::bad_request(format!("kon de body van dit verzoek niet lezen: {e}"))
            })?;
        let json: &[u8] = if bytes.is_empty() { b"{}" } else { &bytes };
        serde_json::from_slice(json)
            .map(Self)
            .map_err(|e| ApiError::bad_request(format!("de body is geen geldige JSON: {e}")))
    }
}

/// Queryparameters die hun afwijzing als [`ApiError`] geven. Zelfde reden als
/// bij [`JsonBody`].
pub struct QueryParams(pub BTreeMap<String, String>);

impl<S> axum::extract::FromRequestParts<S> for QueryParams
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Query::<BTreeMap<String, String>>::from_request_parts(parts, state)
            .await
            .map(|Query(params)| Self(params))
            .map_err(|e| ApiError::bad_request(format!("kon de queryparameters niet lezen: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    fn definition() -> WorldDefinition {
        WorldDefinition::from_yaml(
            r"
clock:
  start: 2024-01-01
cells:
  - id: brp
    laws: []
    chronicles:
      - stream: relaties
        key: bsn
    lexostatus_definitions:
      - name: partnerschap
        inputs:
          - name: bsn
            type: string
          - name: jaar
            type: number
          - name: met_partner
            type: boolean
        outputs:
          - partnerschap_type
        reduction:
          chronicle: relaties
          key: bsn
          latest: true
",
        )
        .expect("de testwereld moet te lezen zijn")
    }

    fn query(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    /// Het hele punt van de omzetting: een BSN die uit een getal bestaat blijft
    /// tekst, en een jaar wordt een getal. Was alles tekst, dan zou een
    /// kroniekfilter op `jaar` nooit iets vinden; was alles een getal, dan zou
    /// een BSN met een nul vooraan stil van waarde veranderen.
    #[test]
    fn parameters_krijgen_het_type_dat_de_definitie_documenteert() {
        let vraag = Question::read(
            &definition(),
            "brp",
            "partnerschap",
            &query(&[
                ("bsn", "999993653"),
                ("jaar", "2024"),
                ("met_partner", "true"),
                ("op_moment", "2024-06-01"),
            ]),
        )
        .expect("deze vraag hoort te kloppen");

        assert_eq!(
            vraag.params.get("bsn"),
            Some(&Value::String("999993653".to_string()))
        );
        assert_eq!(vraag.params.get("jaar"), Some(&Value::Int(2024)));
        assert_eq!(vraag.params.get("met_partner"), Some(&Value::Bool(true)));
        assert_eq!(
            vraag.op_moment.map(|d| d.to_string()),
            Some("2024-06-01".to_string())
        );
    }

    /// Geen `op_moment` betekent "op de stand van de klok", en dat besluit valt
    /// bij de wereld — hier hoort alleen `None` uit te komen.
    #[test]
    fn zonder_op_moment_blijft_het_moment_open() {
        let vraag = Question::read(
            &definition(),
            "brp",
            "partnerschap",
            &query(&[("bsn", "999993653")]),
        )
        .expect("deze vraag hoort te kloppen");
        assert_eq!(vraag.op_moment, None);
    }

    /// Een onbekende cel of lexostatus is een 404 met de namen die er wél zijn.
    /// Een parameter die de definitie niet documenteert is een 400, om dezelfde
    /// reden als bij de cel zelf: wat niet belooft is, wordt niet stil genegeerd.
    #[test]
    fn onbekende_namen_worden_geweigerd_met_de_bekende_ernaast() {
        let definition = definition();

        let cel = Question::read(&definition, "kiesraad", "partnerschap", &query(&[]))
            .expect_err("een onbekende cel hoort te falen");
        assert_eq!(cel.status(), StatusCode::NOT_FOUND);
        assert!(cel.message().contains("brp"), "{}", cel.message());

        let lexostatus = Question::read(&definition, "brp", "zetelverdeling", &query(&[]))
            .expect_err("een onbekende lexostatus hoort te falen");
        assert_eq!(lexostatus.status(), StatusCode::NOT_FOUND);
        assert!(
            lexostatus.message().contains("partnerschap"),
            "{}",
            lexostatus.message()
        );

        let parameter = Question::read(
            &definition,
            "brp",
            "partnerschap",
            &query(&[("burgerservicenummer", "999993653")]),
        )
        .expect_err("een ongedocumenteerde parameter hoort te falen");
        assert_eq!(parameter.status(), StatusCode::BAD_REQUEST);
        assert!(
            parameter.message().contains("bsn"),
            "{}",
            parameter.message()
        );
    }

    /// Een waarde die niet bij het gedocumenteerde type past, en een moment dat
    /// geen datum is: beide een 400 met de waarde erin, niet een reductie die op
    /// iets anders rekent dan de aanroeper bedoelde.
    #[test]
    fn een_waarde_van_het_verkeerde_type_wordt_geweigerd() {
        let definition = definition();

        for (parameter, waarde) in [("jaar", "vorig jaar"), ("met_partner", "misschien")] {
            let err = Question::read(
                &definition,
                "brp",
                "partnerschap",
                &query(&[(parameter, waarde)]),
            )
            .expect_err("een waarde van het verkeerde type hoort te falen");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);
            assert!(err.message().contains(waarde), "{}", err.message());
        }

        let err = Question::read(
            &definition,
            "brp",
            "partnerschap",
            &query(&[("op_moment", "gisteren")]),
        )
        .expect_err("een onleesbaar moment hoort te falen");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
        assert!(err.message().contains("jjjj-mm-dd"), "{}", err.message());
    }
}

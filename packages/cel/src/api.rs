//! De routes van een cel. De runtime biedt ze aan onder `/cellen/<id>`
//! (zie [`crate::runtime`]).
//!
//! Elke cel:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/kroniek` | de grammen, elk met YAML |
//! | `GET /api/lexostatus/{naam}?<input>=...` | een reductie, met de inputs als query |
//!
//! Een cel met een portaal heeft daarnaast:
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/eherkenning/login` | `{kvk, persoon, machtiging}` naar een sessie |
//! | `GET /api/eherkenning/sessie` | wie is ingelogd |
//! | `POST /api/eherkenning/logout` | sessie beeindigen |
//! | `GET /api/stroom` | de stroomdefinitie en de velden van het formulier |
//! | `POST /api/aanvraag/toets` | concept naar gram in het geheugen, reductie, synthese, engine |
//! | `POST /api/aanvraag` | het gram vastleggen in de kroniek |
//!
//! Bij een cel met een portaal zijn kroniek en lexostatus alleen voor de
//! ingelogde KvK, en alleen over diens eigen grammen.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::cel::Cel;
use crate::eherkenning::{Login, Sessie, Sessies, COOKIE};
use crate::kroniek::Kroniek;
use crate::reductie;
use crate::stroom::{self, Binding, Gram, Indiening};
use crate::synthese::{self, Bron};
use crate::toets;

/// Levert het moment waarop iets tot feit wordt gemaakt.
pub type Klok = Arc<dyn Fn() -> DateTime<FixedOffset> + Send + Sync>;

/// De klok van de cel: nu, in Nederlandse tijd.
pub fn systeemklok() -> Klok {
    Arc::new(|| {
        chrono::Utc::now()
            .with_timezone(&chrono_tz::Europe::Amsterdam)
            .fixed_offset()
    })
}

/// De toestand van een cel in de runtime.
#[derive(Clone)]
pub struct AppState {
    pub cel: Arc<Cel>,
    pub kroniek: Arc<Kroniek>,
    pub sessies: Arc<Sessies>,
    pub klok: Klok,
    /// De synthese-bronnen, met het transport dat de runtime koos.
    pub bronnen: Arc<Vec<Bron>>,
}

/// De routes van een cel, relatief aan `/cellen/<id>`.
pub fn router(state: AppState) -> Router {
    let mut r = Router::new()
        .route("/api/kroniek", get(kroniek_route))
        .route("/api/lexostatus/{naam}", get(lexostatus_route));
    if state.cel.portaal().is_some() {
        r = r
            .route("/api/eherkenning/login", post(login))
            .route("/api/eherkenning/sessie", get(sessie))
            .route("/api/eherkenning/logout", post(logout))
            .route("/api/stroom", get(stroom_route))
            .route("/api/aanvraag/toets", post(toets_route))
            .route("/api/aanvraag", post(indienen));
    }
    r.with_state(state)
}

/// Een fout als `{"fout": "..."}` met een status.
pub struct Fout(StatusCode, String);

impl IntoResponse for Fout {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"fout": self.1}))).into_response()
    }
}

fn fout(status: StatusCode, tekst: impl Into<String>) -> Fout {
    Fout(status, tekst.into())
}

fn ingelogd(state: &AppState, headers: &HeaderMap) -> Result<Sessie, Fout> {
    state
        .sessies
        .zoek(headers)
        .ok_or_else(|| fout(StatusCode::UNAUTHORIZED, "niet ingelogd"))
}

/// De sessie als de cel een portaal heeft; zonder portaal is er geen login
/// en ziet iedereen alles.
fn sessie_als_portaal(state: &AppState, headers: &HeaderMap) -> Result<Option<Sessie>, Fout> {
    if state.cel.portaal().is_some() {
        ingelogd(state, headers).map(Some)
    } else {
        Ok(None)
    }
}

/// De cookie geldt alleen onder het pad van deze cel.
fn cookie(state: &AppState, waarde: &str, extra: &str) -> String {
    format!(
        "{COOKIE}={waarde}; Path=/cellen/{}/; HttpOnly; SameSite=Strict{extra}",
        state.cel.id()
    )
}

async fn login(State(state): State<AppState>, Json(login): Json<Login>) -> Result<Response, Fout> {
    let sessie = login
        .valideer()
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let token = state.sessies.nieuw(sessie.clone());
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(sessie)).into_response())
}

async fn sessie(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Sessie>, Fout> {
    ingelogd(&state, &headers).map(Json)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    state.sessies.verwijder(&headers);
    let cookie = cookie(&state, "", "; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

async fn stroom_route(State(state): State<AppState>) -> Result<Json<Value>, Fout> {
    let (stroom, event) = portaal_event(&state)?;
    let velden = crate::formulier::velden(event, state.cel.formulier.as_ref());
    Ok(Json(json!({
        "stroom": stroom.document,
        "event": event.name,
        "titel": state.cel.formulier.as_ref().and_then(|f| f.titel.clone()),
        "velden": velden,
    })))
}

fn portaal_event(state: &AppState) -> Result<(&stroom::Stroom, &stroom::Event), Fout> {
    state.cel.portaal_event().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen portaal geconfigureerd",
        )
    })
}

#[derive(Deserialize)]
struct Concept {
    #[serde(default)]
    external: Map<String, Value>,
}

/// Bouw een gram uit een concept. Een indiening opent een nieuwe zaak.
fn bouw(state: &AppState, sessie: &Sessie, concept: &Concept) -> Result<Gram, Fout> {
    let (stroom, event) = portaal_event(state)?;
    let zaakkenmerk = uuid::Uuid::new_v4().to_string();
    let gram = stroom::bouw_gram(
        stroom,
        event,
        &Indiening {
            intake: &sessie.intake(&event.intake),
            external: &concept.external,
            op_moment: (state.klok)(),
            zaakkenmerk: &zaakkenmerk,
        },
    )
    .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    gram.valideer().map_err(|f| {
        fout(
            StatusCode::BAD_REQUEST,
            format!("gram valideert niet: {}", f.join("; ")),
        )
    })?;
    Ok(gram)
}

/// Het gram als YAML, velden in de volgorde van de stroom.
pub fn als_yaml(cel: &Cel, gram: &Gram) -> String {
    let mut doc = match serde_yaml_ng::to_value(gram) {
        Ok(serde_yaml_ng::Value::Mapping(m)) => m,
        _ => return String::new(),
    };
    let event = cel
        .strommen
        .iter()
        .find(|s| s.id == gram.stroom.id)
        .and_then(|s| s.event(&gram.name));
    if let Some(event) = event {
        doc.insert(
            serde_yaml_ng::Value::String("fields".into()),
            serde_yaml_ng::Value::Mapping(event.geordend(&gram.fields)),
        );
    }
    serde_yaml_ng::to_string(&doc).unwrap_or_default()
}

async fn toets_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<Json<Value>, Fout> {
    let sessie = ingelogd(&state, &headers)?;
    // Een concept is geen feit: het gram blijft in het geheugen.
    let gram = bouw(&state, &sessie, &concept)?;
    let portaal = state.cel.portaal().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen portaal geconfigureerd",
        )
    })?;
    let def = state
        .cel
        .lexostatussen
        .lexostatus(&portaal.toets.lexostatus)
        .ok_or_else(|| {
            fout(
                StatusCode::INTERNAL_SERVER_ERROR,
                "toets-lexostatus ontbreekt",
            )
        })?;
    let lexostatus =
        reductie::leid_af(def, &gram).map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    // Synthese: de eigen lexostatus plus die van de bronnen. Niets hiervan
    // wordt vastgelegd.
    let samen = synthese::voeg_samen(&lexostatus, &state.bronnen).await;
    let datum = gram.op_moment.get(..10).unwrap_or_default().to_string();
    let mut uitslag = toets::toets(
        &state.cel.service,
        &portaal.toets.regeling,
        &portaal.toets.uitkomst,
        &samen.parameters,
        reductie::ontbreekt(def, &lexostatus.parameters),
        &datum,
    );
    if !uitslag.te_beoordelen {
        if let Some(reden) = samen.reden() {
            uitslag.reden = Some(reden);
        }
    }
    Ok(Json(json!({
        "uitslag": uitslag,
        "lexostatus": lexostatus,
        // Wat naar de engine ging, en per parameter waar het vandaan kwam.
        "parameters": samen.parameters,
        "herkomst": samen.herkomst,
        "bronnen": samen.bronnen,
    })))
}

async fn indienen(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<(StatusCode, Json<Value>), Fout> {
    let sessie = ingelogd(&state, &headers)?;
    let gram = bouw(&state, &sessie, &concept)?;
    state
        .kroniek
        .voeg_toe(&gram)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    tracing::info!(cel = %state.cel.id(), zaakkenmerk = %gram.zaakkenmerk, name = %gram.name, "gram vastgelegd");
    let yaml = als_yaml(&state.cel, &gram);
    Ok((
        StatusCode::CREATED,
        Json(json!({"gram": gram, "yaml": yaml})),
    ))
}

/// Of een gram van deze KvK is: het veld dat aan `$intake.eherkenning.kvk`
/// bindt, heeft dat nummer.
fn van_kvk(cel: &Cel, gram: &Gram, kvk: &str) -> bool {
    let Some(event) = cel
        .strommen
        .iter()
        .find(|s| s.id == gram.stroom.id)
        .and_then(|s| s.event(&gram.name))
    else {
        return false;
    };
    event.bladeren().iter().any(|b| {
        b.binding == Binding::Intake("eherkenning.kvk".into())
            && gram.veld(&b.pad).and_then(Value::as_str) == Some(kvk)
    })
}

/// De grammen die deze vrager mag zien.
fn zichtbaar(state: &AppState, sessie: Option<&Sessie>, grammen: Vec<Gram>) -> Vec<Gram> {
    match sessie {
        Some(s) => grammen
            .into_iter()
            .filter(|g| van_kvk(&state.cel, g, &s.kvk))
            .collect(),
        None => grammen,
    }
}

async fn kroniek_route(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Fout> {
    let sessie = sessie_als_portaal(&state, &headers)?;
    let mut uit = Vec::new();
    for chronicle in state.cel.kronieken() {
        let grammen = state
            .kroniek
            .lees(chronicle)
            .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        for gram in zichtbaar(&state, sessie.as_ref(), grammen) {
            let yaml = als_yaml(&state.cel, &gram);
            uit.push(json!({"gram": gram, "yaml": yaml}));
        }
    }
    Ok(Json(Value::Array(uit)))
}

async fn lexostatus_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(naam): Path<String>,
    Query(inputs): Query<Map<String, Value>>,
) -> Result<Json<reductie::Lexostatus>, Fout> {
    let sessie = sessie_als_portaal(&state, &headers)?;
    let def = state
        .cel
        .lexostatussen
        .lexostatus(&naam)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, format!("geen lexostatus '{naam}'")))?;
    for i in &def.inputs {
        if !inputs.get(&i.name).is_some_and(reductie::gevuld) {
            return Err(fout(
                StatusCode::BAD_REQUEST,
                format!("input '{}' ontbreekt", i.name),
            ));
        }
    }
    let grammen = state
        .kroniek
        .lees(&def.reduction.kroniek)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let grammen = zichtbaar(&state, sessie.as_ref(), grammen);
    reductie::reduceer(def, &inputs, &grammen)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?
        .map(Json)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, "geen gram voor deze vraag"))
}

/// Wat `GET /api/cellen` over een cel zegt: wie ze is, of ze een portaal
/// heeft, welke lexostatussen ze aanbiedt en uit welke bronnen haar toets
/// samenvoegt.
pub fn beschrijving(state: &AppState) -> Value {
    let cel = &state.cel;
    let lexostatussen: Vec<Value> = cel
        .lexostatussen
        .lexostatus_definitions
        .iter()
        .map(|d| {
            json!({
                "name": d.name,
                "inputs": d.inputs,
                "parameters": d.reduction.afleidingen.keys().collect::<Vec<_>>(),
                "extra_velden": d.reduction.extra_velden.keys().collect::<Vec<_>>(),
            })
        })
        .collect();
    let synthese: Vec<Value> = state
        .bronnen
        .iter()
        .map(|b| {
            json!({
                "cel": b.definitie.cel,
                "lexostatus": b.definitie.lexostatus,
                "transport": b.transport.soort(),
                "parameters": b.definitie.parameters,
            })
        })
        .collect();
    json!({
        "id": cel.id(),
        "recording_actor": cel.definitie.recording_actor,
        "portaal": cel.portaal().is_some(),
        "titel": cel.formulier.as_ref().and_then(|f| f.titel.clone()),
        "kronieken": cel.kronieken(),
        "lexostatussen": lexostatussen,
        "synthese": synthese,
    })
}

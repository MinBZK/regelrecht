//! De routes van de cel.
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/eherkenning/login` | `{kvk, persoon, machtiging}` naar een sessie |
//! | `GET /api/eherkenning/sessie` | wie is ingelogd |
//! | `POST /api/eherkenning/logout` | sessie beeindigen |
//! | `GET /api/stroom` | de stroomdefinitie en de velden van het formulier |
//! | `POST /api/aanvraag/toets` | concept naar gram in het geheugen, reductie, engine |
//! | `POST /api/aanvraag` | het gram vastleggen in de kroniek |
//! | `GET /api/kroniek` | de grammen van de ingelogde KvK |
//! | `GET /api/lexostatus/{naam}?zaakkenmerk=...` | de reductie van een eigen zaak |

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::config::Cel;
use crate::eherkenning::{Login, Sessie, Sessies, COOKIE};
use crate::kroniek::Kroniek;
use crate::reductie;
use crate::stroom::{self, Binding, Gram, Indiening};
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

#[derive(Clone)]
pub struct AppState {
    pub cel: Arc<Cel>,
    pub kroniek: Arc<Kroniek>,
    pub sessies: Arc<Sessies>,
    pub klok: Klok,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/eherkenning/login", post(login))
        .route("/api/eherkenning/sessie", get(sessie))
        .route("/api/eherkenning/logout", post(logout))
        .route("/api/stroom", get(stroom_route))
        .route("/api/aanvraag/toets", post(toets_route))
        .route("/api/aanvraag", post(indienen))
        .route("/api/kroniek", get(kroniek_route))
        .route("/api/lexostatus/{naam}", get(lexostatus_route))
        .with_state(state)
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

async fn login(State(state): State<AppState>, Json(login): Json<Login>) -> Result<Response, Fout> {
    let sessie = login
        .valideer()
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let token = state.sessies.nieuw(sessie.clone());
    let cookie = format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict");
    Ok(([(header::SET_COOKIE, cookie)], Json(sessie)).into_response())
}

async fn sessie(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Sessie>, Fout> {
    ingelogd(&state, &headers).map(Json)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    state.sessies.verwijder(&headers);
    let cookie = format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
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
        .config
        .lexostatus(&portaal.toets.lexostatus)
        .ok_or_else(|| {
            fout(
                StatusCode::INTERNAL_SERVER_ERROR,
                "toets-lexostatus ontbreekt",
            )
        })?;
    let inputs = json!({"zaakkenmerk": gram.zaakkenmerk});
    let lexostatus = reductie::reduceer(
        def,
        inputs.as_object().unwrap_or(&Map::new()),
        std::slice::from_ref(&gram),
    )
    .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?
    .ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "de reductie vond het concept niet",
        )
    })?;
    let datum = gram.op_moment.get(..10).unwrap_or_default().to_string();
    let uitslag = toets::toets(
        &state.cel.service,
        &portaal.toets.regeling,
        &portaal.toets.uitkomst,
        &lexostatus.parameters,
        &datum,
    );
    Ok(Json(json!({"uitslag": uitslag, "lexostatus": lexostatus})))
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
    tracing::info!(zaakkenmerk = %gram.zaakkenmerk, name = %gram.name, "gram vastgelegd");
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

fn kronieken(cel: &Cel) -> Vec<&str> {
    let mut v: Vec<&str> = cel.strommen.iter().map(|s| s.chronicle.as_str()).collect();
    v.sort_unstable();
    v.dedup();
    v
}

async fn kroniek_route(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Fout> {
    let sessie = ingelogd(&state, &headers)?;
    let mut uit = Vec::new();
    for chronicle in kronieken(&state.cel) {
        let grammen = state
            .kroniek
            .lees(chronicle)
            .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        for gram in grammen
            .into_iter()
            .filter(|g| van_kvk(&state.cel, g, &sessie.kvk))
        {
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
    let sessie = ingelogd(&state, &headers)?;
    let def = state
        .cel
        .config
        .lexostatus(&naam)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, format!("geen lexostatus '{naam}'")))?;
    let grammen: Vec<Gram> = state
        .kroniek
        .lees(&def.reduction.kroniek)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .into_iter()
        .filter(|g| van_kvk(&state.cel, g, &sessie.kvk))
        .collect();
    reductie::reduceer(def, &inputs, &grammen)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?
        .map(Json)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, "geen gram voor deze vraag"))
}

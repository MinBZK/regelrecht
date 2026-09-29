//! Inloggen en uitloggen bij een proces, langs de kanalen van zijn rollen
//! (zie [`crate::kanaal`] en [`crate::sessie`]), en de toegang per
//! routegroep: een route vraagt een ingelogde gebruiker in een rol die die
//! groep mag gebruiken.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{Map, Value};

use super::{error, Error, ProcesState};
use crate::kanaal::{KanaalDefinitie, Routes, Sessie};
use crate::sessie::COOKIE;

fn gebruiker(state: &ProcesState, headers: &HeaderMap) -> Result<Sessie, Error> {
    state
        .sessies
        .zoek(headers)
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "niet ingelogd"))
}

/// De ingelogde gebruiker, als zijn rol routegroep `r` mag gebruiken; anders
/// 403 met de rollen die het wel mogen.
pub(super) fn met_routes(
    state: &ProcesState,
    headers: &HeaderMap,
    r: Routes,
) -> Result<Sessie, Error> {
    let s = gebruiker(state, headers)?;
    let d = &state.proces.definitie;
    if d.roles.get(&s.role).is_some_and(|role| role.mag(r)) {
        return Ok(s);
    }
    let wel: Vec<&str> = d.rollen_met(r).map(|(id, _)| id.as_str()).collect();
    Err(error(
        StatusCode::FORBIDDEN,
        format!(
            "alleen voor de rol {} (routes {}), en u bent ingelogd als {}",
            wel.join(" of "),
            r.als_tekst(),
            s.role
        ),
    ))
}

/// De ingelogde gebruiker van het portaal.
pub(super) fn ingelogd(state: &ProcesState, headers: &HeaderMap) -> Result<Sessie, Error> {
    met_routes(state, headers, Routes::Portal)
}

/// De ingelogde gebruiker van de behandeling.
pub(super) fn behandelaar(state: &ProcesState, headers: &HeaderMap) -> Result<Sessie, Error> {
    met_routes(state, headers, Routes::Handling)
}

/// De ingelogde gebruiker die een handeling mag doen: een rol die de
/// behandeling mag, en als de handeling een rol noemt, die rol.
pub(super) fn voor_handeling(
    state: &ProcesState,
    headers: &HeaderMap,
    role: Option<&str>,
) -> Result<Sessie, Error> {
    let s = behandelaar(state, headers)?;
    match role {
        Some(r) if s.role != r => Err(error(
            StatusCode::FORBIDDEN,
            format!("alleen voor de rol {r}, en u bent ingelogd als {}", s.role),
        )),
        _ => Ok(s),
    }
}

/// De cookie geldt alleen onder het pad van dit proces.
fn cookie(state: &ProcesState, value: &str, extra: &str) -> String {
    format!(
        "{COOKIE}={value}; Path=/processes/{}/; HttpOnly; SameSite=Strict{extra}",
        state.proces.id()
    )
}

fn channel<'a>(state: &'a ProcesState, id: &str) -> Result<&'a KanaalDefinitie, Error> {
    state
        .proces
        .definitie
        .channels
        .get(id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("geen kanaal '{id}'")))
}

/// De rol waarin iemand langs dit kanaal inlogt: de rol uit de invoer
/// (`rol`), of de enige rol van het kanaal.
fn rol_voor(
    state: &ProcesState,
    channel: &str,
    input: &Map<String, Value>,
) -> Result<String, Error> {
    let roles: Vec<&String> = state
        .proces
        .definitie
        .roles
        .iter()
        .filter(|(_, r)| r.channel == channel)
        .map(|(id, _)| id)
        .collect();
    match input.get("role").and_then(Value::as_str) {
        Some(r) if roles.iter().any(|x| *x == r) => Ok(r.to_string()),
        Some(r) => Err(error(
            StatusCode::BAD_REQUEST,
            format!("rol '{r}' logt niet in langs kanaal '{channel}'"),
        )),
        None => match roles.as_slice() {
            [een] => Ok((*een).clone()),
            [] => Err(error(
                StatusCode::NOT_FOUND,
                format!("geen rol logt in langs kanaal '{channel}'"),
            )),
            meer => Err(error(
                StatusCode::BAD_REQUEST,
                format!(
                    "kies een role: langs kanaal '{channel}' loggen {} in",
                    meer.iter()
                        .map(|r| r.as_str())
                        .collect::<Vec<_>>()
                        .join(" en ")
                ),
            )),
        },
    }
}

/// `POST /api/kanalen/{kanaal}/login`: de velden van het kanaal, en `rol`
/// als er langs het kanaal meer dan een rol inlogt.
pub(super) async fn login(
    State(state): State<ProcesState>,
    Path(id): Path<String>,
    Json(input): Json<Map<String, Value>>,
) -> Result<Response, Error> {
    let k = channel(&state, &id)?;
    let role = rol_voor(&state, &id, &input)?;
    let fields = k
        .valideer(&input)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let session = Sessie {
        role,
        channel: id,
        fields,
    };
    let token = state.sessies.nieuw(session.clone());
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(session)).into_response())
}

/// `GET /api/kanalen/{kanaal}/sessie`: wie langs dit kanaal is ingelogd; 401
/// zonder sessie, 403 bij een sessie langs een ander kanaal.
pub(super) async fn kanaalsessie(
    State(state): State<ProcesState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Sessie>, Error> {
    channel(&state, &id)?;
    let s = gebruiker(&state, &headers)?;
    if s.channel != id {
        return Err(error(
            StatusCode::FORBIDDEN,
            format!("ingelogd langs kanaal '{}', niet langs '{id}'", s.channel),
        ));
    }
    Ok(Json(s))
}

/// `GET /api/sessie`: wie er is ingelogd, langs welk kanaal ook.
pub(super) async fn session(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Sessie>, Error> {
    gebruiker(&state, &headers).map(Json)
}

pub(super) async fn logout(State(state): State<ProcesState>, headers: HeaderMap) -> Response {
    state.sessies.verwijder(&headers);
    let cookie = cookie(&state, "", "; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

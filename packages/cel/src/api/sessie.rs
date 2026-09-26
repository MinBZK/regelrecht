//! Inloggen en uitloggen bij een proces, langs de kanalen van zijn rollen
//! (zie [`crate::kanaal`] en [`crate::sessie`]), en de toegang per
//! routegroep: een route vraagt een ingelogde gebruiker in een rol die die
//! groep mag gebruiken.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{Map, Value};

use super::{fout, Fout, ProcesState};
use crate::kanaal::{KanaalDefinitie, Routes, Sessie};
use crate::sessie::COOKIE;

fn gebruiker(state: &ProcesState, headers: &HeaderMap) -> Result<Sessie, Fout> {
    state
        .sessies
        .zoek(headers)
        .ok_or_else(|| fout(StatusCode::UNAUTHORIZED, "niet ingelogd"))
}

/// De ingelogde gebruiker, als zijn rol routegroep `r` mag gebruiken; anders
/// 403 met de rollen die het wel mogen.
pub(super) fn met_routes(
    state: &ProcesState,
    headers: &HeaderMap,
    r: Routes,
) -> Result<Sessie, Fout> {
    let s = gebruiker(state, headers)?;
    let d = &state.proces.definitie;
    if d.rollen.get(&s.rol).is_some_and(|rol| rol.mag(r)) {
        return Ok(s);
    }
    let wel: Vec<&str> = d.rollen_met(r).map(|(id, _)| id.as_str()).collect();
    Err(fout(
        StatusCode::FORBIDDEN,
        format!(
            "alleen voor de rol {} (routes {}), en u bent ingelogd als {}",
            wel.join(" of "),
            r.als_tekst(),
            s.rol
        ),
    ))
}

/// De ingelogde gebruiker van het portaal.
pub(super) fn ingelogd(state: &ProcesState, headers: &HeaderMap) -> Result<Sessie, Fout> {
    met_routes(state, headers, Routes::Portaal)
}

/// De ingelogde gebruiker van de behandeling.
pub(super) fn behandelaar(state: &ProcesState, headers: &HeaderMap) -> Result<Sessie, Fout> {
    met_routes(state, headers, Routes::Behandeling)
}

/// De ingelogde gebruiker die een handeling mag doen: een rol die de
/// behandeling mag, en als de handeling een rol noemt, die rol.
pub(super) fn voor_handeling(
    state: &ProcesState,
    headers: &HeaderMap,
    rol: Option<&str>,
) -> Result<Sessie, Fout> {
    let s = behandelaar(state, headers)?;
    match rol {
        Some(r) if s.rol != r => Err(fout(
            StatusCode::FORBIDDEN,
            format!("alleen voor de rol {r}, en u bent ingelogd als {}", s.rol),
        )),
        _ => Ok(s),
    }
}

/// De cookie geldt alleen onder het pad van dit proces.
fn cookie(state: &ProcesState, waarde: &str, extra: &str) -> String {
    format!(
        "{COOKIE}={waarde}; Path=/processen/{}/; HttpOnly; SameSite=Strict{extra}",
        state.proces.id()
    )
}

fn kanaal<'a>(state: &'a ProcesState, id: &str) -> Result<&'a KanaalDefinitie, Fout> {
    state
        .proces
        .definitie
        .kanalen
        .get(id)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, format!("geen kanaal '{id}'")))
}

/// De rol waarin iemand langs dit kanaal inlogt: de rol uit de invoer
/// (`rol`), of de enige rol van het kanaal.
fn rol_voor(
    state: &ProcesState,
    kanaal: &str,
    invoer: &Map<String, Value>,
) -> Result<String, Fout> {
    let rollen: Vec<&String> = state
        .proces
        .definitie
        .rollen
        .iter()
        .filter(|(_, r)| r.kanaal == kanaal)
        .map(|(id, _)| id)
        .collect();
    match invoer.get("rol").and_then(Value::as_str) {
        Some(r) if rollen.iter().any(|x| *x == r) => Ok(r.to_string()),
        Some(r) => Err(fout(
            StatusCode::BAD_REQUEST,
            format!("rol '{r}' logt niet in langs kanaal '{kanaal}'"),
        )),
        None => match rollen.as_slice() {
            [een] => Ok((*een).clone()),
            [] => Err(fout(
                StatusCode::NOT_FOUND,
                format!("geen rol logt in langs kanaal '{kanaal}'"),
            )),
            meer => Err(fout(
                StatusCode::BAD_REQUEST,
                format!(
                    "kies een rol: langs kanaal '{kanaal}' loggen {} in",
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
    Json(invoer): Json<Map<String, Value>>,
) -> Result<Response, Fout> {
    let k = kanaal(&state, &id)?;
    let rol = rol_voor(&state, &id, &invoer)?;
    let velden = k
        .valideer(&invoer)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let sessie = Sessie {
        rol,
        kanaal: id,
        velden,
    };
    let token = state.sessies.nieuw(sessie.clone());
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(sessie)).into_response())
}

/// `GET /api/kanalen/{kanaal}/sessie`: wie langs dit kanaal is ingelogd; 401
/// zonder sessie, 403 bij een sessie langs een ander kanaal.
pub(super) async fn kanaalsessie(
    State(state): State<ProcesState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Sessie>, Fout> {
    kanaal(&state, &id)?;
    let s = gebruiker(&state, &headers)?;
    if s.kanaal != id {
        return Err(fout(
            StatusCode::FORBIDDEN,
            format!("ingelogd langs kanaal '{}', niet langs '{id}'", s.kanaal),
        ));
    }
    Ok(Json(s))
}

/// `GET /api/sessie`: wie er is ingelogd, langs welk kanaal ook.
pub(super) async fn sessie(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Sessie>, Fout> {
    gebruiker(&state, &headers).map(Json)
}

pub(super) async fn logout(State(state): State<ProcesState>, headers: HeaderMap) -> Response {
    state.sessies.verwijder(&headers);
    let cookie = cookie(&state, "", "; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

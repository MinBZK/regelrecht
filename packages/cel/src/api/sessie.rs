//! Inloggen en uitloggen bij een proces: de nep-eHerkenning van de
//! aanvrager en de nagebootste login van de behandelaar (zie
//! [`crate::sessie`]).

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use super::{fout, Fout, ProcesState};
use crate::eherkenning::{Login, Sessie};
use crate::sessie::{Gebruiker, Medewerker, COOKIE};

fn gebruiker(state: &ProcesState, headers: &HeaderMap) -> Result<Gebruiker, Fout> {
    state
        .sessies
        .zoek(headers)
        .ok_or_else(|| fout(StatusCode::UNAUTHORIZED, "niet ingelogd"))
}

/// De ingelogde aanvrager; een behandelaar mag hier niet.
pub(super) fn ingelogd(state: &ProcesState, headers: &HeaderMap) -> Result<Sessie, Fout> {
    gebruiker(state, headers)?
        .aanvrager()
        .cloned()
        .ok_or_else(|| fout(StatusCode::FORBIDDEN, "alleen voor de aanvrager"))
}

/// De ingelogde behandelaar; een aanvrager mag hier niet.
pub(super) fn behandelaar(state: &ProcesState, headers: &HeaderMap) -> Result<Medewerker, Fout> {
    gebruiker(state, headers)?
        .behandelaar()
        .cloned()
        .ok_or_else(|| fout(StatusCode::FORBIDDEN, "alleen voor de behandelaar"))
}

/// De cookie geldt alleen onder het pad van dit proces.
fn cookie(state: &ProcesState, waarde: &str, extra: &str) -> String {
    format!(
        "{COOKIE}={waarde}; Path=/processen/{}/; HttpOnly; SameSite=Strict{extra}",
        state.proces.id()
    )
}

pub(super) async fn login(
    State(state): State<ProcesState>,
    Json(login): Json<Login>,
) -> Result<Response, Fout> {
    let sessie = login
        .valideer()
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let token = state.sessies.nieuw(Gebruiker::Aanvrager(sessie.clone()));
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(sessie)).into_response())
}

pub(super) async fn sessie(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Sessie>, Fout> {
    ingelogd(&state, &headers).map(Json)
}

pub(super) async fn medewerker_login(
    State(state): State<ProcesState>,
    Json(login): Json<Medewerker>,
) -> Result<Response, Fout> {
    let m = login
        .valideer()
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let token = state.sessies.nieuw(Gebruiker::Behandelaar(m.clone()));
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(m)).into_response())
}

pub(super) async fn medewerker_sessie(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Medewerker>, Fout> {
    behandelaar(&state, &headers).map(Json)
}

pub(super) async fn logout(State(state): State<ProcesState>, headers: HeaderMap) -> Response {
    state.sessies.verwijder(&headers);
    let cookie = cookie(&state, "", "; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

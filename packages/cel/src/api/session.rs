//! Logging in to and out of a process, through the channels of its roles
//! (see [`crate::channel`] and [`crate::session`]), and the access per
//! route group: a route asks for a logged-in user in a role that may use
//! that group.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{Map, Value};

use super::{error, Error, ProcessState};
use crate::channel::{ChannelDefinition, Routes, Session};
use crate::session::COOKIE;

fn user(state: &ProcessState, headers: &HeaderMap) -> Result<Session, Error> {
    state
        .sessions
        .find(headers)
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "not logged in"))
}

/// The logged-in user, if their role may use route group `r`; otherwise
/// 403 with the roles that may.
pub(super) fn with_routes(
    state: &ProcessState,
    headers: &HeaderMap,
    r: Routes,
) -> Result<Session, Error> {
    let s = user(state, headers)?;
    let d = &state.process.definition;
    if d.roles.get(&s.role).is_some_and(|role| role.may(r)) {
        return Ok(s);
    }
    let permitted: Vec<&str> = d.roles_with(r).map(|(id, _)| id.as_str()).collect();
    Err(error(
        StatusCode::FORBIDDEN,
        format!(
            "only for the role {} (routes {}), and you are logged in as {}",
            permitted.join(" or "),
            r.as_text(),
            s.role
        ),
    ))
}

/// The logged-in user of the portal.
pub(super) fn logged_in(state: &ProcessState, headers: &HeaderMap) -> Result<Session, Error> {
    with_routes(state, headers, Routes::Portal)
}

/// The logged-in user of the handling.
pub(super) fn handler(state: &ProcessState, headers: &HeaderMap) -> Result<Session, Error> {
    with_routes(state, headers, Routes::Handling)
}

/// The logged-in user who may perform an action: a role that may do the
/// handling, and if the action names a role, that role.
pub(super) fn for_action(
    state: &ProcessState,
    headers: &HeaderMap,
    role: Option<&str>,
) -> Result<Session, Error> {
    let s = handler(state, headers)?;
    match role {
        Some(r) if s.role != r => Err(error(
            StatusCode::FORBIDDEN,
            format!("only for the role {r}, and you are logged in as {}", s.role),
        )),
        _ => Ok(s),
    }
}

/// The cookie is valid only under the path of this process.
fn cookie(state: &ProcessState, value: &str, extra: &str) -> String {
    format!(
        "{COOKIE}={value}; Path=/processes/{}/; HttpOnly; SameSite=Strict{extra}",
        state.process.id()
    )
}

fn channel<'a>(state: &'a ProcessState, id: &str) -> Result<&'a ChannelDefinition, Error> {
    state
        .process
        .definition
        .channels
        .get(id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("no channel '{id}'")))
}

/// The role in which someone logs in through this channel: the role from the
/// input (`role`), or the only role of the channel.
fn role_for(
    state: &ProcessState,
    channel: &str,
    input: &Map<String, Value>,
) -> Result<String, Error> {
    let roles: Vec<&String> = state
        .process
        .definition
        .roles
        .iter()
        .filter(|(_, r)| r.channel == channel)
        .map(|(id, _)| id)
        .collect();
    match input.get("role").and_then(Value::as_str) {
        Some(r) if roles.iter().any(|x| *x == r) => Ok(r.to_string()),
        Some(r) => Err(error(
            StatusCode::BAD_REQUEST,
            format!("role '{r}' does not log in through channel '{channel}'"),
        )),
        None => match roles.as_slice() {
            [a] => Ok((*a).clone()),
            [] => Err(error(
                StatusCode::NOT_FOUND,
                format!("no role logs in through channel '{channel}'"),
            )),
            more => Err(error(
                StatusCode::BAD_REQUEST,
                format!(
                    "choose a role: {} log in through channel '{channel}'",
                    more.iter()
                        .map(|r| r.as_str())
                        .collect::<Vec<_>>()
                        .join(" and ")
                ),
            )),
        },
    }
}

/// `POST /api/channels/{channel}/login`: the fields of the channel, and `role`
/// if more than one role logs in through the channel.
pub(super) async fn login(
    State(state): State<ProcessState>,
    Path(id): Path<String>,
    Json(input): Json<Map<String, Value>>,
) -> Result<Response, Error> {
    let k = channel(&state, &id)?;
    let role = role_for(&state, &id, &input)?;
    let fields = k
        .validate(&input)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let session = Session {
        role,
        channel: id,
        fields,
    };
    let token = state.sessions.open(session.clone());
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(session)).into_response())
}

/// `GET /api/channels/{channel}/session`: who is logged in through this
/// channel; 401 without a session, 403 for a session through another channel.
pub(super) async fn channel_session(
    State(state): State<ProcessState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Session>, Error> {
    channel(&state, &id)?;
    let s = user(&state, &headers)?;
    if s.channel != id {
        return Err(error(
            StatusCode::FORBIDDEN,
            format!(
                "logged in through channel '{}', not through '{id}'",
                s.channel
            ),
        ));
    }
    Ok(Json(s))
}

/// `GET /api/session`: who is logged in, through any channel.
pub(super) async fn session(
    State(state): State<ProcessState>,
    headers: HeaderMap,
) -> Result<Json<Session>, Error> {
    user(&state, &headers).map(Json)
}

pub(super) async fn logout(State(state): State<ProcessState>, headers: HeaderMap) -> Response {
    state.sessions.remove(&headers);
    let cookie = cookie(&state, "", "; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

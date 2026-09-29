//! The counter of a process: an application that came in another way
//! (on paper, at the desk), entered by an employee on behalf of the
//! applicant, with the day of receipt.
//!
//! Legally that day counts (Awb 4:1, 4:13: the decision deadline runs from
//! receipt), not the day of entry. The event of the portal therefore binds its
//! `effective_at` to a path under `$intake`; the counter fills that path, the
//! portal never does, so that an applicant cannot choose their own receipt.
//! The cell records as with the portal, with two times: `effective_at` the
//! receipt, `recorded_at` the entry.
//!
//! The counter refuses a receipt after today (what has yet to happen is not a
//! fact; the cell refuses it too), and a receipt before the opening of the
//! window, if the policy names one (`portal.offer.opening`).

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::session::with_routes;
use super::{error, internal, of_cell, Error, ProcessState};
use crate::cell_client::{self, RecordRequest, WithYaml};
use crate::channel::{self, Routes};
use crate::date::{self, TimePoint};
use crate::possibility::{self, Choice};

#[derive(Deserialize)]
pub(super) struct CounterInput {
    /// Who made the application: the fields of a portal channel, and `channel`
    /// if the portal has more than one. Nobody logged in: the counter
    /// copies what is on the application.
    applicant: Map<String, Value>,
    /// The day (`YYYY-MM-DD`) or the moment of receipt.
    received_at: String,
    #[serde(default)]
    external: Map<String, Value>,
}

/// `POST /api/counter/application`: the cell records the application in the
/// event of the portal, with the receipt as `effective_at`.
pub(super) async fn counter_submit(
    State(state): State<ProcessState>,
    headers: HeaderMap,
    Json(input): Json<CounterInput>,
) -> Result<(StatusCode, Json<WithYaml>), Error> {
    let who = with_routes(&state, &headers, Routes::Counter)?;
    let d = &state.process.definition;
    let (stream, event) = state
        .process
        .portal_event()
        .ok_or_else(|| internal("no portal configured"))?;
    let path = channel::receipt_path(event)
        .ok_or_else(|| internal("the portal event does not bind effective_at to $intake"))?;

    // The applicant, identified by the fields of a portal channel.
    let channels = d.channels_with(Routes::Portal);
    let requested = input.applicant.get("channel").and_then(Value::as_str);
    let (kid, k) = match (requested, channels.as_slice()) {
        (Some(g), _) => channels
            .iter()
            .find(|(id, _)| *id == g)
            .copied()
            .ok_or_else(|| {
                error(
                    StatusCode::BAD_REQUEST,
                    format!("applicant: '{g}' is not a channel of the portal"),
                )
            })?,
        (None, [a]) => *a,
        (None, _) => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!(
                    "applicant: name the channel ({})",
                    channels
                        .iter()
                        .map(|(id, _)| *id)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ))
        }
    };
    let fields = k
        .validate(&input.applicant)
        .map_err(|e| error(StatusCode::BAD_REQUEST, format!("applicant: {e}")))?;

    // The receipt: not after today, and not before the opening.
    let receipt = TimePoint::read("received_at", &input.received_at)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let now = (state.clock)();
    let day = receipt.as_moment(*now.offset()).date_naive();
    if day > now.date_naive() {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!(
                "received_at {} is after today ({}): a receipt that is yet to come is not entered",
                input.received_at,
                now.date_naive()
            ),
        ));
    }
    if let Some(open) = opening(&state, &input.external, &now)? {
        if day < open {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!(
                    "received_at {} is before the opening of the window ({open})",
                    input.received_at
                ),
            ));
        }
    }

    let mut intake = channel::intake("counter", channels.iter().copied(), Some((kid, &fields)));
    // What the channel of the applicant supplies (not the day of
    // submission: the application states its own date), and what a register
    // fills in beforehand.
    channel::supply(&mut intake, k, &fields, None);
    channel::prefill(
        &mut intake,
        event,
        &state.process.service,
        &now.date_naive().format("%Y-%m-%d").to_string(),
    );
    if let Value::Object(m) = &mut intake {
        crate::gram::set_path(m, &path, Value::String(input.received_at.clone()));
        m.insert(
            "ingevoerd_door".into(),
            json!({"role": who.role, "channel": who.channel, "identity": who.fields}),
        );
    }
    let request = RecordRequest {
        actor: d.actor.clone(),
        stream: stream.id.clone(),
        event: event.name.clone(),
        intake,
        external: input.external,
        refers_to: Default::default(),
        decision: None,
        root_grams: None,
    };
    let recorded = cell_client::record(state.cell.as_ref(), state.cell_id(), &request)
        .await
        .map_err(of_cell)?;
    Ok((StatusCode::CREATED, Json(recorded)))
}

/// The first day on which an application for the chosen window can come in,
/// if the policy names it (`offer.opening`, with the window as parameter).
/// The window comes from the field of the draft from which the assessment
/// derives it; without that field the opening cannot be assessed, and that is
/// an error of the input.
fn opening(
    state: &ProcessState,
    external: &Map<String, Value>,
    now: &chrono::DateTime<chrono::FixedOffset>,
) -> Result<Option<chrono::NaiveDate>, Error> {
    let Some(a) = state.process.portal().and_then(|p| p.offer.as_ref()) else {
        return Ok(None);
    };
    let Some(output) = &a.opening else {
        return Ok(None);
    };
    let window = state
        .process
        .window
        .as_ref()
        .ok_or_else(|| internal("offer.opening without window"))?;
    let value = window
        .field
        .as_ref()
        .and_then(|v| external.get(v))
        .filter(|w| !w.is_null())
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                format!(
                    "the window ({}) is absent; without a window the opening cannot be assessed",
                    window.field.as_deref().unwrap_or(&window.parameter)
                ),
            )
        })?;
    let choice = Choice {
        parameter: window.parameter.clone(),
        field: window.field.clone(),
        value: value.clone(),
    };
    possibility::start(
        &state.process.service,
        &a.regulation,
        output,
        &choice,
        &date::reference_date(now),
    )
    .map(Some)
    .map_err(internal)
}

//! The portal of a process: the form, the assessment, the offer and the
//! submission, for the logged-in applicant.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::session::logged_in;
use super::{error, internal, of_cell, Error, ProcessState};
use crate::assessment;
use crate::cell_client::{self, RecordRequest};
use crate::channel::{self, Routes, Session};
use crate::date::{self, TimePoint};
use crate::gram::Gram;
use crate::possibility;
use crate::reduction::{self, AsOf, Lexostatus};
use crate::rows;
use crate::stream;
use crate::synthesis;
use crate::transport::TransportError;

fn portal_event(state: &ProcessState) -> Result<(&stream::Stream, &stream::Event), Error> {
    state
        .process
        .portal_event()
        .ok_or_else(|| error(StatusCode::INTERNAL_SERVER_ERROR, "no portal configured"))
}

/// The fields of the application form: the `$external` fields of the event
/// in the stream of the cell, with labels and order from the form of the
/// process. For a logged-in applicant, what the channel supplies and what a
/// register fills in beforehand comes with each field (`supplied`): the
/// portal shows it as filled in automatically.
pub(super) async fn form_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let (stream, event) = portal_event(&state)?;
    let form = state.process.form.as_ref();
    let mut fields = crate::form::fields(event, form).map_err(internal)?;
    if let Ok(session) = logged_in(&state, &headers) {
        let intake = portal_intake(&state, event, &session);
        crate::form::with_supplied(&mut fields, &intake);
    }
    Ok(Json(json!({
        "cell": state.cell_id(),
        "stream": stream.document,
        "event": event.name,
        "title": form.and_then(|f| f.title.clone()),
        "fields": fields,
    })))
}

#[derive(Deserialize)]
pub(super) struct Concept {
    #[serde(default)]
    external: Map<String, Value>,
    /// Only for an event with references: per name the id of the gram the
    /// new gram refers to.
    #[serde(default)]
    refers_to: std::collections::BTreeMap<String, String>,
}

/// The owner path of the user's channel (`channels.<id>.owner`,
/// under `$intake`) and its value there. A channel without an owner makes
/// no one the owner.
fn owner_of<'s>(state: &ProcessState, session: &'s Session) -> Option<(String, &'s str)> {
    let k = state.process.definition.channels.get(&session.channel)?;
    let path = k.owner_path(&session.channel)?;
    let value = session.fields.get(k.owner.as_ref()?)?;
    Some((path, value.as_str()))
}

/// What the portal passes under `$intake` for the applicant: the channel and
/// its fields, what the channel supplies to the application (with the day of
/// submission) and what a register fills in beforehand.
fn portal_intake(state: &ProcessState, event: &stream::Event, session: &Session) -> Value {
    let mut intake = channel::intake(
        &event.intake,
        state.process.definition.channels_with(Routes::Portal),
        Some((&session.channel, &session.fields)),
    );
    let today = (state.clock)().date_naive();
    if let Some(k) = state.process.definition.channels.get(&session.channel) {
        channel::supply(&mut intake, k, &session.fields, Some(today));
    }
    channel::prefill(
        &mut intake,
        event,
        &state.process.service,
        &today.format("%Y-%m-%d").to_string(),
    );
    intake
}

/// The request to the cell for a draft of the applicant. If the event refers
/// to another gram, the applicant must know the group of that gram (the gram
/// they refer to is then itself a root, such as the application). Whether
/// they do is said by the cell: its [`crate::reduction::CaseState`] derives
/// whether the group holds a gram whose field bound to their owner path has
/// their value. The process reads no grams for this. The cell checks the rest.
async fn request_for(
    state: &ProcessState,
    session: &Session,
    concept: &Concept,
) -> Result<RecordRequest, Error> {
    let (stream, event) = portal_event(state)?;
    for z in concept.refers_to.values() {
        let known = match owner_of(state, session) {
            None => false,
            Some((path, value)) => {
                // A case the cell does not know, the applicant does not know either.
                match cell_client::case_state(
                    state.cell.as_ref(),
                    state.cell_id(),
                    z,
                    Some((&path, value)),
                )
                .await
                {
                    Err(TransportError::Response { status: 404, .. }) => false,
                    otherwise => otherwise.map_err(of_cell)?.owner == Some(true),
                }
            }
        };
        if !known {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!("no root '{z}' in the chronicle that you know"),
            ));
        }
    }
    Ok(RecordRequest {
        actor: state.process.definition.actor.clone(),
        stream: stream.id.clone(),
        event: event.name.clone(),
        intake: portal_intake(state, event, session),
        external: concept.external.clone(),
        refers_to: concept.refers_to.clone(),
        decision: None,
        root_grams: None,
    })
}

/// A draft, reduced on trial in the cell to the assessment lexostatus and
/// combined with the sources. A draft is not a fact: none of this is
/// recorded. Shared by the assessment and the application possibilities.
struct DraftAssessment<'a> {
    portal: &'a crate::config::Portal,
    def: &'a reduction::LexostatusDefinition,
    gram: Gram,
    lexostatus: Lexostatus,
    combined: synthesis::Combination,
    /// What the synthesis per row of the assessment yielded.
    rows: Vec<rows::RowsOutcome>,
}

/// `as_of`: as of when the sources reduce their chronicle (see [`AsOf`]); the
/// cell reduces the draft itself as it would be recorded now.
async fn draft_assessment<'a>(
    state: &'a ProcessState,
    session: &Session,
    concept: &Concept,
    as_of: &AsOf,
) -> Result<DraftAssessment<'a>, Error> {
    let portal = state
        .process
        .portal()
        .ok_or_else(|| error(StatusCode::INTERNAL_SERVER_ERROR, "no portal configured"))?;
    let def = state
        .process
        .cell
        .lexostatuses
        .lexostatus(&portal.assessment.lexostatus)
        .ok_or_else(|| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "assessment lexostatus is absent",
            )
        })?;
    let request = request_for(state, session, concept).await?;
    let cell_client::TrialReduction { gram, lexostatus } = cell_client::trial(
        state.cell.as_ref(),
        state.cell_id(),
        &portal.assessment.lexostatus,
        &request,
        &serde_json::Map::new(),
    )
    .await
    .map_err(of_cell)?;
    // Synthesis: the lexostatus of the draft plus those of the sources, and
    // then the synthesis per row, before the engine. An input from the law
    // reads the regulation on the day of the draft, like the assessment.
    let mut combined = synthesis::combine(&lexostatus, &state.sources, as_of).await;
    let date = date::reference_date_of(&gram.effective_at).map_err(internal)?;
    let law = rows::Environment {
        service: &state.process.service,
        date: &date,
        as_of,
    };
    let rows = rows::apply(
        &state.assessment_rows,
        std::slice::from_ref(&lexostatus),
        &mut combined,
        law,
    )
    .await;
    Ok(DraftAssessment {
        portal,
        def,
        gram,
        lexostatus,
        combined,
        rows,
    })
}

pub(super) async fn assessment_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<Json<Value>, Error> {
    let session = logged_in(&state, &headers)?;
    // The sources are read as of today, the day on which the engine reads the law.
    let today = AsOf::at(TimePoint::Date((state.clock)().date_naive()));
    let c = draft_assessment(&state, &session, &concept, &today).await?;
    let date = date::reference_date_of(&c.gram.effective_at).map_err(internal)?;
    let mut result = assessment::assessment(
        &state.process.service,
        &c.portal.assessment.regulation,
        &c.portal.assessment.output,
        &c.combined.parameters,
        reduction::absent(c.def, &c.lexostatus.parameters),
        &date,
    );
    if !result.to_assess {
        if let Some(reason) = c.combined.reason() {
            result.reason = Some(reason);
        }
    }
    Ok(Json(json!({
        "result": result,
        "lexostatus": c.lexostatus,
        // What went to the engine, and per parameter where it came from.
        "parameters": c.combined.parameters,
        "provenance": c.combined.provenance,
        "sources": c.combined.sources,
        "rows": c.rows,
    })))
}

/// What the policy offers the logged-in person (`portal.offer`), per
/// window the policy offers (`offer.windows`, an output of the same
/// regulation, computed on today's date). The window is the parameter of the
/// offer article with origin BELANGHEBBENDE and legal basis Awb 4:2 lid 1;
/// the policy is executed on a draft with only that window, plus what the
/// channel and the synthesis know. Output and deadline come from a run,
/// with trace. A fact a source did not deliver makes the offer
/// undeterminable. Nothing is recorded.
///
/// The sources are read as of a moment per window (see [`as_of_for`]): for a
/// window that has yet to begin, on its first day, so that a fact taking
/// effect before that window (a removal as of January 1) counts and the
/// registers do not give today's state for a year that is still to come.
pub(super) async fn possibilities_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let session = logged_in(&state, &headers)?;
    let c0 = state
        .process
        .portal()
        .and_then(|p| p.offer.clone())
        .ok_or_else(|| error(StatusCode::INTERNAL_SERVER_ERROR, "no offer configured"))?;
    let now = (state.clock)();
    let date = date::reference_date(&now);
    // Without a window one run; with a window one run per window the policy
    // offers.
    let choices: Vec<Option<possibility::Choice>> = match (&state.process.window, &c0.windows) {
        (Some(t), Some(u)) => {
            possibility::windows(&state.process.service, &c0.regulation, u, &date)
                .map_err(internal)?
                .into_iter()
                .map(|w| {
                    Some(possibility::Choice {
                        parameter: t.parameter.clone(),
                        field: t.field.clone(),
                        value: w,
                    })
                })
                .collect()
        }
        _ => vec![None],
    };
    let mut out = Vec::new();
    for choice in choices {
        let mut external = Map::new();
        if let Some(k) = &choice {
            if let Some(field) = &k.field {
                external.insert(field.clone(), k.value.clone());
            }
        }
        let concept = Concept {
            external,
            refers_to: Default::default(),
        };
        let start = match (&choice, &c0.start) {
            (Some(k), Some(u)) => Some(
                possibility::start(&state.process.service, &c0.regulation, u, k, &date)
                    .map_err(internal)?,
            ),
            _ => None,
        };
        let as_of = as_of_for(&now, start);
        let mut c = draft_assessment(&state, &session, &concept, &as_of).await?;
        // If the assessment lexostatus does not derive the window, the choice
        // itself goes along.
        if let Some(k) = &choice {
            if !c.combined.parameters.contains_key(&k.parameter) {
                c.combined
                    .parameters
                    .insert(k.parameter.clone(), k.value.clone());
                c.combined
                    .provenance
                    .insert(k.parameter.clone(), synthesis::Provenance::Choice);
            }
        }
        let m = possibility::determine(
            &state.process.service,
            choice,
            &c0,
            &c.combined.parameters,
            &date,
        );
        out.push(json!({
            "possibility": m,
            "as_of": as_of.as_of.map(|t| t.to_string()),
            "parameters": c.combined.parameters,
            "provenance": c.combined.provenance,
            "sources": c.combined.sources,
        }));
    }
    Ok(Json(json!({
        "session": session,
        // The date of the runtime, so that the frontend does not judge
        // "expired" by the browser's clock.
        "date": date,
        "possibilities": out,
    })))
}

/// The as-of of the sources for an offer: the start of the chosen window if
/// it has yet to begin, otherwise today. The policy says the start
/// (`offer.start`, see [`possibility::start`]); without a start, or without a
/// window, the offer is read as of today.
fn as_of_for(
    now: &chrono::DateTime<chrono::FixedOffset>,
    start: Option<chrono::NaiveDate>,
) -> AsOf {
    let today = now.date_naive();
    AsOf::at(TimePoint::Date(
        start.filter(|b| *b > today).unwrap_or(today),
    ))
}

/// Submission: the cell builds the gram and records it.
pub(super) async fn submit(
    State(state): State<ProcessState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<(StatusCode, Json<cell_client::WithYaml>), Error> {
    let session = logged_in(&state, &headers)?;
    let request = request_for(&state, &session, &concept).await?;
    let recorded = cell_client::record(state.cell.as_ref(), state.cell_id(), &request)
        .await
        .map_err(of_cell)?;
    Ok((StatusCode::CREATED, Json(recorded)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// A window that has yet to begin is read as of its start; a start that
    /// has already passed, and no start, as of today.
    #[test]
    fn the_offer_is_read_as_of_the_start_of_an_upcoming_window() {
        let now = date::moment("2026-09-25T10:00:00+02:00").unwrap();
        let op = |b: Option<&str>| {
            let b = b.map(|b| chrono::NaiveDate::parse_from_str(b, "%Y-%m-%d").unwrap());
            as_of_for(&now, b).as_of.unwrap().to_string()
        };
        assert_eq!(op(Some("2027-01-01")), "2027-01-01");
        assert_eq!(op(Some("2026-01-01")), "2026-09-25");
        assert_eq!(op(None), "2026-09-25");
    }
}

//! The handling in a process: the worklist, all cases, a case and the
//! actions in it,
//! on trial and taken. One generic route for every action
//! (`cases/{root}/actions/{name}`), not a route per kind of decision: what an
//! action needs follows from the stage and the origin (see
//! [`crate::action`]).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use futures_util::future::join_all;
use serde_json::{json, Map, Value};

use super::session::{for_action, handler};
use super::{error, of_cell, Error, ProcessState};
use crate::action::{self, ActionInput, Environment, Refusal};
use crate::cell_client;
use crate::config::ActionDefinition;
use crate::reduction::AsOf;
use crate::reduction::CaseState;
use crate::rows::Rows;
use crate::synthesis::{self, Source};
use crate::transport::{Remember, Transport};
use chrono::{DateTime, FixedOffset};

fn handling(state: &ProcessState) -> Result<&crate::config::Handling, Error> {
    state
        .process
        .definition
        .handling
        .as_ref()
        .ok_or_else(|| error(StatusCode::INTERNAL_SERVER_ERROR, "no handling configured"))
}

/// The worklist: the list lexostatus of the undecided cases from the cell.
pub(super) async fn worklist_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    handler(&state, &headers)?;
    let w = &handling(&state)?.worklist;
    list(&state, w).await
}

/// All cases: the list lexostatus of every case from the cell, decided or
/// not.
pub(super) async fn cases_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    handler(&state, &headers)?;
    let c = &handling(&state)?.cases;
    list(&state, c).await
}

/// A list lexostatus of the handling, from the cell.
async fn list(
    state: &ProcessState,
    w: &crate::config::LexostatusReference,
) -> Result<Json<Value>, Error> {
    let v = state
        .cell
        .fetch(&synthesis::path(
            &w.cell,
            &w.lexostatus,
            &Map::new(),
            &AsOf::default(),
        ))
        .await
        .map_err(of_cell)?;
    Ok(Json(v))
}

/// A refusal as HTTP response. What the state of the case does not allow
/// (not takeable, already recorded, a different competent authority) is a 409.
fn refusal(w: Refusal) -> Error {
    match w {
        Refusal::Invalid(t) => error(StatusCode::BAD_REQUEST, t),
        Refusal::NotTakeable(t) | Refusal::Conflict(t) | Refusal::Unauthorized(t) => {
            error(StatusCode::CONFLICT, t)
        }
        Refusal::Cell(t) => error(StatusCode::INTERNAL_SERVER_ERROR, t),
    }
}

/// The grams of a case, as the cell gives them; a 404 if the cell does not
/// know the case. Only for inspection of the file: the process derives
/// nothing from them.
async fn case_grams(state: &ProcessState, root: &str) -> Result<Vec<cell_client::WithYaml>, Error> {
    cell_client::read_case(state.cell.as_ref(), state.cell_id(), root)
        .await
        .map_err(of_cell)
}

/// The state of a case, as the cell derives it; a 404 if the cell does not
/// know the case.
async fn case_state(state: &ProcessState, root: &str) -> Result<CaseState, Error> {
    cell_client::case_state(state.cell.as_ref(), state.cell_id(), root, None)
        .await
        .map_err(of_cell)
}

/// The action with this name, with its position; 404 if the process does
/// not know it.
fn action_with<'s>(
    state: &'s ProcessState,
    name: &str,
) -> Result<(usize, &'s ActionDefinition), Error> {
    handling(state)?
        .actions
        .iter()
        .enumerate()
        .find(|(_, h)| h.name == name)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("no action '{name}'")))
}

fn environment(state: &ProcessState, i: usize) -> Environment<'_> {
    let h = &state.actions[i];
    environment_with(
        state,
        state.cell.as_ref(),
        &h.sources,
        &h.rows,
        (state.clock)(),
    )
}

/// The environment of an action with a given transport to the cell and
/// given sources (as the case screen shares them).
fn environment_with<'a>(
    state: &'a ProcessState,
    cell: &'a dyn Transport,
    sources: &'a [Source],
    rows: &'a [Rows],
    nu: DateTime<FixedOffset>,
) -> Environment<'a> {
    Environment {
        process: &state.process,
        cell,
        sources,
        rows,
        regulations: &state.regulations,
        now: nu,
    }
}

/// The case: its grams (inspection of the file, without interpreting
/// them), the procedure of the case (the stages that belong to no decision,
/// such as the application), the decisions with per decision the stages that
/// are recorded and the legal protection that follows from them, and per
/// action whether it can be taken, which decision it acts on, its form and a
/// trial without form (for a payment: what is still to be paid). What the
/// process knows about the case comes from the state the cell derives.
pub(super) async fn case_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
    Path(root): Path<String>,
) -> Result<Json<Value>, Error> {
    handler(&state, &headers)?;
    let case = case_state(&state, &root).await?;
    let grams = case_grams(&state, &root).await?;
    let b = handling(&state)?;
    let empty = ActionInput::default();
    // The case context (the lexostatuses of the case, the synthesis and the
    // synthesis per row) is the same for every action on the same reference
    // date: the trials share a memory for what they read, so that the
    // process asks for each lexostatus and each source once. A fact that
    // counts as a draft on trial is reduced by the cell per action.
    let memory = Remember::default();
    let cell = memory.wrap(state.cell.clone());
    let shared: Vec<(Vec<Source>, Vec<Rows>)> = state
        .actions
        .iter()
        .map(|hs| {
            let sources = hs
                .sources
                .iter()
                .map(|b| b.along(|t| memory.wrap(t)))
                .collect();
            let rows = hs
                .rows
                .iter()
                .map(|r| Rows {
                    definition: r.definition.clone(),
                    sources: r
                        .sources
                        .iter()
                        .map(|b| b.along(|t| memory.wrap(t)))
                        .collect(),
                })
                .collect();
            (sources, rows)
        })
        .collect();
    let nu = (state.clock)();
    let trials = join_all(b.actions.iter().enumerate().map(|(i, h)| {
        let env = environment_with(&state, cell.as_ref(), &shared[i].0, &shared[i].1, nu);
        let status = action::state(&state.process, h, &case);
        let case = &case;
        let root = &root;
        let empty = &empty;
        async move {
            // A stage that is already recorded or not yet possible is not computed.
            let trial = if status.available {
                Some(action::trial(&env, h, root, case, empty).await)
            } else {
                None
            };
            (status, trial)
        }
    }))
    .await;
    let mut actions = Vec::new();
    for (h, (status, trial)) in b.actions.iter().zip(trials) {
        // The check at startup already read this; an error here is one
        // of the runtime.
        let required = action::required(&state.process.service, h)
            .map_err(|f| error(StatusCode::INTERNAL_SERVER_ERROR, f))?;
        let trial = match trial {
            None => Value::Null,
            Some(Ok(p)) => json!(p),
            Some(Err(w)) => json!({"error": refusal_text(&w)}),
        };
        let form: Vec<Value> = h
            .verdicts
            .iter()
            .map(|o| {
                let typing = required.get(&o.parameter).map(|b| &b.typing);
                let mut v = json!({
                    "name": o.parameter,
                    "label": o.label,
                    "type": typing.map(|t| action::field_kind(t.kind)),
                    "group": o.group,
                    "kind": "verdict",
                });
                if let Some(e) = typing.and_then(|t| t.unit.as_deref()) {
                    v["unit"] = json!(e);
                }
                v
            })
            .chain(h.facts.iter().map(|f| {
                let mut v = json!(f);
                v["kind"] = json!("fact");
                v
            }))
            .collect();
        actions.push(json!({
            "name": h.name,
            "label": h.label(),
            "role": h.role,
            "kind": h.kind,
            "stage": h.stage,
            "regulation": h.regulation,
            "article": h.article,
            "outputs": h.outputs,
            "assessments": h.assessments,
            "types": h.types,
            "hooks": h.hooks,
            "not_yet": h.not_yet,
            "form": form,
            "available": status.available,
            "reason": status.reason,
            "recorded": status.recorded,
            "decision": status.decision,
            "decision_role": h.decision_role,
            "trial": trial,
        }));
    }
    Ok(Json(json!({
        "root": root,
        "grams": grams,
        "procedure": action::procedure_of_the_case(&state.process, &case),
        "decisions": action::decisions_in_case(&state.process, &case),
        "actions": actions,
    })))
}

fn refusal_text(w: &Refusal) -> String {
    match w {
        Refusal::Invalid(t)
        | Refusal::NotTakeable(t)
        | Refusal::Conflict(t)
        | Refusal::Unauthorized(t)
        | Refusal::Cell(t) => t.clone(),
    }
}

/// An action on trial: nothing is recorded.
pub(super) async fn trial_action_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
    Path((root, name)): Path<(String, String)>,
    Json(action_input): Json<ActionInput>,
) -> Result<Json<action::TrialAction>, Error> {
    let (i, h) = action_with(&state, &name)?;
    for_action(&state, &headers, h.role.as_deref())?;
    let case = case_state(&state, &root).await?;
    action::trial(&environment(&state, i), h, &root, &case, &action_input)
        .await
        .map(Json)
        .map_err(refusal)
}

/// Take an action and have it recorded (201), or a refusal (409):
/// not takeable, the stage is already in the case, the case changed, or the
/// law designates a different competent authority. With `happened: true` the
/// handler reports a fact that happened while the trial said no on the
/// content; the cell then records it (see [`action::take`]).
pub(super) async fn action_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
    Path((root, name)): Path<(String, String)>,
    Json(action_input): Json<ActionInput>,
) -> Result<(StatusCode, Json<action::Taken>), Error> {
    let (i, h) = action_with(&state, &name)?;
    let who = for_action(&state, &headers, h.role.as_deref())?;
    let case = case_state(&state, &root).await?;
    let taken = action::take(
        &environment(&state, i),
        h,
        &root,
        &case,
        &action_input,
        &who,
    )
    .await
    .map_err(refusal)?;
    tracing::info!(
        process = %state.process.id(),
        root = %root,
        action = %h.name,
        stage = taken.gram.stage.as_deref().unwrap_or("-"),
        "action recorded"
    );
    Ok((StatusCode::CREATED, Json(taken)))
}

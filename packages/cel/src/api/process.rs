//! The state and the routes of a process, relative to `/processes/<id>`;
//! see the table in [`crate::api`]. The handlers are in [`super::session`],
//! [`super::portal`], [`super::handling`] and [`super::fragment`].

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::counter::counter_submit;
use super::fragment;
use super::handling::{action_route, case_route, trial_action_route, worklist_route};
use super::inspection;
use super::portal::{assessment_route, form_route, possibilities_route, submit};
use super::session::{channel_session, login, logout, session};
use super::Clock;
use crate::channel::Routes;
use crate::gram::LoadedRegulation;
use crate::process::Process;
use crate::rows::Rows;
use crate::session::Sessions;
use crate::synthesis::Source;
use crate::transport::Transport;

/// The state of a process in the runtime.
#[derive(Clone)]
pub struct ProcessState {
    pub process: Arc<Process>,
    /// The transport to the cell in which the process records (internal).
    pub cell: Arc<dyn Transport>,
    pub sessions: Arc<Sessions>,
    pub clock: Clock,
    /// The synthesis sources that are not a lexostatus of the case, with the
    /// transport the runtime chose.
    pub sources: Arc<Vec<Source>>,
    /// Per action (in the order of `process.yaml`) the sources its article
    /// asks for and its synthesis per row.
    pub actions: Arc<Vec<ActionState>>,
    /// The synthesis per row of the assessment, with its sources.
    pub assessment_rows: Arc<Vec<Rows>>,
    /// The loaded regulations, for the receipt of a decision.
    pub regulations: Arc<Vec<LoadedRegulation>>,
    /// The corpus root (the parent of `REGULATION_PATH`): the fragment routes
    /// name files relative to it.
    pub root: Arc<PathBuf>,
    /// The file of every loaded regulation, per `(id, version_key)` (see
    /// [`crate::regulations::version_key`]).
    pub regulation_files: Arc<BTreeMap<(String, String), PathBuf>>,
}

/// What the runtime prepares per action: the synthesis sources its
/// article asks for (see [`crate::action::sources_for`]) and its synthesis
/// per row, with the transport the runtime chose.
#[derive(Clone)]
pub struct ActionState {
    pub sources: Vec<Source>,
    pub rows: Vec<Rows>,
}

impl ProcessState {
    pub(super) fn cell_id(&self) -> &str {
        self.process.cell.id()
    }
}

/// The routes of a process, relative to `/processes/<id>`. A process with
/// roles has the routes of its channels; the route groups are there if the
/// process has them, and every route checks whether the role of the
/// logged-in user may use that group.
pub fn process_router(state: ProcessState) -> Router {
    let mut r = Router::new()
        .route("/api/examples", get(examples_route))
        .route("/api/law/{regulation}/{article}", get(fragment::law_route))
        .route("/api/config/{*config}", get(fragment::config_route));
    let d = &state.process.definition;
    if !d.roles.is_empty() {
        r = r
            .route("/api/channels/{channel}/login", post(login))
            .route("/api/channels/{channel}/session", get(channel_session))
            .route("/api/channels/{channel}/logout", post(logout))
            .route("/api/session", get(session));
    }
    if state.process.portal().is_some() {
        r = r
            .route("/api/form", get(form_route))
            .route("/api/application/assessment", post(assessment_route))
            .route("/api/application", post(submit))
            .route("/api/possibilities", get(possibilities_route));
    }
    if d.roles_with(Routes::Counter).next().is_some() {
        r = r.route("/api/counter/application", post(counter_submit));
    }
    if d.handling.is_some() {
        r = r
            .route(
                "/api/inspection/{cell}/chronicle",
                get(inspection::chronicle_route),
            )
            .route(
                "/api/inspection/{cell}/lexostatus/{name}",
                get(inspection::lexostatus_route),
            )
            .route("/api/worklist", get(worklist_route))
            .route("/api/cases/{root}", get(case_route))
            .route("/api/cases/{root}/actions/{name}", post(action_route))
            .route(
                "/api/cases/{root}/actions/{name}/trial",
                post(trial_action_route),
            );
    }
    r.with_state(state)
}

/// The examples of the process. Also without login: the login examples are
/// there precisely for logging in.
async fn examples_route(State(state): State<ProcessState>) -> Json<crate::examples::Examples> {
    Json(
        state
            .process
            .examples
            .on(&crate::date::reference_date(&(state.clock)())),
    )
}

/// What `GET /api/processes` says about a process: who acts, in which cell,
/// with which roles, whether there is a portal and a handling, and from which
/// sources the synthesis combines.
pub fn process_description(state: &ProcessState) -> Value {
    let p = &state.process;
    let d = &p.definition;
    let synthesis: Vec<Value> = d
        .case_sources()
        .map(|b| {
            json!({
                "cell": b.cell,
                "lexostatus": b.lexostatus,
                "case": true,
                "transport": state.cell.kind(),
                "parameters": Vec::<String>::new(),
            })
        })
        .chain(state.sources.iter().map(|b| {
            json!({
                "cell": b.definition.cell,
                "lexostatus": b.definition.lexostatus,
                "case": false,
                "transport": b.transport.kind(),
                "parameters": b.definition.parameters,
                // The source speaks its own language; the consumer translates.
                "translation": b.definition.parameters.translated(),
            })
        }))
        .collect();
    json!({
        "id": p.id(),
        "actor": d.actor,
        "cell": p.cell.id(),
        "portal": p.portal().is_some(),
        "authority": p.authority,
        "channels": d.channels.iter().map(|(id, k)| (id.clone(), json!({
            "label": k.label,
            "explanation": k.explanation,
            "fields": k.fields,
            "owner": k.owner,
        }))).collect::<serde_json::Map<String, Value>>(),
        "roles": d.roles.iter().map(|(id, r)| (id.clone(), json!({
            "channel": r.channel,
            "routes": r.routes,
            "label": r.label.as_deref().unwrap_or(id),
        }))).collect::<serde_json::Map<String, Value>>(),
        "counter": d.roles_with(Routes::Counter).next().is_some(),
        "handling": d.handling.as_ref().map(|b| json!({
            "worklist": b.worklist.lexostatus,
            "actions": b.actions.iter().map(|h| json!({
                "name": h.name,
                "label": h.label(),
                "role": h.role,
                "kind": h.kind,
                "stage": h.stage,
                "regulation": h.regulation,
                "article": h.article,
                "outputs": h.outputs,
            })).collect::<Vec<_>>(),
        })),
        "title": p.form.as_ref().and_then(|f| f.title.clone()),
        "synthesis": synthesis,
        // The cells a handler may inspect through this process.
        "inspection": if d.handling.is_some() { state.inspection_cells().into_iter().collect() } else { Vec::new() },
    })
}

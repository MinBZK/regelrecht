//! The routes of a cell: record, store and reduce. Relative to
//! `/cells/<id>`; see the table in [`crate::api`].

use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{error, internal, Clock, Error};
use crate::cell::Cell;
use crate::cell_client::RecordRequest;
use crate::chronicle::{Chronicle, Recorded, View};
use crate::date;
use crate::gram::Gram;
use crate::lexostatus_engine;
use crate::reduction::{self, AsOf, Lexostatus, ReductionRoute};
use crate::stream::{self, Decision, Submission};
use crate::transport::{ReadToken, RuntimeToken, READ_TOKEN_HEADER, RUNTIME_TOKEN_HEADER};

/// The state of a cell in the runtime.
#[derive(Clone)]
pub struct CellState {
    pub cell: Arc<Cell>,
    pub chronicle: Arc<Chronicle>,
    pub clock: Clock,
    /// Whoever sends this token is a process of this runtime; only such a
    /// process may record or reduce on trial.
    pub runtime_token: RuntimeToken,
    /// Whoever sends this token may read (another runtime with the same
    /// `CELL_READ_TOKEN`). Without it only the own runtime reads.
    pub read_token: Option<ReadToken>,
}

/// The routes of a cell, relative to `/cells/<id>`. Recording and reducing on
/// trial ask for the runtime token (see [`only_the_runtime`]); reading (the
/// chronicle, a case, a lexostatus) the runtime token or the read token (see
/// [`only_readers`]), because the grams carry the identity and the intake of
/// whoever submitted. Only the stream definitions are open: they say nothing
/// about anyone.
pub fn cell_router(state: CellState) -> Router {
    let write = Router::new()
        .route("/api/lexostatus/{name}/trial", post(trial_route))
        .route("/api/grams", post(grams_route))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            only_the_runtime,
        ));
    let read = Router::new()
        .route("/api/chronicle", get(chronicle_route))
        .route("/api/cases/{root}", get(case_of_cell_route))
        .route("/api/lexostatus/{name}", get(lexostatus_route))
        .route_layer(middleware::from_fn_with_state(state.clone(), only_readers));
    Router::new()
        .route("/api/stream", get(stream_route))
        .merge(read)
        .merge(write)
        .with_state(state)
}

/// Whether a request carries a token that grants access: the runtime token,
/// or for reading also the read token. Without a token 401, with a wrong one 403.
fn access(state: &CellState, request: &Request, read: bool) -> Result<(), Error> {
    let h = request.headers();
    let runtime = h.get(RUNTIME_TOKEN_HEADER);
    let read_token = h.get(READ_TOKEN_HEADER);
    if runtime.is_some_and(|t| state.runtime_token.holds(t.as_bytes()))
        || read_token.filter(|_| read).is_some_and(|t| {
            state
                .read_token
                .as_ref()
                .is_some_and(|l| l.holds(t.as_bytes()))
        })
    {
        return Ok(());
    }
    let what = if read {
        "only a process of this runtime, or a runtime with the read token, reads a chronicle, a case or a lexostatus"
    } else {
        "only a process of this runtime records or reduces on trial"
    };
    let token = if read {
        "the runtime token or the read token"
    } else {
        "the runtime token"
    };
    Err(if runtime.is_none() && read_token.is_none() {
        error(
            StatusCode::UNAUTHORIZED,
            format!("{what}: {token} is absent"),
        )
    } else {
        error(
            StatusCode::FORBIDDEN,
            format!("{what}: {token} is not correct"),
        )
    })
}

/// Let a request through only if it carries the token of the runtime:
/// without a token 401, with a different token 403. That way only a process of
/// this runtime records, and not anyone who reaches the port.
async fn only_the_runtime(
    State(state): State<CellState>,
    request: Request,
    further: Next,
) -> Result<Response, Error> {
    access(&state, &request, false)?;
    Ok(further.run(request).await)
}

/// Let a read request through only with the runtime token or the read token.
/// A handler or administrator reads through a process (see
/// [`super::process`], the inspection), not directly.
async fn only_readers(
    State(state): State<CellState>,
    request: Request,
    further: Next,
) -> Result<Response, Error> {
    access(&state, &request, true)?;
    Ok(further.run(request).await)
}

/// Build a gram from a request, without recording it. The actor must be the
/// `recording_actor` of the stream. Whether the references exist and fit is
/// assessed by [`assessment_references`] under the lock; the cell also gives
/// the id there.
fn build(state: &CellState, v: &RecordRequest) -> Result<Gram, Error> {
    let (stream, event) = state.cell.event(&v.stream, &v.event).ok_or_else(|| {
        error(
            StatusCode::BAD_REQUEST,
            format!(
                "cell '{}' has no event '{}' in stream '{}'",
                state.cell.id(),
                v.event,
                v.stream
            ),
        )
    })?;
    if v.actor != stream.recording_actor {
        return Err(error(
            StatusCode::FORBIDDEN,
            format!(
                "actor '{}' does not record in stream '{}': the recording_actor is '{}'",
                v.actor, stream.id, stream.recording_actor
            ),
        ));
    }
    let mut gram = stream::build_gram(
        stream,
        event,
        &Submission {
            intake: &v.intake,
            external: &v.external,
            recorded_at: (state.clock)(),
            refers_to: &v.refers_to,
        },
    )
    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    if let Some(b) = &v.decision {
        gram.legal_character = b.legal_character.clone();
        gram.decision_type = b.decision_type.clone();
        gram.regulation = b.regulation.clone();
        gram.regulation_valid_from = b.regulation_valid_from.clone();
        gram.competent_authority = b.competent_authority.clone();
        gram.acting_actor = b.acting_actor.clone();
        gram.inputs = b.inputs.clone();
        gram.receipt = b.receipt.clone();
    }
    Ok(gram)
}

/// Validate a built gram against `gram.json`; a 400 if it does not fit.
fn validate(cell: &Cell, gram: &Gram) -> Result<(), Error> {
    gram.validate().map_err(|f| {
        error(
            StatusCode::BAD_REQUEST,
            format!("gram does not validate: {}", f.join("; ")),
        )
    })?;
    reduction::fields_in_order(&cell.lexostatuses.lexostatus_definitions, gram)
        .map_err(|f| error(StatusCode::BAD_REQUEST, f))
}

/// The gram as YAML, fields in the order of the stream.
pub fn as_yaml(cell: &Cell, gram: &Gram) -> Result<String, String> {
    let not = |e: String| format!("gram '{}' cannot be written as YAML: {e}", gram.name);
    let mut doc = match serde_yaml_ng::to_value(gram) {
        Ok(serde_yaml_ng::Value::Mapping(m)) => m,
        Ok(_) => return Err(not("not a mapping".into())),
        Err(e) => return Err(not(e.to_string())),
    };
    if let Some((_, event)) = cell.event(&gram.stream.id, &gram.name) {
        doc.insert(
            serde_yaml_ng::Value::String("fields".into()),
            serde_yaml_ng::Value::Mapping(event.ordered(&gram.fields)),
        );
    }
    serde_yaml_ng::to_string(&doc).map_err(|e| not(e.to_string()))
}

/// Whether a gram fits the grams it refers to, given what the cell sees
/// under its lock (`view`: the targets and the group of the root).
/// The cell enforces the shape, never the content: what an action is worth,
/// the process concludes before it acts. Which facts are recordable, the
/// paper leaves open (P:110); this boundary is our own choice (RFC-044 par. 1
/// and 4), per reference instead of per case.
///
/// - Every gram referred to is in the chronicles of the cell, fits what the
///   reference may point to (`to`: an article it establishes, an event or a
///   stage), and all targets have the same root.
/// - The gram does not legally precede a gram it refers to: the day of its
///   `effective_at` is not before that of the target (weaker than the old rule
///   "not before the last fact in the case": a payment on the advance may be
///   recorded later than the determination).
/// - A decision (stage BESLUIT, not an amendment) of the same event that
///   refers to the same gram exists at most once: another decision on the
///   same application requires its own legal basis (`amends`).
///   Another gram with a stage (such as the announcement) exists at most
///   once per stage and per target: a decision is announced once.
/// - If the request says how many grams the group had when the process read
///   it (`root_grams`), the cell only records if that is still so.
fn assessment_references(
    cell: &Cell,
    gram: &Gram,
    view: &View<'_>,
    expected: Option<usize>,
) -> Result<(), Error> {
    if let Some(f) = &view.root_error {
        return Err(error(StatusCode::BAD_REQUEST, f.clone()));
    }
    let event = cell.event(&gram.stream.id, &gram.name).map(|(_, e)| e);
    for (name, id) in &gram.refers_to {
        let Some(target) = view.targets.get(id) else {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!("no gram '{id}' in the chronicle ({name})"),
            ));
        };
        if let Some(v) = event.and_then(|e| e.refers_to.get(name)) {
            let target_event = cell.event(&target.stream.id, &target.name).map(|(_, e)| e);
            if !v.to.fits(target, target_event) {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "'{name}' points to {}, but gram {id} is '{}'",
                        v.to, target.name
                    ),
                ));
            }
        }
        not_for(gram, target)?;
    }
    // A gram without a reference is its own root: it has no group (yet)
    // to compare.
    if let Some(n) = expected.filter(|_| !gram.refers_to.is_empty()) {
        if view.group.len() != n {
            return Err(error(
                StatusCode::CONFLICT,
                format!(
                    "the group of root {} changed since the process read it ({n} grams, now {}); compute the action again",
                    gram.root.as_deref().unwrap_or("-"),
                    view.group.len()
                ),
            ));
        }
    }
    let Some(stage) = gram.stage.as_deref() else {
        return Ok(());
    };
    let role = event.and_then(|e| e.decision);
    // The same target under the same name: a different decision under
    // `decision` for the same `application` is a different fact.
    let shares = |g: &Gram| {
        g.refers_to
            .iter()
            .any(|(name, d)| gram.refers_to.get(name) == Some(d))
    };
    if role.is_some_and(Decision::is_decision) {
        if role == Some(Decision::Opens) {
            if let Some(earlier) = view
                .group
                .iter()
                .find(|g| g.name == gram.name && g.stream.id == gram.stream.id && shares(g))
            {
                return Err(error(
                    StatusCode::CONFLICT,
                    format!(
                        "there is already a decision '{}' ({}) that refers to the same gram; another decision on this requires its own legal basis, an event with an amends reference",
                        earlier.name, earlier.id
                    ),
                ));
            }
        }
        return Ok(());
    }
    if let Some(earlier) = view
        .group
        .iter()
        .find(|g| g.stage.as_deref() == Some(stage) && shares(g))
    {
        let target = earlier
            .refers_to
            .iter()
            .find(|(name, d)| gram.refers_to.get(*name) == Some(d))
            .map_or("-", |(_, d)| d.as_str());
        return Err(error(
            StatusCode::CONFLICT,
            format!(
                "for gram {target} there is already a gram with stage {stage} ('{}'); a decision passes through each stage once (RFC-022 par. 1.2)",
                earlier.name
            ),
        ));
    }
    Ok(())
}

/// A gram does not legally precede a gram it refers to: the day of its
/// `effective_at` is not before that of the target. It is about the day,
/// because a bound moment is often a date (the start of that day) and the
/// target may have been recorded later on the same day. An unbound
/// `effective_at` is the moment of recording and is therefore never before it.
fn not_for(gram: &Gram, target: &Gram) -> Result<(), Error> {
    let day = date::reference_date_of(&gram.effective_at)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let d = date::reference_date_of(&target.effective_at).map_err(internal)?;
    if day < d {
        return Err(error(
            StatusCode::CONFLICT,
            format!(
                "effective_at {day} is before the gram it refers to ('{}', {d}); what follows moves forward in time",
                target.name
            ),
        ));
    }
    Ok(())
}

/// Record a gram. Response: the gram, with YAML. The stamping
/// (`recorded_at`), the assessment against the case and the writing happen
/// under one lock: two concurrent requests do not both record the same stage,
/// and the order in the file is that of `recorded_at`. The writing waits on
/// the disk, so it runs outside the async threads.
async fn grams_route(
    State(state): State<CellState>,
    Json(request): Json<RecordRequest>,
) -> Result<(StatusCode, Json<Value>), Error> {
    let gram = build(&state, &request)?;
    let (chronicle, cell, clock) = (
        state.chronicle.clone(),
        state.cell.clone(),
        state.clock.clone(),
    );
    let expected = request.root_grams;
    let gram = tokio::task::spawn_blocking(move || {
        chronicle.record_provided(
            gram,
            &cell.chronicles(),
            || clock(),
            |f| error(StatusCode::BAD_REQUEST, f),
            |g, view| {
                assessment_references(&cell, g, view, expected)?;
                validate(&cell, g)
            },
        )
    })
    .await
    .map_err(|e| internal(format!("recording was aborted: {e}")))?
    .map_err(internal)??;
    tracing::info!(cell = %state.cell.id(), id = %gram.gram.id, root = gram.gram.root.as_deref().unwrap_or("-"), name = %gram.gram.name, "gram recorded");
    // The YAML goes into the recorded gram, so that a later read does not
    // make it again.
    let yaml = gram.yaml(|g| as_yaml(&state.cell, g)).map_err(internal)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"gram": gram.gram, "yaml": yaml})),
    ))
}

#[derive(Deserialize)]
struct TrialRequest {
    draft: RecordRequest,
    /// The inputs of the lexostatus, and the as-of if needed (`as_of`,
    /// `known_at`), as with `GET lexostatus`.
    #[serde(default)]
    inputs: Map<String, Value>,
}

/// A trial reduction: the gram of the draft in memory, the chronicle
/// reduced with that gram. Without input `root` the draft's root counts.
/// A draft is not a fact: nothing is recorded. The draft counts as
/// recorded at the clock's now; with an as-of the same holds for the draft
/// as for any other gram.
async fn trial_route(
    State(state): State<CellState>,
    Path(name): Path<String>,
    Json(request): Json<TrialRequest>,
) -> Result<Json<Value>, Error> {
    let def = lexostatus_def(&state, &name)?;
    let mut gram = build(&state, &request.draft)?;
    let view = state
        .chronicle
        .view_for(&state.cell.chronicles(), &mut gram)
        .map_err(internal)?;
    assessment_references(&state.cell, &gram, &view.view(), None)?;
    validate(&state.cell, &gram)?;
    let mut inputs = request.inputs;
    let as_of = AsOf::from_query(&mut inputs).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    if let Some(w) = &gram.root {
        if def.inputs.iter().any(|i| i.name == ROOT) && !inputs.contains_key(ROOT) {
            inputs.insert(ROOT.into(), Value::String(w.clone()));
        }
    }
    inputs_complete(def, &inputs)?;
    let chronicle = grams_for(&state, def, &inputs)?;
    // The draft last: with equal moments `pick: latest` chooses it. A draft
    // with an earlier effective_at (an earlier receipt) is not automatically
    // the latest.
    let grams = chronicle
        .iter()
        .map(|v| &v.gram)
        .chain(std::iter::once(&gram));
    let lexostatus = reduce(&state, def, &inputs, grams, &as_of, false)?;
    Ok(Json(json!({"gram": gram, "lexostatus": lexostatus})))
}

fn lexostatus_def<'s>(
    state: &'s CellState,
    name: &str,
) -> Result<&'s reduction::LexostatusDefinition, Error> {
    state
        .cell
        .lexostatuses
        .lexostatus(name)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("no lexostatus '{name}'")))
}

/// The name of the input and the filter key of the root.
pub const ROOT: &str = "root";

/// The grams a reduction reads: the chronicle of the definition, and if
/// its filter filters on the root of an input (and it is not a list),
/// only the grams of that root, from the index per root. The reduction
/// applies the filter itself afterwards; this only saves reading the
/// rest of the chronicle.
fn grams_for(
    state: &CellState,
    def: &reduction::LexostatusDefinition,
    inputs: &Map<String, Value>,
) -> Result<Vec<Arc<Recorded>>, Error> {
    let r = &def.reduction;
    let root = r
        .filter
        .get(ROOT)
        .filter(|_| r.group_by.is_none())
        .and_then(|v| match v.strip_prefix('$') {
            Some(input) => inputs.get(input).and_then(Value::as_str),
            None => Some(v.as_str()),
        });
    match root {
        Some(w) => state.chronicle.read_root(&[r.chronicle.as_str()], w),
        None => state.chronicle.read(&r.chronicle),
    }
    .map_err(internal)
}

fn inputs_complete(
    def: &reduction::LexostatusDefinition,
    inputs: &Map<String, Value>,
) -> Result<(), Error> {
    for i in &def.inputs {
        if !inputs.get(&i.name).is_some_and(reduction::filled) {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!("input '{}' is absent", i.name),
            ));
        }
    }
    Ok(())
}

/// The grams with their YAML. The YAML of a gram is made once and
/// kept afterwards.
fn with_yaml(state: &CellState, grams: &[Arc<Recorded>]) -> Result<Value, Error> {
    let mut out = Vec::with_capacity(grams.len());
    for v in grams {
        let yaml = v.yaml(|g| as_yaml(&state.cell, g)).map_err(internal)?;
        out.push(json!({"gram": v.gram, "yaml": yaml}));
    }
    Ok(Value::Array(out))
}

/// The chronicle of the cell: all grams, across all its chronicles.
async fn chronicle_route(State(state): State<CellState>) -> Result<Json<Value>, Error> {
    let grams = state
        .chronicle
        .all(&state.cell.chronicles())
        .map_err(internal)?;
    Ok(Json(with_yaml(&state, &grams)?))
}

/// The grams with one root, across all chronicles of the cell. The filtering
/// happens here, in the cell; a process gets only the group it asks for.
async fn case_of_cell_route(
    State(state): State<CellState>,
    Path(root): Path<String>,
) -> Result<Json<Value>, Error> {
    let grams = state
        .chronicle
        .read_root(&state.cell.chronicles(), &root)
        .map_err(internal)?;
    if grams.is_empty() {
        return Err(error(
            StatusCode::NOT_FOUND,
            format!("no root '{root}' in the chronicle"),
        ));
    }
    Ok(Json(with_yaml(&state, &grams)?))
}

/// A lexostatus: the chronicle reduced, with the inputs as query. With
/// `as_of` and/or `known_at` (a date or a moment) at an earlier moment:
/// see [`AsOf`]. The runtime itself offers the lexostatus
/// [`reduction::CASE_STATE`], for every cell with a case (see [`case_state`]).
async fn lexostatus_route(
    State(state): State<CellState>,
    Path(name): Path<String>,
    Query(mut inputs): Query<Map<String, Value>>,
) -> Result<Json<Lexostatus>, Error> {
    let as_of = AsOf::from_query(&mut inputs).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    // Only with the engine route is `engine_trace` not an input.
    let with_trace = state.cell.route.is_some() && inputs.remove(ENGINE_TRACE).is_some();
    if name == reduction::CASE_STATE && state.cell.has_cases() {
        // Not a reduction of a lexostatus definition but code of the
        // runtime; with the engine route the lexostatus says so too.
        return case_state(&state, &inputs, &as_of)
            .map(|l| Lexostatus {
                reduction: state.cell.route.as_ref().map(|_| ReductionRoute {
                    route: "runtime".into(),
                    regulation: None,
                    reason: Some(CASE_STATE_REASON.into()),
                    duration_us: 0,
                    dsl_duration_us: None,
                    trace_text: None,
                }),
                ..l
            })
            .map(Json);
    }
    let def = lexostatus_def(&state, &name)?;
    inputs_complete(def, &inputs)?;
    let grams = grams_for(&state, def, &inputs)?;
    reduce(
        &state,
        def,
        &inputs,
        grams.iter().map(|v| &v.gram),
        &as_of,
        with_trace,
    )
    .map(Json)
}

/// The query parameter that, with the engine route, asks for the trace of the
/// engine run (`?engine_trace=1`); not an input of the lexostatus.
pub const ENGINE_TRACE: &str = "engine_trace";

/// Why the case state does not go through the engine, in the route.
const CASE_STATE_REASON: &str = "the runtime itself offers the state of a case";

/// The day on which the engine reads the regulation of a lexostatus: that of
/// the as-of moment, without an as-of moment today.
fn engine_date(state: &CellState, as_of: &AsOf) -> String {
    as_of
        .as_of
        .and_then(|t| date::date_of(&t.to_string()))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| date::reference_date(&(state.clock)()))
}

/// Reduce a lexostatus, at the one place where the cell does so: along the
/// reduction DSL, or, in a runtime with the engine route, along the route of
/// the cell (see [`lexostatus_engine`]). 404: the lexostatus picks a gram and
/// there is none.
fn reduce<'g>(
    state: &CellState,
    def: &reduction::LexostatusDefinition,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
    as_of: &AsOf,
    with_trace: bool,
) -> Result<Lexostatus, Error> {
    match &state.cell.route {
        None => reduction::reduce_at(def, inputs, grams, as_of),
        Some(route) => lexostatus_engine::reduce_lexostatus(
            route,
            def,
            inputs,
            grams,
            as_of,
            &engine_date(state, as_of),
            with_trace,
        ),
    }
    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?
    .ok_or_else(|| error(StatusCode::NOT_FOUND, crate::synthesis::NO_GRAM))
}

/// The state of the group around a root (see [`reduction::CaseState`]): the
/// cell filters the grams of the root and derives what a process asks about
/// it. Input `root`; with `owner_path` (an `$intake` path without
/// `$intake.`) and `owner` also whether someone with that value knows the
/// group. 404 if the cell does not know the root.
fn case_state(
    state: &CellState,
    inputs: &Map<String, Value>,
    as_of: &AsOf,
) -> Result<Lexostatus, Error> {
    let text = |k: &str| {
        inputs
            .get(k)
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
    };
    let z = text(ROOT).ok_or_else(|| error(StatusCode::BAD_REQUEST, "input 'root' is absent"))?;
    let owner = match (text(reduction::OWNER_PATH), text(reduction::OWNER)) {
        (Some(p), Some(w)) => Some((p, w)),
        (None, None) => None,
        _ => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "ask for the owner with owner_path and owner together",
            ))
        }
    };
    let grams = state
        .chronicle
        .read_root(&state.cell.chronicles(), z)
        .map_err(internal)?;
    let cell = &state.cell;
    let binds = |g: &Gram, path: &str| -> Vec<String> {
        let Some((_, event)) = cell.event(&g.stream.id, &g.name) else {
            return Vec::new();
        };
        event
            .leaves()
            .into_iter()
            .filter(|b| {
                b.binding == stream::Binding::Intake(path.to_string())
                    || b.binding == stream::Binding::Supplied(path.to_string())
            })
            .map(|b| b.path)
            .collect()
    };
    reduction::reduce_case(grams.iter().map(|v| &v.gram), as_of, owner, binds)
        .map_err(internal)?
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                format!("no root '{z}' in the chronicle"),
            )
        })?
        .as_lexostatus(z, as_of)
        .map_err(internal)
}

/// The stream definitions of the cell, each with the hash that is in its
/// grams and in the receipt of a decision.
async fn stream_route(State(state): State<CellState>) -> Json<Value> {
    let streams: Vec<Value> = state
        .cell
        .streams
        .iter()
        .map(|s| json!({"id": s.id, "sha256": s.sha256, "stream": s.document}))
        .collect();
    Json(json!({"cell": state.cell.id(), "streams": streams}))
}

/// What `GET /api/cells` says about a cell: who it is, which chronicles it
/// keeps and which lexostatuses it offers.
pub fn cell_description(state: &CellState) -> Value {
    let cell = &state.cell;
    let mut lexostatuses: Vec<Value> = cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .map(|d| {
            let mut l = json!({
                "name": d.name,
                "inputs": d.inputs,
                "list": d.is_list(),
                "parameters": if d.is_list() { Vec::new() } else { d.reduction.derivations.keys().collect::<Vec<_>>() },
                "columns": if d.is_list() { d.reduction.derivations.keys().collect::<Vec<_>>() } else { Vec::new() },
                "extra_fields": d.reduction.extra_fields.keys().collect::<Vec<_>>(),
            });
            let r = reduction_of(cell, &d.name);
            if !r.is_null() {
                l["reduction"] = r;
            }
            l
        })
        .collect();
    if cell.has_cases() {
        // The runtime offers the state of a case, not the configuration.
        lexostatuses.push(json!({
            "name": reduction::CASE_STATE,
            "inputs": [{"name": ROOT, "type": "string"}],
            "list": false,
            "parameters": [],
            "columns": [],
            "extra_fields": ["grams", "events", "latest_effective_at", "stages", "owner"],
            "runtime": true,
        }));
        if cell.route.is_some() {
            if let Some(l) = lexostatuses.last_mut() {
                l["reduction"] = json!({"route": "runtime", "reason": CASE_STATE_REASON});
            }
        }
    }
    let mut out = json!({
        "id": cell.id(),
        "recording_actor": cell.definition.recording_actor,
        "chronicles": cell.chronicles(),
        "lexostatuses": lexostatuses,
    });
    // Only with the engine route: without it the description stays the same.
    if let Some(route) = &cell.route {
        out["reduction"] = json!(if route.compare { "compare" } else { "engine" });
    }
    out
}

/// Along which route the cell reduces a lexostatus, for the description:
/// null without engine route.
fn reduction_of(cell: &Cell, name: &str) -> Value {
    match cell.route.as_ref().and_then(|r| r.modes.get(name)) {
        None => Value::Null,
        Some(lexostatus_engine::Mode::Engine {
            regulation,
            article,
        }) => {
            json!({"route": "engine", "regulation": article.as_ref().unwrap_or(regulation)})
        }
        Some(lexostatus_engine::Mode::Dsl { reason }) => json!({"route": "dsl", "reason": reason}),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::{test_follower, test_gram};

    const Z: &str = "00000000-0000-4000-8000-000000000001";
    const DAY: &str = "2025-03-12T10:00:00+01:00";

    /// A cell with the fictional consumer: application, progress, decision,
    /// announcement and payment.
    fn cell() -> Cell {
        let map = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let service = Arc::new(
            crate::regulations::load(&map.join("regulation"))
                .unwrap()
                .service,
        );
        Cell::load(&map.join("cells/afnemer"), service).unwrap()
    }

    fn alias(name: &str, stream: &str, g: &mut Gram) {
        g.name = name.into();
        g.stream.id = stream.into();
    }

    fn application() -> Gram {
        let mut g = test_gram(Z);
        alias("aanvraag_ontvangen", "test_afnemer_aanvragen", &mut g);
        g.stage = Some("AANVRAAG".into());
        g.effective_at = "2025-03-01T10:00:00+01:00".into();
        g
    }

    fn follower(name: &str, reference: &str, target: &Gram, stage: Option<&str>) -> Gram {
        let mut g = test_follower(reference, target);
        alias(name, "test_afnemer_zaakverloop", &mut g);
        g.stage = stage.map(str::to_string);
        g.effective_at = DAY.into();
        g
    }

    fn assessment(
        c: &Cell,
        g: &Gram,
        targets: &[&Gram],
        group: &[&Gram],
        expected: Option<usize>,
    ) -> Result<(), (u16, String)> {
        let view = View {
            targets: targets.iter().map(|d| (d.id.clone(), *d)).collect(),
            group: group.to_vec(),
            root_error: None,
        };
        assessment_references(c, g, &view, expected).map_err(|Error(s, t)| (s.as_u16(), t))
    }

    #[test]
    fn a_reference_to_an_unknown_gram_is_refused() {
        let c = cell();
        let a = application();
        let b = follower("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let (status, f) = assessment(&c, &b, &[], &[&a], None).unwrap_err();
        assert_eq!(status, 400);
        assert!(f.contains("no gram"), "{f}");
    }

    /// A payment that refers to the application instead of to the decision is
    /// refused by the cell: `decision` points to a gram with stage
    /// BESLUIT.
    #[test]
    fn a_reference_to_the_wrong_gram_is_refused() {
        let c = cell();
        let a = application();
        let payment = follower("betaling_verricht", "decision", &a, None);
        let (status, f) = assessment(&c, &payment, &[&a], &[&a], None).unwrap_err();
        assert_eq!(status, 400);
        assert!(f.contains("stage BESLUIT"), "{f}");
        let b = follower("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let payment = follower("betaling_verricht", "decision", &b, None);
        assessment(&c, &payment, &[&b], &[&a, &b], None).unwrap();
    }

    /// The optimistic check: if the process records on a group with more
    /// (or fewer) grams than it read, the cell refuses (409).
    #[test]
    fn a_group_that_changed_since_reading_is_refused() {
        let c = cell();
        let a = application();
        let b = follower("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let (status, f) = assessment(&c, &b, &[&a], &[&a], Some(2)).unwrap_err();
        assert_eq!(status, 409);
        assert!(f.contains("changed since the process read it"), "{f}");
        assessment(&c, &b, &[&a], &[&a], Some(1)).unwrap();
    }

    #[test]
    fn a_gram_does_not_precede_the_gram_it_refers_to() {
        let c = cell();
        let a = application();
        let mut b = follower("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        b.effective_at = "2025-02-28T10:00:00+01:00".into();
        let (status, f) = assessment(&c, &b, &[&a], &[&a], None).unwrap_err();
        assert_eq!(status, 409);
        assert!(f.contains("is before the gram it refers to"), "{f}");
    }

    /// The cell refuses a second decision of the same event on the same
    /// application; a decision is announced once, another decision has
    /// its own announcement.
    #[test]
    fn a_decision_and_a_stage_once_per_target() {
        let c = cell();
        let a = application();
        let b1 = follower("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let yet_a = follower("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let (status, f) = assessment(&c, &yet_a, &[&a], &[&a, &b1], None).unwrap_err();
        assert_eq!(status, 409);
        assert!(
            f.contains("there is already a decision 'besluit_genomen'"),
            "{f}"
        );
        let bm1 = follower(
            "besluit_bekendgemaakt",
            "decision",
            &b1,
            Some("BEKENDMAKING"),
        );
        let bm2 = follower(
            "besluit_bekendgemaakt",
            "decision",
            &b1,
            Some("BEKENDMAKING"),
        );
        let (status, f) = assessment(&c, &bm2, &[&b1], &[&a, &b1, &bm1], None).unwrap_err();
        assert_eq!(status, 409);
        assert!(f.contains(&format!("for gram {}", b1.id)), "{f}");
        let mut b2 = follower("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        b2.id = uuid::Uuid::now_v7().to_string();
        let bm = follower(
            "besluit_bekendgemaakt",
            "decision",
            &b2,
            Some("BEKENDMAKING"),
        );
        assessment(&c, &bm, &[&b2], &[&a, &b1, &bm1, &b2], None).unwrap();
    }
}

//! The YAML fragment behind a step of an explanation (spec "waarom in de
//! aanvraag"): an article of the corpus in the version the cell uses, or a
//! block of a configuration file this process loaded. Read only, without
//! login: the corpus is not secret in the demo. Anything not loaded is 404.

use std::collections::BTreeMap;
use std::path::PathBuf;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use super::{error, Error, ProcessState};
use crate::fragment::{self, Fragment};

fn not_found(what: impl std::fmt::Display) -> Error {
    error(StatusCode::NOT_FOUND, format!("{what}: not loaded"))
}

/// The file of a regulation in the version that applies today (the version
/// the cell loaded; by the process clock), the newest without one.
fn law_file<'s>(state: &'s ProcessState, regulation: &str) -> Result<&'s PathBuf, Error> {
    let today = (state.clock)().date_naive();
    crate::regulations::file_of(
        &state.regulation_files,
        &state.process.service,
        regulation,
        Some(today),
    )
    .ok_or_else(|| not_found(regulation))
}

/// `GET /api/law/{regulation}/{article}`: the block of the article.
pub(super) async fn law_route(
    State(state): State<ProcessState>,
    Path((regulation, article)): Path<(String, String)>,
) -> Result<Json<Fragment>, Error> {
    let file = law_file(&state, &regulation)?;
    fragment::read(&state.root, file, |l| fragment::is_article(l, &article))
        .map(Json)
        .ok_or_else(|| not_found(format!("{regulation}#{article}")))
}

/// `GET /api/law/{regulation}`: the whole file of the regulation, in the
/// same version as its articles.
pub(super) async fn law_file_route(
    State(state): State<ProcessState>,
    Path(regulation): Path<String>,
) -> Result<Json<Fragment>, Error> {
    let file = law_file(&state, &regulation)?;
    fragment::whole(&state.root, file)
        .map(Json)
        .ok_or_else(|| not_found(&regulation))
}

/// The file of a configuration this process loaded: `process` (only for a
/// process from `process.yaml`), `form`, `stream/<id>` (a stream of its
/// cell), `cell` and `lexostatuses` (of its cell), `registers` (the binding
/// file of the deployment, if there is one) and, for a process from policy
/// (RFC-047), the deployment files `channels`, `synthesis` and `examples`.
fn config_file(state: &ProcessState, config: &str) -> Option<PathBuf> {
    let p = &state.process;
    match config.split_once('/') {
        None if config == "process" && p.definition.from_policy().is_none() => {
            Some(p.dir.join(crate::config::PROCESS_FILE))
        }
        None if config == "cell" => Some(p.cell.dir.join(crate::config::CELL_FILE)),
        None if config == "lexostatuses" => Some(p.cell.dir.join(&p.cell.definition.lexostatuses)),
        None if config == "registers" => state.registers_file.as_deref().cloned(),
        None if config == "form" => p
            .definition
            .portal
            .as_ref()?
            .form
            .as_ref()
            .map(|f| p.dir.join(&f.path)),
        None if config == "channels" => state.channels_file.as_deref().cloned(),
        None if config == "synthesis" => state.synthesis_file.as_deref().cloned(),
        None if config == "examples" => state.examples_file.as_deref().cloned(),
        Some(("stream", id)) => p.cell.streams.iter().find(|s| s.id == id)?.file.clone(),
        _ => None,
    }
}

/// `GET /api/config/{*config}?anchor=<key>`: the block of `anchor`, or the
/// whole file without one.
pub(super) async fn config_route(
    State(state): State<ProcessState>,
    Path(config): Path<String>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Result<Json<Fragment>, Error> {
    let file = config_file(&state, &config).ok_or_else(|| not_found(&config))?;
    let found = match query.get("anchor") {
        Some(a) => fragment::read(&state.root, &file, |l| fragment::is_anchor(l, a)),
        None => fragment::whole(&state.root, &file),
    };
    found.map(Json).ok_or_else(|| not_found(&config))
}

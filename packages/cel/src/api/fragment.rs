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

/// `GET /api/law/{regulation}/{article}`: the block of the article in the
/// version that applies today (the version the cell loaded; by the process
/// clock), the newest without one.
pub(super) async fn law_route(
    State(state): State<ProcessState>,
    Path((regulation, article)): Path<(String, String)>,
) -> Result<Json<Fragment>, Error> {
    let today = (state.clock)().date_naive();
    let file = crate::regulations::file_of(
        &state.regulation_files,
        &state.process.service,
        &regulation,
        Some(today),
    )
    .ok_or_else(|| not_found(&regulation))?;
    fragment::read(&state.root, file, |l| fragment::is_article(l, &article))
        .map(Json)
        .ok_or_else(|| not_found(format!("{regulation}#{article}")))
}

/// The file of a configuration this process loaded: `process`, `form`,
/// `stream/<id>` (a stream of its cell), `cell` and `lexostatuses` (of its
/// cell) and `registers` (the binding file of the deployment, if there is
/// one).
fn config_file(state: &ProcessState, config: &str) -> Option<PathBuf> {
    let p = &state.process;
    match config.split_once('/') {
        None if config == "process" => Some(p.dir.join(crate::config::PROCESS_FILE)),
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

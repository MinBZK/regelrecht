//! The YAML fragment behind a step of an explanation (spec "waarom in de
//! aanvraag"): an article of the corpus in the version the cell uses, or a
//! block of a configuration file this process loaded, also of a cell it
//! queries. Read only, without login: the corpus is not secret in the demo.
//! Anything not loaded is 404.

use std::collections::BTreeMap;
use std::path::PathBuf;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use super::{error, Error, ProcessState};
use crate::cell::Cell;
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

/// The file of a configuration of a cell: `cell`, `lexostatuses`,
/// `initial_state` (if it has one) and `stream/<id>`.
fn cell_file(cell: &Cell, config: &str) -> Option<PathBuf> {
    match config.split_once('/') {
        None if config == "cell" => Some(cell.dir.join(crate::config::CELL_FILE)),
        None if config == "lexostatuses" => Some(cell.dir.join(&cell.definition.lexostatuses)),
        None if config == "initial_state" => {
            Some(cell.dir.join(cell.definition.initial_state.as_ref()?))
        }
        Some(("stream", id)) => cell.streams.iter().find(|s| s.id == id)?.file.clone(),
        _ => None,
    }
}

/// The configuration files of the process and its deployment that are
/// loaded, under their key: `form` (the form of the portal), `registers`
/// (the binding file, if there is one) and `channels`, `synthesis` and
/// `examples` (RFC-047). The map gives each a node.
pub(super) fn process_files(state: &ProcessState) -> Vec<(&'static str, PathBuf)> {
    let p = &state.process;
    let form = p
        .definition
        .portal
        .as_ref()
        .and_then(|portal| portal.form.as_ref())
        .map(|f| p.dir.join(&f.path));
    [
        ("form", form),
        ("channels", state.channels_file.as_deref().cloned()),
        ("synthesis", state.synthesis_file.as_deref().cloned()),
        ("examples", state.examples_file.as_deref().cloned()),
        ("registers", state.registers_file.as_deref().cloned()),
    ]
    .into_iter()
    .filter_map(|(key, file)| Some((key, file?)))
    .collect()
}

/// The file of a configuration this process loaded: one of
/// [`process_files`], one of its own cell ([`cell_file`]), or
/// `cells/<id>/<config>` of a cell the process queries
/// ([`crate::map::queried_cells`], the cells on its map). Not of any other
/// cell: an initial state holds grams, which only the cell's readers read.
fn config_file(state: &ProcessState, config: &str) -> Option<PathBuf> {
    if let Some(rest) = config.strip_prefix("cells/") {
        let (id, config) = rest.split_once('/')?;
        let cells = crate::map::queried_cells(&state.process, &state.register_links, &state.cells);
        return cell_file(cells.into_iter().find(|c| c.id() == id)?, config);
    }
    process_files(state)
        .into_iter()
        .find(|(key, _)| *key == config)
        .map(|(_, file)| file)
        .or_else(|| cell_file(&state.process.cell, config))
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

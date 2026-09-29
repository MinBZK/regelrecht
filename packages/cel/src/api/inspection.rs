//! Inspection of the cells a process reads, for whoever may handle cases in
//! that process. The read routes of a cell are not open (the grams carry
//! the identity and the intake of whoever submitted, see [`super::cell`]); a
//! handler sees them through the process, which queries the cell with the
//! runtime token. Which cells those are follows from the configuration: the
//! own cell and every source that runs in this runtime. The process passes on
//! what the cell responds and derives nothing from it.

use std::collections::BTreeSet;

use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::Value;

use super::session::handler;
use super::{error, of_cell, Error, ProcessState};
use crate::cell_client::cell_path;

impl ProcessState {
    /// The cells in this runtime the process reads: the own cell, and the
    /// synthesis sources and the per-row sources without a url.
    pub fn inspection_cells(&self) -> BTreeSet<String> {
        let d = &self.process.definition;
        let mut out: BTreeSet<String> = std::iter::once(self.cell_id().to_string()).collect();
        out.extend(
            d.synthesis
                .iter()
                .filter(|b| b.url.is_none())
                .map(|b| b.cell.clone()),
        );
        let rows = d
            .portal
            .iter()
            .flat_map(|p| p.assessment.rows.iter())
            .chain(
                d.handling
                    .iter()
                    .flat_map(|b| b.actions.iter().flat_map(|h| h.rows.iter())),
            );
        out.extend(
            rows.flat_map(|r| r.sources.iter())
                .filter(|b| b.url.is_none())
                .map(|b| b.cell.clone()),
        );
        out
    }
}

fn allowed(state: &ProcessState, headers: &HeaderMap, cell: &str) -> Result<(), Error> {
    handler(state, headers)?;
    if state.inspection_cells().contains(cell) {
        Ok(())
    } else {
        Err(error(
            StatusCode::NOT_FOUND,
            format!("process '{}' reads no cell '{cell}'", state.process.id()),
        ))
    }
}

/// The chronicle of a cell the process reads.
pub(super) async fn chronicle_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
    Path(cell): Path<String>,
) -> Result<Json<Value>, Error> {
    allowed(&state, &headers, &cell)?;
    state
        .cell
        .fetch(&cell_path(&cell, "chronicle"))
        .await
        .map(Json)
        .map_err(of_cell)
}

/// A lexostatus of a cell the process reads, with the query as it came in
/// (the inputs, and the as-of if needed).
pub(super) async fn lexostatus_route(
    State(state): State<ProcessState>,
    headers: HeaderMap,
    Path((cell, name)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Json<Value>, Error> {
    allowed(&state, &headers, &cell)?;
    if name.is_empty() || name.chars().all(|c| c == '.') {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!("no lexostatus '{name}'"),
        ));
    }
    let name: String = crate::cell_client::url_segment(&name);
    let path = match query {
        Some(q) if !q.is_empty() => format!("lexostatus/{name}?{q}"),
        _ => format!("lexostatus/{name}"),
    };
    state
        .cell
        .fetch(&cell_path(&cell, &path))
        .await
        .map(Json)
        .map_err(of_cell)
}

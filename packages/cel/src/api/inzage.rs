//! Inzage in de cellen die een proces leest, voor wie in dat proces mag
//! behandelen. De leesroutes van een cel zijn niet open (de grammen dragen
//! de identiteit en de intake van wie indiende, zie [`super::cel`]); een
//! behandelaar ziet ze via het proces, dat de cel met het runtime-token
//! vraagt. Welke cellen dat zijn, volgt uit de configuratie: de eigen cel en
//! elke bron die in deze runtime draait. Het proces geeft door wat de cel
//! antwoordt en leidt er niets uit af.

use std::collections::BTreeSet;

use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::Value;

use super::sessie::behandelaar;
use super::{error, van_cel, Error, ProcesState};
use crate::celclient::celpad;

impl ProcesState {
    /// De cellen in deze runtime die het proces leest: de eigen cel, en de
    /// synthese-bronnen en de bronnen per regel zonder url.
    pub fn inzage_cellen(&self) -> BTreeSet<String> {
        let d = &self.proces.definitie;
        let mut uit: BTreeSet<String> = std::iter::once(self.cel_id().to_string()).collect();
        uit.extend(
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
        uit.extend(
            rows.flat_map(|r| r.sources.iter())
                .filter(|b| b.url.is_none())
                .map(|b| b.cell.clone()),
        );
        uit
    }
}

fn toegestaan(state: &ProcesState, headers: &HeaderMap, cell: &str) -> Result<(), Error> {
    behandelaar(state, headers)?;
    if state.inzage_cellen().contains(cell) {
        Ok(())
    } else {
        Err(error(
            StatusCode::NOT_FOUND,
            format!("proces '{}' leest geen cel '{cell}'", state.proces.id()),
        ))
    }
}

/// De kroniek van een cel die het proces leest.
pub(super) async fn kroniek_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path(cell): Path<String>,
) -> Result<Json<Value>, Error> {
    toegestaan(&state, &headers, &cell)?;
    state
        .cell
        .haal(&celpad(&cell, "chronicle"))
        .await
        .map(Json)
        .map_err(van_cel)
}

/// Een lexostatus van een cel die het proces leest, met de query zoals hij
/// kwam (de inputs, en zo nodig het peil).
pub(super) async fn lexostatus_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path((cell, name)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Json<Value>, Error> {
    toegestaan(&state, &headers, &cell)?;
    if name.is_empty() || name.chars().all(|c| c == '.') {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!("geen lexostatus '{name}'"),
        ));
    }
    let name: String = crate::celclient::url_segment(&name);
    let path = match query {
        Some(q) if !q.is_empty() => format!("lexostatus/{name}?{q}"),
        _ => format!("lexostatus/{name}"),
    };
    state
        .cell
        .haal(&celpad(&cell, &path))
        .await
        .map(Json)
        .map_err(van_cel)
}

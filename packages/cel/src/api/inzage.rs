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
use super::{fout, van_cel, Fout, ProcesState};
use crate::celclient::celpad;

impl ProcesState {
    /// De cellen in deze runtime die het proces leest: de eigen cel, en de
    /// synthese-bronnen en de bronnen per regel zonder url.
    pub fn inzage_cellen(&self) -> BTreeSet<String> {
        let d = &self.proces.definitie;
        let mut uit: BTreeSet<String> = std::iter::once(self.cel_id().to_string()).collect();
        uit.extend(
            d.synthese
                .iter()
                .filter(|b| b.url.is_none())
                .map(|b| b.cel.clone()),
        );
        let rijen = d.portaal.iter().flat_map(|p| p.toets.rijen.iter()).chain(
            d.behandeling
                .iter()
                .flat_map(|b| b.handelingen.iter().flat_map(|h| h.rijen.iter())),
        );
        uit.extend(
            rijen
                .flat_map(|r| r.bronnen.iter())
                .filter(|b| b.url.is_none())
                .map(|b| b.cel.clone()),
        );
        uit
    }
}

fn toegestaan(state: &ProcesState, headers: &HeaderMap, cel: &str) -> Result<(), Fout> {
    behandelaar(state, headers)?;
    if state.inzage_cellen().contains(cel) {
        Ok(())
    } else {
        Err(fout(
            StatusCode::NOT_FOUND,
            format!("proces '{}' leest geen cel '{cel}'", state.proces.id()),
        ))
    }
}

/// De kroniek van een cel die het proces leest.
pub(super) async fn kroniek_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path(cel): Path<String>,
) -> Result<Json<Value>, Fout> {
    toegestaan(&state, &headers, &cel)?;
    state
        .cel
        .haal(&celpad(&cel, "kroniek"))
        .await
        .map(Json)
        .map_err(van_cel)
}

/// Een lexostatus van een cel die het proces leest, met de query zoals hij
/// kwam (de inputs, en zo nodig het peil).
pub(super) async fn lexostatus_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path((cel, naam)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Json<Value>, Fout> {
    toegestaan(&state, &headers, &cel)?;
    if naam.is_empty() || naam.chars().all(|c| c == '.') {
        return Err(fout(
            StatusCode::BAD_REQUEST,
            format!("geen lexostatus '{naam}'"),
        ));
    }
    let naam: String = url_segment(&naam);
    let pad = match query {
        Some(q) if !q.is_empty() => format!("lexostatus/{naam}?{q}"),
        _ => format!("lexostatus/{naam}"),
    };
    state
        .cel
        .haal(&celpad(&cel, &pad))
        .await
        .map(Json)
        .map_err(van_cel)
}

/// Een padsegment zoals het in een url staat: alleen letters, cijfers en
/// `_-.` blijven staan.
fn url_segment(t: &str) -> String {
    t.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"_-.".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

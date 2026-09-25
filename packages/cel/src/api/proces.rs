//! De toestand en de routes van een proces, relatief aan `/processen/<id>`;
//! zie de tabel in [`crate::api`]. De handelingen staan in [`super::sessie`],
//! [`super::portaal`] en [`super::behandeling`].

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::behandeling::{besluit_route, proefbesluit_route, werkvoorraad_route, zaak_route};
use super::portaal::{formulier_route, indienen, mogelijkheden_route, toets_route};
use super::sessie::{login, logout, medewerker_login, medewerker_sessie, sessie};
use super::Klok;
use crate::gram::GeladenRegeling;
use crate::proces::Proces;
use crate::rijen::Rijen;
use crate::sessie::Sessies;
use crate::synthese::Bron;
use crate::transport::Transport;

/// De toestand van een proces in de runtime.
#[derive(Clone)]
pub struct ProcesState {
    pub proces: Arc<Proces>,
    /// Het transport naar de cel waarin het proces vastlegt (intern).
    pub cel: Arc<dyn Transport>,
    pub sessies: Arc<Sessies>,
    pub klok: Klok,
    /// De synthese-bronnen die geen lexostatus van de zaak zijn, met het
    /// transport dat de runtime koos.
    pub bronnen: Arc<Vec<Bron>>,
    /// De synthese per regel van het besluit, met haar bronnen.
    pub rijen: Arc<Vec<Rijen>>,
    /// De synthese per regel van de toets, met haar bronnen.
    pub toets_rijen: Arc<Vec<Rijen>>,
    /// De geladen regelingen, voor het receipt van een besluit.
    pub regelingen: Arc<Vec<GeladenRegeling>>,
}

impl ProcesState {
    pub(super) fn cel_id(&self) -> &str {
        self.proces.cel.id()
    }
}

/// De routes van een proces, relatief aan `/processen/<id>`.
pub fn proces_router(state: ProcesState) -> Router {
    let mut r = Router::new().route("/api/voorbeelden", get(voorbeelden_route));
    let rollen = &state.proces.definitie.rollen;
    if rollen.aanvrager.is_some() {
        r = r
            .route("/api/eherkenning/login", post(login))
            .route("/api/eherkenning/sessie", get(sessie))
            .route("/api/eherkenning/logout", post(logout));
    }
    if state.proces.portaal().is_some() {
        r = r
            .route("/api/formulier", get(formulier_route))
            .route("/api/aanvraag/toets", post(toets_route))
            .route("/api/aanvraag", post(indienen))
            .route("/api/mogelijkheden", get(mogelijkheden_route));
    }
    if rollen.behandelaar.is_some() {
        r = r
            .route("/api/medewerker/login", post(medewerker_login))
            .route("/api/medewerker/sessie", get(medewerker_sessie))
            .route("/api/medewerker/logout", post(logout));
    }
    if state.proces.definitie.behandeling.is_some() {
        r = r
            .route("/api/werkvoorraad", get(werkvoorraad_route))
            .route("/api/zaken/{zaakkenmerk}", get(zaak_route))
            .route(
                "/api/zaken/{zaakkenmerk}/proefbesluit",
                post(proefbesluit_route),
            )
            .route("/api/zaken/{zaakkenmerk}/besluit", post(besluit_route));
    }
    r.with_state(state)
}

/// De voorbeelden van het proces. Ook zonder login: de inlogvoorbeelden zijn
/// er juist voor het inloggen.
async fn voorbeelden_route(
    State(state): State<ProcesState>,
) -> Json<crate::voorbeelden::Voorbeelden> {
    Json(state.proces.voorbeelden.clone())
}

/// Wat `GET /api/processen` over een proces zegt: wie handelt, in welke cel,
/// met welke rollen, of er een portaal en een behandeling is, en uit welke
/// bronnen de synthese samenvoegt.
pub fn proces_beschrijving(state: &ProcesState) -> Value {
    let p = &state.proces;
    let d = &p.definitie;
    let synthese: Vec<Value> = d
        .zaakbronnen()
        .map(|b| {
            json!({
                "cel": b.cel,
                "lexostatus": b.lexostatus,
                "zaak": true,
                "transport": state.cel.soort(),
                "parameters": Vec::<String>::new(),
            })
        })
        .chain(state.bronnen.iter().map(|b| {
            json!({
                "cel": b.definitie.cel,
                "lexostatus": b.definitie.lexostatus,
                "zaak": false,
                "transport": b.transport.soort(),
                "parameters": b.definitie.parameters,
            })
        }))
        .collect();
    json!({
        "id": p.id(),
        "actor": d.actor,
        "cel": p.cel.id(),
        "portaal": p.portaal().is_some(),
        "rollen": {
            "aanvrager": d.rollen.aanvrager.is_some(),
            "behandelaar": d.rollen.behandelaar.is_some(),
        },
        "behandeling": d.behandeling.as_ref().map(|b| json!({
            "werkvoorraad": b.werkvoorraad.lexostatus,
            "regeling": b.besluit.regeling,
            "uitkomsten": b.besluit.uitkomsten,
        })),
        "titel": p.formulier.as_ref().and_then(|f| f.titel.clone()),
        "synthese": synthese,
    })
}

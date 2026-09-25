//! De toestand en de routes van een proces, relatief aan `/processen/<id>`;
//! zie de tabel in [`crate::api`]. De handelingen staan in [`super::sessie`],
//! [`super::portaal`] en [`super::behandeling`].

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::behandeling::{handeling_route, proefhandeling_route, werkvoorraad_route, zaak_route};
use super::loket::loket_indienen;
use super::portaal::{formulier_route, indienen, mogelijkheden_route, toets_route};
use super::sessie::{kanaalsessie, login, logout, sessie};
use super::Klok;
use crate::gram::GeladenRegeling;
use crate::kanaal::Routes;
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
    /// Per handeling (in de volgorde van `proces.yaml`) de bronnen die haar
    /// artikel vraagt en haar synthese per regel.
    pub handelingen: Arc<Vec<HandelingState>>,
    /// De synthese per regel van de toets, met haar bronnen.
    pub toets_rijen: Arc<Vec<Rijen>>,
    /// De geladen regelingen, voor het receipt van een besluit.
    pub regelingen: Arc<Vec<GeladenRegeling>>,
}

/// Wat de runtime per handeling klaarzet: de synthese-bronnen die haar
/// artikel vraagt (zie [`crate::handeling::bronnen_voor`]) en haar synthese
/// per regel, met het transport dat de runtime koos.
#[derive(Clone)]
pub struct HandelingState {
    pub bronnen: Vec<Bron>,
    pub rijen: Vec<Rijen>,
}

impl ProcesState {
    pub(super) fn cel_id(&self) -> &str {
        self.proces.cel.id()
    }
}

/// De routes van een proces, relatief aan `/processen/<id>`. Een proces met
/// rollen heeft de routes van zijn kanalen; de routegroepen staan er als het
/// proces ze heeft, en elke route controleert of de rol van de ingelogde
/// gebruiker die groep mag gebruiken.
pub fn proces_router(state: ProcesState) -> Router {
    let mut r = Router::new().route("/api/voorbeelden", get(voorbeelden_route));
    let d = &state.proces.definitie;
    if !d.rollen.is_empty() {
        r = r
            .route("/api/kanalen/{kanaal}/login", post(login))
            .route("/api/kanalen/{kanaal}/sessie", get(kanaalsessie))
            .route("/api/kanalen/{kanaal}/logout", post(logout))
            .route("/api/sessie", get(sessie));
    }
    if state.proces.portaal().is_some() {
        r = r
            .route("/api/formulier", get(formulier_route))
            .route("/api/aanvraag/toets", post(toets_route))
            .route("/api/aanvraag", post(indienen))
            .route("/api/mogelijkheden", get(mogelijkheden_route));
    }
    if d.rollen_met(Routes::Loket).next().is_some() {
        r = r.route("/api/loket/aanvraag", post(loket_indienen));
    }
    if d.behandeling.is_some() {
        r = r
            .route("/api/werkvoorraad", get(werkvoorraad_route))
            .route("/api/zaken/{zaakkenmerk}", get(zaak_route))
            .route(
                "/api/zaken/{zaakkenmerk}/handelingen/{naam}",
                post(handeling_route),
            )
            .route(
                "/api/zaken/{zaakkenmerk}/handelingen/{naam}/proef",
                post(proefhandeling_route),
            );
    }
    r.with_state(state)
}

/// De voorbeelden van het proces. Ook zonder login: de inlogvoorbeelden zijn
/// er juist voor het inloggen.
async fn voorbeelden_route(
    State(state): State<ProcesState>,
) -> Json<crate::voorbeelden::Voorbeelden> {
    Json(
        state
            .proces
            .voorbeelden
            .op(&crate::datum::peildatum(&(state.klok)())),
    )
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
                // De bron spreekt haar eigen taal; de afnemer vertaalt.
                "vertaling": b.definitie.parameters.vertaald(),
            })
        }))
        .collect();
    json!({
        "id": p.id(),
        "actor": d.actor,
        "cel": p.cel.id(),
        "portaal": p.portaal().is_some(),
        "gezag": p.gezag,
        "kanalen": d.kanalen.iter().map(|(id, k)| (id.clone(), json!({
            "label": k.label,
            "uitleg": k.uitleg,
            "velden": k.velden,
            "eigenaar": k.eigenaar,
        }))).collect::<serde_json::Map<String, Value>>(),
        "rollen": d.rollen.iter().map(|(id, r)| (id.clone(), json!({
            "kanaal": r.kanaal,
            "routes": r.routes,
            "label": r.label.as_deref().unwrap_or(id),
        }))).collect::<serde_json::Map<String, Value>>(),
        "loket": d.rollen_met(Routes::Loket).next().is_some(),
        "behandeling": d.behandeling.as_ref().map(|b| json!({
            "werkvoorraad": b.werkvoorraad.lexostatus,
            "handelingen": b.handelingen.iter().map(|h| json!({
                "naam": h.naam,
                "label": h.label(),
                "rol": h.rol,
                "soort": h.soort,
                "stage": h.stage,
                "regeling": h.regeling,
                "artikel": h.artikel,
                "uitkomsten": h.uitkomsten,
            })).collect::<Vec<_>>(),
        })),
        "titel": p.formulier.as_ref().and_then(|f| f.titel.clone()),
        "synthese": synthese,
    })
}

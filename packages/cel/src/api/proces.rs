//! De toestand en de routes van een proces, relatief aan `/processen/<id>`;
//! zie de tabel in [`crate::api`]. De handelingen staan in [`super::sessie`],
//! [`super::portaal`] en [`super::behandeling`].

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::behandeling::{handeling_route, proefhandeling_route, werkvoorraad_route, zaak_route};
use super::inzage;
use super::loket::loket_indienen;
use super::portaal::{formulier_route, indienen, mogelijkheden_route, toets_route};
use super::sessie::{kanaalsessie, login, logout, session};
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
    pub cell: Arc<dyn Transport>,
    pub sessies: Arc<Sessies>,
    pub klok: Klok,
    /// De synthese-bronnen die geen lexostatus van de zaak zijn, met het
    /// transport dat de runtime koos.
    pub sources: Arc<Vec<Bron>>,
    /// Per handeling (in de volgorde van `proces.yaml`) de bronnen die haar
    /// artikel vraagt en haar synthese per regel.
    pub actions: Arc<Vec<HandelingState>>,
    /// De synthese per regel van de toets, met haar bronnen.
    pub toets_rijen: Arc<Vec<Rijen>>,
    /// De geladen regelingen, voor het receipt van een besluit.
    pub regulations: Arc<Vec<GeladenRegeling>>,
}

/// Wat de runtime per handeling klaarzet: de synthese-bronnen die haar
/// artikel vraagt (zie [`crate::handeling::bronnen_voor`]) en haar synthese
/// per regel, met het transport dat de runtime koos.
#[derive(Clone)]
pub struct HandelingState {
    pub sources: Vec<Bron>,
    pub rows: Vec<Rijen>,
}

impl ProcesState {
    pub(super) fn cel_id(&self) -> &str {
        self.proces.cell.id()
    }
}

/// De routes van een proces, relatief aan `/processen/<id>`. Een proces met
/// rollen heeft de routes van zijn kanalen; de routegroepen staan er als het
/// proces ze heeft, en elke route controleert of de rol van de ingelogde
/// gebruiker die groep mag gebruiken.
pub fn proces_router(state: ProcesState) -> Router {
    let mut r = Router::new().route("/api/examples", get(voorbeelden_route));
    let d = &state.proces.definitie;
    if !d.roles.is_empty() {
        r = r
            .route("/api/channels/{channel}/login", post(login))
            .route("/api/channels/{channel}/session", get(kanaalsessie))
            .route("/api/channels/{channel}/logout", post(logout))
            .route("/api/session", get(session));
    }
    if state.proces.portal().is_some() {
        r = r
            .route("/api/form", get(formulier_route))
            .route("/api/application/assessment", post(toets_route))
            .route("/api/application", post(indienen))
            .route("/api/possibilities", get(mogelijkheden_route));
    }
    if d.rollen_met(Routes::Counter).next().is_some() {
        r = r.route("/api/counter/application", post(loket_indienen));
    }
    if d.handling.is_some() {
        r = r
            .route(
                "/api/inspection/{cell}/chronicle",
                get(inzage::kroniek_route),
            )
            .route(
                "/api/inspection/{cell}/lexostatus/{name}",
                get(inzage::lexostatus_route),
            )
            .route("/api/worklist", get(werkvoorraad_route))
            .route("/api/cases/{root}", get(zaak_route))
            .route("/api/cases/{root}/actions/{name}", post(handeling_route))
            .route(
                "/api/cases/{root}/actions/{name}/trial",
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
            .examples
            .op(&crate::datum::reference_date(&(state.klok)())),
    )
}

/// Wat `GET /api/processen` over een proces zegt: wie handelt, in welke cel,
/// met welke rollen, of er een portaal en een behandeling is, en uit welke
/// bronnen de synthese samenvoegt.
pub fn proces_beschrijving(state: &ProcesState) -> Value {
    let p = &state.proces;
    let d = &p.definitie;
    let synthesis: Vec<Value> = d
        .zaakbronnen()
        .map(|b| {
            json!({
                "cell": b.cell,
                "lexostatus": b.lexostatus,
                "case": true,
                "transport": state.cell.soort(),
                "parameters": Vec::<String>::new(),
            })
        })
        .chain(state.sources.iter().map(|b| {
            json!({
                "cell": b.definitie.cell,
                "lexostatus": b.definitie.lexostatus,
                "case": false,
                "transport": b.transport.soort(),
                "parameters": b.definitie.parameters,
                // De bron spreekt haar eigen taal; de afnemer vertaalt.
                "translation": b.definitie.parameters.vertaald(),
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
        "counter": d.rollen_met(Routes::Counter).next().is_some(),
        "handling": d.handling.as_ref().map(|b| json!({
            "worklist": b.worklist.lexostatus,
            "actions": b.actions.iter().map(|h| json!({
                "name": h.name,
                "label": h.label(),
                "role": h.role,
                "kind": h.soort,
                "stage": h.stage,
                "regulation": h.regulation,
                "article": h.article,
                "outputs": h.outputs,
            })).collect::<Vec<_>>(),
        })),
        "title": p.form.as_ref().and_then(|f| f.title.clone()),
        "synthesis": synthesis,
        // De cellen die een behandelaar via dit proces mag inzien.
        "inspection": if d.handling.is_some() { state.inzage_cellen().into_iter().collect() } else { Vec::new() },
    })
}

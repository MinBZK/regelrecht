//! De behandeling in een proces: de werkvoorraad, een zaak en de handelingen
//! erin, op proef en genomen. Een generieke route voor elke handeling
//! (`zaken/{z}/handelingen/{naam}`), geen route per soort besluit: wat een
//! handeling nodig heeft, volgt uit de stage en de origin (zie
//! [`crate::handeling`]).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use futures_util::future::join_all;
use serde_json::{json, Map, Value};

use super::sessie::{behandelaar, voor_handeling};
use super::{error, van_cel, Error, ProcesState};
use crate::celclient;
use crate::config::HandelingDefinitie;
use crate::handeling::{self, Omgeving, Opgave, Weigering};
use crate::reductie::Peil;
use crate::reductie::Zaakstand;
use crate::rijen::Rijen;
use crate::synthese::{self, Bron};
use crate::transport::{Onthouden, Transport};
use chrono::{DateTime, FixedOffset};

fn handling(state: &ProcesState) -> Result<&crate::config::Handling, Error> {
    state.proces.definitie.handling.as_ref().ok_or_else(|| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen behandeling geconfigureerd",
        )
    })
}

/// De werkvoorraad: de lijst-lexostatus uit de cel.
pub(super) async fn werkvoorraad_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    behandelaar(&state, &headers)?;
    let w = &handling(&state)?.worklist;
    let v = state
        .cell
        .haal(&synthese::path(
            &w.cell,
            &w.lexostatus,
            &Map::new(),
            &Peil::default(),
        ))
        .await
        .map_err(van_cel)?;
    Ok(Json(v))
}

/// Een weigering als HTTP-antwoord. Wat de stand van de zaak niet toelaat
/// (niet te nemen, al vastgelegd, een ander bevoegd gezag) is een 409.
fn weigering(w: Weigering) -> Error {
    match w {
        Weigering::Ongeldig(t) => error(StatusCode::BAD_REQUEST, t),
        Weigering::NietTeNemen(t) | Weigering::Conflict(t) | Weigering::Onbevoegd(t) => {
            error(StatusCode::CONFLICT, t)
        }
        Weigering::Cell(t) => error(StatusCode::INTERNAL_SERVER_ERROR, t),
    }
}

/// De grammen van een zaak, zoals de cel ze geeft; een 404 als de cel de
/// zaak niet kent. Alleen voor inzage in het dossier: het proces leidt er
/// niets uit af.
async fn zaakgrammen(state: &ProcesState, root: &str) -> Result<Vec<celclient::MetYaml>, Error> {
    celclient::lees_zaak(state.cell.as_ref(), state.cel_id(), root)
        .await
        .map_err(van_cel)
}

/// De stand van een zaak, zoals de cel haar afleidt; een 404 als de cel de
/// zaak niet kent.
async fn zaakstand(state: &ProcesState, root: &str) -> Result<Zaakstand, Error> {
    celclient::zaakstand(state.cell.as_ref(), state.cel_id(), root, None)
        .await
        .map_err(van_cel)
}

/// De handeling met deze naam, met haar plek; 404 als het proces haar niet
/// kent.
fn handeling_met<'s>(
    state: &'s ProcesState,
    name: &str,
) -> Result<(usize, &'s HandelingDefinitie), Error> {
    handling(state)?
        .actions
        .iter()
        .enumerate()
        .find(|(_, h)| h.name == name)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("geen handeling '{name}'")))
}

fn omgeving(state: &ProcesState, i: usize) -> Omgeving<'_> {
    let h = &state.actions[i];
    omgeving_met(
        state,
        state.cell.as_ref(),
        &h.sources,
        &h.rows,
        (state.klok)(),
    )
}

/// De omgeving van een handeling met een gegeven transport naar de cel en
/// gegeven bronnen (zoals die het zaakscherm deelt).
fn omgeving_met<'a>(
    state: &'a ProcesState,
    cell: &'a dyn Transport,
    sources: &'a [Bron],
    rows: &'a [Rijen],
    nu: DateTime<FixedOffset>,
) -> Omgeving<'a> {
    Omgeving {
        proces: &state.proces,
        cell,
        sources,
        rows,
        regulations: &state.regulations,
        nu,
    }
}

/// De zaak: haar grammen (inzage in het dossier, zonder ze te
/// interpreteren), de procedure van de zaak (de stages die bij geen besluit
/// horen, zoals de aanvraag), de besluiten met per besluit de stages die er
/// liggen en de rechtsbescherming die daaruit volgt, en per handeling of zij
/// kan, op welk besluit zij handelt, haar formulier en een proef zonder
/// formulier (bij een betaling: wat er nog te betalen is). Wat het proces over de zaak weet, komt uit de stand die de
/// cel afleidt.
pub(super) async fn zaak_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path(root): Path<String>,
) -> Result<Json<Value>, Error> {
    behandelaar(&state, &headers)?;
    let case = zaakstand(&state, &root).await?;
    let grams = zaakgrammen(&state, &root).await?;
    let b = handling(&state)?;
    let leeg = Opgave::default();
    // De zaakcontext (de lexostatussen van de zaak, de synthese en de
    // synthese per regel) is voor elke handeling op dezelfde peildatum
    // dezelfde: de proeven delen een geheugen voor wat ze lezen, zodat het
    // proces elke lexostatus en elke bron een keer vraagt. Een feit dat op
    // proef als concept meetelt, reduceert de cel per handeling.
    let geheugen = Onthouden::default();
    let cell = geheugen.om(state.cell.clone());
    let gedeeld: Vec<(Vec<Bron>, Vec<Rijen>)> = state
        .actions
        .iter()
        .map(|hs| {
            let sources = hs
                .sources
                .iter()
                .map(|b| b.langs(|t| geheugen.om(t)))
                .collect();
            let rows = hs
                .rows
                .iter()
                .map(|r| Rijen {
                    definitie: r.definitie.clone(),
                    sources: r
                        .sources
                        .iter()
                        .map(|b| b.langs(|t| geheugen.om(t)))
                        .collect(),
                })
                .collect();
            (sources, rows)
        })
        .collect();
    let nu = (state.klok)();
    let proeven = join_all(b.actions.iter().enumerate().map(|(i, h)| {
        let om = omgeving_met(&state, cell.as_ref(), &gedeeld[i].0, &gedeeld[i].1, nu);
        let stand = handeling::stand(&state.proces, h, &case);
        let case = &case;
        let root = &root;
        let leeg = &leeg;
        async move {
            // Een stage die al ligt of nog niet kan, rekent de zaak niet uit.
            let trial = if stand.available {
                Some(handeling::trial(&om, h, root, case, leeg).await)
            } else {
                None
            };
            (stand, trial)
        }
    }))
    .await;
    let mut actions = Vec::new();
    for (h, (stand, trial)) in b.actions.iter().zip(proeven) {
        // De controle bij het opstarten las dit al; een fout hier is er een
        // van de runtime.
        let benodigd = handeling::benodigd(&state.proces.service, h)
            .map_err(|f| error(StatusCode::INTERNAL_SERVER_ERROR, f))?;
        let trial = match trial {
            None => Value::Null,
            Some(Ok(p)) => json!(p),
            Some(Err(w)) => json!({"error": weigering_tekst(&w)}),
        };
        let form: Vec<Value> = h
            .verdicts
            .iter()
            .map(|o| {
                let typing = benodigd.get(&o.parameter).map(|b| &b.typing);
                let mut v = json!({
                    "name": o.parameter,
                    "label": o.label,
                    "type": typing.map(|t| handeling::veldsoort(t.soort)),
                    "group": o.group,
                    "kind": "verdict",
                });
                if let Some(e) = typing.and_then(|t| t.unit.as_deref()) {
                    v["unit"] = json!(e);
                }
                v
            })
            .chain(h.feiten.iter().map(|f| {
                let mut v = json!(f);
                v["kind"] = json!("fact");
                v
            }))
            .collect();
        actions.push(json!({
            "name": h.name,
            "label": h.label(),
            "role": h.role,
            "kind": h.soort,
            "stage": h.stage,
            "regulation": h.regulation,
            "article": h.article,
            "outputs": h.outputs,
            "assessments": h.assessments,
            "types": h.types,
            "hooks": h.hooks,
            "not_yet": h.not_yet,
            "form": form,
            "available": stand.available,
            "reason": stand.reason,
            "recorded": stand.recorded,
            "decision": stand.decision,
            "decision_role": h.decision_role,
            "trial": trial,
        }));
    }
    Ok(Json(json!({
        "root": root,
        "grams": grams,
        "procedure": handeling::procedure_van_de_zaak(&state.proces, &case),
        "decisions": handeling::besluiten_in_zaak(&state.proces, &case),
        "actions": actions,
    })))
}

fn weigering_tekst(w: &Weigering) -> String {
    match w {
        Weigering::Ongeldig(t)
        | Weigering::NietTeNemen(t)
        | Weigering::Conflict(t)
        | Weigering::Onbevoegd(t)
        | Weigering::Cell(t) => t.clone(),
    }
}

/// Een handeling op proef: niets wordt vastgelegd.
pub(super) async fn proefhandeling_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path((root, name)): Path<(String, String)>,
    Json(opgave): Json<Opgave>,
) -> Result<Json<handeling::Proefhandeling>, Error> {
    let (i, h) = handeling_met(&state, &name)?;
    voor_handeling(&state, &headers, h.role.as_deref())?;
    let case = zaakstand(&state, &root).await?;
    handeling::trial(&omgeving(&state, i), h, &root, &case, &opgave)
        .await
        .map(Json)
        .map_err(weigering)
}

/// Een handeling nemen en laten vastleggen (201), of een weigering (409):
/// niet te nemen, de stage ligt al in de zaak, de zaak veranderde, of de
/// wet wijst een ander bevoegd gezag aan. Met `gebeurd: true` meldt de
/// behandelaar een feit dat gebeurde terwijl de proef om de inhoud nee zei;
/// de cel legt het dan vast (zie [`handeling::neem`]).
pub(super) async fn handeling_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path((root, name)): Path<(String, String)>,
    Json(opgave): Json<Opgave>,
) -> Result<(StatusCode, Json<handeling::Genomen>), Error> {
    let (i, h) = handeling_met(&state, &name)?;
    let wie = voor_handeling(&state, &headers, h.role.as_deref())?;
    let case = zaakstand(&state, &root).await?;
    let genomen = handeling::neem(&omgeving(&state, i), h, &root, &case, &opgave, &wie)
        .await
        .map_err(weigering)?;
    tracing::info!(
        proces = %state.proces.id(),
        root = %root,
        action = %h.name,
        stage = genomen.gram.stage.as_deref().unwrap_or("-"),
        "handeling vastgelegd"
    );
    Ok((StatusCode::CREATED, Json(genomen)))
}

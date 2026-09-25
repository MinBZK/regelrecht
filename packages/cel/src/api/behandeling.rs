//! De behandeling in een proces: de werkvoorraad, een zaak en de handelingen
//! erin, op proef en genomen. Een generieke route voor elke handeling
//! (`zaken/{z}/handelingen/{naam}`), geen route per soort besluit: wat een
//! handeling nodig heeft, volgt uit de stage en de origin (zie
//! [`crate::handeling`]).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use futures_util::future::join_all;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::sessie::{behandelaar, voor_handeling};
use super::{fout, van_cel, Fout, ProcesState};
use crate::celclient;
use crate::config::HandelingDefinitie;
use crate::gram::Gram;
use crate::handeling::{self, Omgeving, Weigering};
use crate::reductie::Peil;
use crate::synthese;

fn behandeling(state: &ProcesState) -> Result<&crate::config::Behandeling, Fout> {
    state.proces.definitie.behandeling.as_ref().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen behandeling geconfigureerd",
        )
    })
}

/// De werkvoorraad: de lijst-lexostatus uit de cel.
pub(super) async fn werkvoorraad_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Fout> {
    behandelaar(&state, &headers)?;
    let w = &behandeling(&state)?.werkvoorraad;
    let v = state
        .cel
        .haal(&synthese::pad(
            &w.cel,
            &w.lexostatus,
            &Map::new(),
            &Peil::default(),
        ))
        .await
        .map_err(van_cel)?;
    Ok(Json(v))
}

#[derive(Deserialize, Default)]
pub(super) struct Formulier {
    #[serde(default)]
    formulier: Map<String, Value>,
}

/// Een weigering als HTTP-antwoord. Wat de stand van de zaak niet toelaat
/// (niet te nemen, al vastgelegd, een ander bevoegd gezag) is een 409.
fn weigering(w: Weigering) -> Fout {
    match w {
        Weigering::Ongeldig(t) => fout(StatusCode::BAD_REQUEST, t),
        Weigering::NietTeNemen(t) | Weigering::Conflict(t) | Weigering::Onbevoegd(t) => {
            fout(StatusCode::CONFLICT, t)
        }
        Weigering::Cel(t) => fout(StatusCode::INTERNAL_SERVER_ERROR, t),
    }
}

/// De grammen van een zaak, zoals de cel ze geeft; een 404 als de cel de
/// zaak niet kent.
async fn zaakgrammen(
    state: &ProcesState,
    zaakkenmerk: &str,
) -> Result<Vec<celclient::MetYaml>, Fout> {
    celclient::lees_zaak(state.cel.as_ref(), state.cel_id(), zaakkenmerk)
        .await
        .map_err(van_cel)
}

/// De handeling met deze naam, met haar plek; 404 als het proces haar niet
/// kent.
fn handeling_met<'s>(
    state: &'s ProcesState,
    naam: &str,
) -> Result<(usize, &'s HandelingDefinitie), Fout> {
    behandeling(state)?
        .handelingen
        .iter()
        .enumerate()
        .find(|(_, h)| h.naam == naam)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, format!("geen handeling '{naam}'")))
}

fn omgeving(state: &ProcesState, i: usize) -> Omgeving<'_> {
    let h = &state.handelingen[i];
    Omgeving {
        proces: &state.proces,
        cel: state.cel.as_ref(),
        bronnen: &h.bronnen,
        rijen: &h.rijen,
        regelingen: &state.regelingen,
        nu: (state.klok)(),
    }
}

/// De zaak: haar grammen, de procedure met de stages die er liggen, de
/// rechtsbescherming die daaruit volgt, en per handeling of zij kan, haar
/// formulier en een proef zonder formulier (bij een betaling: wat er nog te
/// betalen is).
pub(super) async fn zaak_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path(zaakkenmerk): Path<String>,
) -> Result<Json<Value>, Fout> {
    behandelaar(&state, &headers)?;
    let grammen = zaakgrammen(&state, &zaakkenmerk).await?;
    let zaak: Vec<Gram> = grammen.iter().map(|m| m.gram.clone()).collect();
    let b = behandeling(&state)?;
    let leeg = Map::new();
    let proeven = join_all(b.handelingen.iter().enumerate().map(|(i, h)| {
        let om = omgeving(&state, i);
        let stand = handeling::stand(&state.proces, h, &zaak);
        let zaak = &zaak;
        let zaakkenmerk = &zaakkenmerk;
        let leeg = &leeg;
        async move {
            // Een stage die al ligt of nog niet kan, rekent de zaak niet uit.
            let proef = if stand.beschikbaar {
                Some(handeling::proef(&om, h, zaakkenmerk, zaak, leeg).await)
            } else {
                None
            };
            (stand, proef)
        }
    }))
    .await;
    let mut handelingen = Vec::new();
    for (h, (stand, proef)) in b.handelingen.iter().zip(proeven) {
        let proef = match proef {
            None => Value::Null,
            Some(Ok(p)) => json!(p),
            Some(Err(w)) => json!({"fout": weigering_tekst(&w)}),
        };
        let formulier: Vec<Value> = h
            .oordelen
            .iter()
            .map(|o| {
                let soort = handeling::benodigd(&state.proces.service, h)
                    .ok()
                    .and_then(|b| b.get(&o.parameter).cloned())
                    .and_then(|b| handeling::veldsoort(&b.soort));
                json!({
                    "naam": o.parameter,
                    "label": o.label,
                    "type": soort,
                    "groep": o.groep,
                    "soort": "oordeel",
                })
            })
            .chain(h.feiten.iter().map(|f| {
                let mut v = json!(f);
                v["soort"] = json!("feit");
                v
            }))
            .collect();
        handelingen.push(json!({
            "naam": h.naam,
            "label": h.label(),
            "rol": h.rol,
            "soort": h.soort,
            "stage": h.stage,
            "regeling": h.regeling,
            "artikel": h.artikel,
            "uitkomsten": h.uitkomsten,
            "toetsen": h.toetsen,
            "haken": h.haken,
            "nog_niet": h.nog_niet,
            "formulier": formulier,
            "beschikbaar": stand.beschikbaar,
            "reden": stand.reden,
            "vastgelegd": stand.vastgelegd,
            "proef": proef,
        }));
    }
    let (procedure, rechtsbescherming) = handeling::procedure_en_route(&state.proces, &zaak);
    Ok(Json(json!({
        "zaakkenmerk": zaakkenmerk,
        "grammen": grammen,
        "procedure": procedure,
        "rechtsbescherming": rechtsbescherming,
        "handelingen": handelingen,
    })))
}

fn weigering_tekst(w: &Weigering) -> String {
    match w {
        Weigering::Ongeldig(t)
        | Weigering::NietTeNemen(t)
        | Weigering::Conflict(t)
        | Weigering::Onbevoegd(t)
        | Weigering::Cel(t) => t.clone(),
    }
}

/// Een handeling op proef: niets wordt vastgelegd.
pub(super) async fn proefhandeling_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path((zaakkenmerk, naam)): Path<(String, String)>,
    Json(invoer): Json<Formulier>,
) -> Result<Json<handeling::Proefhandeling>, Fout> {
    let (i, h) = handeling_met(&state, &naam)?;
    voor_handeling(&state, &headers, h.rol.as_deref())?;
    let zaak: Vec<Gram> = zaakgrammen(&state, &zaakkenmerk)
        .await?
        .into_iter()
        .map(|m| m.gram)
        .collect();
    handeling::proef(
        &omgeving(&state, i),
        h,
        &zaakkenmerk,
        &zaak,
        &invoer.formulier,
    )
    .await
    .map(Json)
    .map_err(weigering)
}

/// Een handeling nemen en laten vastleggen (201), of een weigering (409):
/// niet te nemen, de stage ligt al in de zaak, de zaak veranderde, of de
/// wet wijst een ander bevoegd gezag aan.
pub(super) async fn handeling_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path((zaakkenmerk, naam)): Path<(String, String)>,
    Json(invoer): Json<Formulier>,
) -> Result<(StatusCode, Json<handeling::Genomen>), Fout> {
    let (i, h) = handeling_met(&state, &naam)?;
    let wie = voor_handeling(&state, &headers, h.rol.as_deref())?;
    let zaak: Vec<Gram> = zaakgrammen(&state, &zaakkenmerk)
        .await?
        .into_iter()
        .map(|m| m.gram)
        .collect();
    let genomen = handeling::neem(
        &omgeving(&state, i),
        h,
        &zaakkenmerk,
        &zaak,
        &invoer.formulier,
        &wie,
    )
    .await
    .map_err(weigering)?;
    tracing::info!(
        proces = %state.proces.id(),
        zaakkenmerk = %zaakkenmerk,
        handeling = %h.naam,
        stage = genomen.gram.stage.as_deref().unwrap_or("-"),
        "handeling vastgelegd"
    );
    Ok((StatusCode::CREATED, Json(genomen)))
}

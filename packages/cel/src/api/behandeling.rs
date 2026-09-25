//! De behandeling in een proces: de werkvoorraad, een zaak, het
//! proefbesluit en het besluit, voor de ingelogde behandelaar.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::sessie::behandelaar;
use super::{fout, intern, van_cel, Fout, ProcesState};
use crate::besluit::{self, Weigering};
use crate::celclient::{self, MetYaml};
use crate::datum;
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
        .haal(&synthese::pad(&w.cel, &w.lexostatus, &Map::new()))
        .await
        .map_err(van_cel)?;
    Ok(Json(v))
}

/// De datum van vandaag volgens de klok van de runtime: de peildatum van een
/// proefbesluit.
fn vandaag(state: &ProcesState) -> String {
    datum::peildatum(&(state.klok)())
}

#[derive(Deserialize, Default)]
pub(super) struct Besluitformulier {
    #[serde(default)]
    formulier: Map<String, Value>,
}

async fn proefbesluit_voor(
    state: &ProcesState,
    zaakkenmerk: &str,
    formulier: &Map<String, Value>,
) -> Result<besluit::Proefbesluit, Fout> {
    besluit::proefbesluit(
        &state.proces,
        state.cel.as_ref(),
        &state.bronnen,
        &state.rijen,
        zaakkenmerk,
        formulier,
        &vandaag(state),
    )
    .await
    .map_err(weigering)
}

/// Een weigering van het besluit als HTTP-antwoord. Een vraag die niet kan
/// (nog niet compleet, al besloten, een ander bevoegd gezag) is een 409: de
/// stand van de zaak laat het niet toe.
fn weigering(w: Weigering) -> Fout {
    match w {
        Weigering::OnbekendOordeel(t) => fout(StatusCode::BAD_REQUEST, t),
        Weigering::NietTeNemen(t) | Weigering::AlBesloten(t) | Weigering::Onbevoegd(t) => {
            fout(StatusCode::CONFLICT, t)
        }
        Weigering::Cel(t) => fout(StatusCode::INTERNAL_SERVER_ERROR, t),
    }
}

/// Het besluit nemen en laten vastleggen. Is het proefbesluit niet compleet,
/// ligt er al een besluit, of wijst de wet een ander bevoegd gezag aan, dan
/// komt er geen gram.
pub(super) async fn besluit_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path(zaakkenmerk): Path<String>,
    Json(invoer): Json<Besluitformulier>,
) -> Result<(StatusCode, Json<Value>), Fout> {
    behandelaar(&state, &headers)?;
    zaakgrammen(&state, &zaakkenmerk).await?;
    let genomen = besluit::neem_besluit(
        &state.proces,
        state.cel.as_ref(),
        &state.bronnen,
        &state.rijen,
        &state.regelingen,
        &zaakkenmerk,
        &invoer.formulier,
        (state.klok)(),
    )
    .await
    .map_err(weigering)?;
    tracing::info!(
        proces = %state.proces.id(),
        zaakkenmerk = %zaakkenmerk,
        stage = genomen.gram.stage.as_deref().unwrap_or("-"),
        "besluit vastgelegd"
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "gram": genomen.gram,
            "yaml": genomen.yaml,
            "proefbesluit": genomen.proefbesluit,
            "waarschuwingen": genomen.waarschuwingen,
        })),
    ))
}

/// De grammen van een zaak, elk met YAML, zoals de cel ze geeft; een 404
/// als de cel de zaak niet kent.
async fn zaakgrammen(state: &ProcesState, zaakkenmerk: &str) -> Result<Vec<MetYaml>, Fout> {
    celclient::lees_zaak(state.cel.as_ref(), state.cel_id(), zaakkenmerk)
        .await
        .map_err(van_cel)
}

pub(super) async fn zaak_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path(zaakkenmerk): Path<String>,
) -> Result<Json<Value>, Fout> {
    behandelaar(&state, &headers)?;
    let grammen = zaakgrammen(&state, &zaakkenmerk).await?;
    let b = &behandeling(&state)?.besluit;
    let proef = proefbesluit_voor(&state, &zaakkenmerk, &Map::new()).await?;
    Ok(Json(json!({
        "zaakkenmerk": zaakkenmerk,
        "grammen": grammen,
        "besluit": {
            "regeling": b.regeling,
            "artikel": proef.artikel,
            "uitkomsten": b.uitkomsten,
            "formulier": besluit::formuliervelden(&state.proces.service, b).map_err(intern)?,
            "stand_bij_besluit": b.stand_bij_besluit,
        },
        "proefbesluit": proef,
    })))
}

pub(super) async fn proefbesluit_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Path(zaakkenmerk): Path<String>,
    Json(invoer): Json<Besluitformulier>,
) -> Result<Json<besluit::Proefbesluit>, Fout> {
    behandelaar(&state, &headers)?;
    zaakgrammen(&state, &zaakkenmerk).await?;
    proefbesluit_voor(&state, &zaakkenmerk, &invoer.formulier)
        .await
        .map(Json)
}

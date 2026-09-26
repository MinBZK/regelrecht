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
use super::{fout, van_cel, Fout, ProcesState};
use crate::celclient;
use crate::config::HandelingDefinitie;
use crate::handeling::{self, Omgeving, Opgave, Weigering};
use crate::reductie::Peil;
use crate::reductie::Zaakstand;
use crate::rijen::Rijen;
use crate::synthese::{self, Bron};
use crate::transport::{Onthouden, Transport};
use chrono::{DateTime, FixedOffset};

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
/// zaak niet kent. Alleen voor inzage in het dossier: het proces leidt er
/// niets uit af.
async fn zaakgrammen(
    state: &ProcesState,
    zaakkenmerk: &str,
) -> Result<Vec<celclient::MetYaml>, Fout> {
    celclient::lees_zaak(state.cel.as_ref(), state.cel_id(), zaakkenmerk)
        .await
        .map_err(van_cel)
}

/// De stand van een zaak, zoals de cel haar afleidt; een 404 als de cel de
/// zaak niet kent.
async fn zaakstand(state: &ProcesState, zaakkenmerk: &str) -> Result<Zaakstand, Fout> {
    celclient::zaakstand(state.cel.as_ref(), state.cel_id(), zaakkenmerk, None)
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
    omgeving_met(
        state,
        state.cel.as_ref(),
        &h.bronnen,
        &h.rijen,
        (state.klok)(),
    )
}

/// De omgeving van een handeling met een gegeven transport naar de cel en
/// gegeven bronnen (zoals die het zaakscherm deelt).
fn omgeving_met<'a>(
    state: &'a ProcesState,
    cel: &'a dyn Transport,
    bronnen: &'a [Bron],
    rijen: &'a [Rijen],
    nu: DateTime<FixedOffset>,
) -> Omgeving<'a> {
    Omgeving {
        proces: &state.proces,
        cel,
        bronnen,
        rijen,
        regelingen: &state.regelingen,
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
    Path(zaakkenmerk): Path<String>,
) -> Result<Json<Value>, Fout> {
    behandelaar(&state, &headers)?;
    let zaak = zaakstand(&state, &zaakkenmerk).await?;
    let grammen = zaakgrammen(&state, &zaakkenmerk).await?;
    let b = behandeling(&state)?;
    let leeg = Opgave::default();
    // De zaakcontext (de lexostatussen van de zaak, de synthese en de
    // synthese per regel) is voor elke handeling op dezelfde peildatum
    // dezelfde: de proeven delen een geheugen voor wat ze lezen, zodat het
    // proces elke lexostatus en elke bron een keer vraagt. Een feit dat op
    // proef als concept meetelt, reduceert de cel per handeling.
    let geheugen = Onthouden::default();
    let cel = geheugen.om(state.cel.clone());
    let gedeeld: Vec<(Vec<Bron>, Vec<Rijen>)> = state
        .handelingen
        .iter()
        .map(|hs| {
            let bronnen = hs
                .bronnen
                .iter()
                .map(|b| b.langs(|t| geheugen.om(t)))
                .collect();
            let rijen = hs
                .rijen
                .iter()
                .map(|r| Rijen {
                    definitie: r.definitie.clone(),
                    bronnen: r
                        .bronnen
                        .iter()
                        .map(|b| b.langs(|t| geheugen.om(t)))
                        .collect(),
                })
                .collect();
            (bronnen, rijen)
        })
        .collect();
    let nu = (state.klok)();
    let proeven = join_all(b.handelingen.iter().enumerate().map(|(i, h)| {
        let om = omgeving_met(&state, cel.as_ref(), &gedeeld[i].0, &gedeeld[i].1, nu);
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
        // De controle bij het opstarten las dit al; een fout hier is er een
        // van de runtime.
        let benodigd = handeling::benodigd(&state.proces.service, h)
            .map_err(|f| fout(StatusCode::INTERNAL_SERVER_ERROR, f))?;
        let proef = match proef {
            None => Value::Null,
            Some(Ok(p)) => json!(p),
            Some(Err(w)) => json!({"fout": weigering_tekst(&w)}),
        };
        let formulier: Vec<Value> = h
            .oordelen
            .iter()
            .map(|o| {
                let typering = benodigd.get(&o.parameter).map(|b| &b.typering);
                let mut v = json!({
                    "naam": o.parameter,
                    "label": o.label,
                    "type": typering.map(|t| handeling::veldsoort(t.soort)),
                    "groep": o.groep,
                    "soort": "oordeel",
                });
                if let Some(e) = typering.and_then(|t| t.eenheid.as_deref()) {
                    v["eenheid"] = json!(e);
                }
                v
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
            "typen": h.typen,
            "haken": h.haken,
            "nog_niet": h.nog_niet,
            "formulier": formulier,
            "beschikbaar": stand.beschikbaar,
            "reden": stand.reden,
            "vastgelegd": stand.vastgelegd,
            "besluit": stand.besluit,
            "besluitrol": h.besluitrol,
            "proef": proef,
        }));
    }
    Ok(Json(json!({
        "zaakkenmerk": zaakkenmerk,
        "grammen": grammen,
        "procedure": handeling::procedure_van_de_zaak(&state.proces, &zaak),
        "besluiten": handeling::besluiten_in_zaak(&state.proces, &zaak),
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
    Json(opgave): Json<Opgave>,
) -> Result<Json<handeling::Proefhandeling>, Fout> {
    let (i, h) = handeling_met(&state, &naam)?;
    voor_handeling(&state, &headers, h.rol.as_deref())?;
    let zaak = zaakstand(&state, &zaakkenmerk).await?;
    handeling::proef(&omgeving(&state, i), h, &zaakkenmerk, &zaak, &opgave)
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
    Path((zaakkenmerk, naam)): Path<(String, String)>,
    Json(opgave): Json<Opgave>,
) -> Result<(StatusCode, Json<handeling::Genomen>), Fout> {
    let (i, h) = handeling_met(&state, &naam)?;
    let wie = voor_handeling(&state, &headers, h.rol.as_deref())?;
    let zaak = zaakstand(&state, &zaakkenmerk).await?;
    let genomen = handeling::neem(&omgeving(&state, i), h, &zaakkenmerk, &zaak, &opgave, &wie)
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

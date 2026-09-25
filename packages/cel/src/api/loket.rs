//! Het loket van een proces: een aanvraag die langs een andere weg binnenkwam
//! (op papier, aan de balie), ingevoerd door een medewerker namens de
//! aanvrager, met de dag van ontvangst.
//!
//! Rechtens telt die dag (Awb 4:1, 4:13: de beslistermijn loopt vanaf de
//! ontvangst), niet de dag van invoeren. Het event van het portaal bindt zijn
//! `op_moment` daarom aan een pad onder `$intake`; het loket vult dat pad, het
//! portaal nooit, zodat een aanvrager zijn ontvangst niet zelf kan kiezen. De
//! cel legt vast als bij het portaal, met twee tijden: `op_moment` de
//! ontvangst, `vastgelegd_op` het invoeren.
//!
//! Het loket weigert een ontvangst na vandaag (wat nog moet gebeuren, is geen
//! feit; de cel weigert het ook), en een ontvangst van vóór de openstelling
//! van het tijdvak, als het beleid die noemt (`portaal.aanbod.openstelling`).

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::sessie::met_routes;
use super::{fout, intern, van_cel, Fout, ProcesState};
use crate::celclient::{self, MetYaml, Vastlegverzoek};
use crate::datum::{self, Tijdpunt};
use crate::kanaal::{self, Routes};
use crate::mogelijkheid::{self, Keuze};

#[derive(Deserialize)]
pub(super) struct Loketinvoer {
    /// Wie de aanvraag deed: de velden van een portaalkanaal, en `kanaal`
    /// als het portaal er meer dan een heeft. Niemand logde in: het loket
    /// neemt over wat op de aanvraag staat.
    aanvrager: Map<String, Value>,
    /// De dag (`JJJJ-MM-DD`) of het moment van ontvangst.
    ontvangen_op: String,
    #[serde(default)]
    external: Map<String, Value>,
}

/// `POST /api/loket/aanvraag`: de cel legt de aanvraag vast in het event van
/// het portaal, met de ontvangst als `op_moment`.
pub(super) async fn loket_indienen(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Json(invoer): Json<Loketinvoer>,
) -> Result<(StatusCode, Json<MetYaml>), Fout> {
    let wie = met_routes(&state, &headers, Routes::Loket)?;
    let d = &state.proces.definitie;
    let (stroom, event) = state
        .proces
        .portaal_event()
        .ok_or_else(|| intern("geen portaal geconfigureerd"))?;
    let pad = kanaal::ontvangstpad(event)
        .ok_or_else(|| intern("het portaal-event bindt op_moment niet aan $intake"))?;

    // De aanvrager, aangeduid met de velden van een portaalkanaal.
    let kanalen = d.kanalen_met(Routes::Portaal);
    let gevraagd = invoer.aanvrager.get("kanaal").and_then(Value::as_str);
    let (kid, k) = match (gevraagd, kanalen.as_slice()) {
        (Some(g), _) => kanalen
            .iter()
            .find(|(id, _)| *id == g)
            .copied()
            .ok_or_else(|| {
                fout(
                    StatusCode::BAD_REQUEST,
                    format!("aanvrager: '{g}' is geen kanaal van het portaal"),
                )
            })?,
        (None, [een]) => *een,
        (None, _) => {
            return Err(fout(
                StatusCode::BAD_REQUEST,
                format!(
                    "aanvrager: noem het kanaal ({})",
                    kanalen
                        .iter()
                        .map(|(id, _)| *id)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ))
        }
    };
    let velden = k
        .valideer(&invoer.aanvrager)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, format!("aanvrager: {e}")))?;

    // De ontvangst: niet na vandaag, en niet vóór de openstelling.
    let ontvangst = Tijdpunt::lees("ontvangen_op", &invoer.ontvangen_op)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let nu = (state.klok)();
    let dag = ontvangst.als_moment(*nu.offset()).date_naive();
    if dag > nu.date_naive() {
        return Err(fout(
            StatusCode::BAD_REQUEST,
            format!(
                "ontvangen_op {} ligt na vandaag ({}): een ontvangst die nog moet komen, wordt niet ingevoerd",
                invoer.ontvangen_op,
                nu.date_naive()
            ),
        ));
    }
    if let Some(open) = openstelling(&state, &invoer.external, &nu)? {
        if dag < open {
            return Err(fout(
                StatusCode::BAD_REQUEST,
                format!(
                    "ontvangen_op {} ligt vóór de openstelling van het tijdvak ({open})",
                    invoer.ontvangen_op
                ),
            ));
        }
    }

    let mut intake = kanaal::intake("loket", kanalen.iter().copied(), Some((kid, &velden)));
    if let Value::Object(m) = &mut intake {
        kanaal::zet_pad(m, &pad, Value::String(invoer.ontvangen_op.clone()));
        m.insert(
            "ingevoerd_door".into(),
            json!({"rol": wie.rol, "kanaal": wie.kanaal, "identiteit": wie.velden}),
        );
    }
    let verzoek = Vastlegverzoek {
        actor: d.actor.clone(),
        stroom: stroom.id.clone(),
        event: event.name.clone(),
        intake,
        external: invoer.external,
        zaakkenmerk: None,
        besluit: None,
    };
    let vastgelegd = celclient::leg_vast(state.cel.as_ref(), state.cel_id(), &verzoek)
        .await
        .map_err(van_cel)?;
    Ok((StatusCode::CREATED, Json(vastgelegd)))
}

/// De eerste dag waarop een aanvraag voor het gekozen tijdvak kan
/// binnenkomen, als het beleid die noemt (`aanbod.openstelling`, met het
/// tijdvak als parameter). Het tijdvak komt uit het veld van het concept
/// waaruit de toets het afleidt; zonder dat veld is de openstelling niet te
/// toetsen, en dat is een fout van de invoer.
fn openstelling(
    state: &ProcesState,
    external: &Map<String, Value>,
    nu: &chrono::DateTime<chrono::FixedOffset>,
) -> Result<Option<chrono::NaiveDate>, Fout> {
    let Some(a) = state.proces.portaal().and_then(|p| p.aanbod.as_ref()) else {
        return Ok(None);
    };
    let Some(uitkomst) = &a.openstelling else {
        return Ok(None);
    };
    let tijdvak = state
        .proces
        .tijdvak
        .as_ref()
        .ok_or_else(|| intern("aanbod.openstelling zonder tijdvak"))?;
    let waarde = tijdvak
        .veld
        .as_ref()
        .and_then(|v| external.get(v))
        .filter(|w| !w.is_null())
        .ok_or_else(|| {
            fout(
                StatusCode::BAD_REQUEST,
                format!(
                    "het tijdvak ({}) ontbreekt; zonder tijdvak is de openstelling niet te toetsen",
                    tijdvak.veld.as_deref().unwrap_or(&tijdvak.parameter)
                ),
            )
        })?;
    let keuze = Keuze {
        parameter: tijdvak.parameter.clone(),
        veld: tijdvak.veld.clone(),
        waarde: waarde.clone(),
    };
    mogelijkheid::begin(
        &state.proces.service,
        &a.regeling,
        uitkomst,
        &keuze,
        &datum::peildatum(nu),
    )
    .map(Some)
    .map_err(intern)
}

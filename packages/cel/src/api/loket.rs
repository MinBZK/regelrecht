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
use super::{error, intern, van_cel, Error, ProcesState};
use crate::celclient::{self, MetYaml, Vastlegverzoek};
use crate::datum::{self, Tijdpunt};
use crate::kanaal::{self, Routes};
use crate::mogelijkheid::{self, Choice};

#[derive(Deserialize)]
pub(super) struct Loketinvoer {
    /// Wie de aanvraag deed: de velden van een portaalkanaal, en `kanaal`
    /// als het portaal er meer dan een heeft. Niemand logde in: het loket
    /// neemt over wat op de aanvraag staat.
    applicant: Map<String, Value>,
    /// De dag (`JJJJ-MM-DD`) of het moment van ontvangst.
    received_at: String,
    #[serde(default)]
    external: Map<String, Value>,
}

/// `POST /api/loket/aanvraag`: de cel legt de aanvraag vast in het event van
/// het portaal, met de ontvangst als `op_moment`.
pub(super) async fn loket_indienen(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Json(input): Json<Loketinvoer>,
) -> Result<(StatusCode, Json<MetYaml>), Error> {
    let wie = met_routes(&state, &headers, Routes::Counter)?;
    let d = &state.proces.definitie;
    let (stream, event) = state
        .proces
        .portaal_event()
        .ok_or_else(|| intern("geen portaal geconfigureerd"))?;
    let path = kanaal::ontvangstpad(event)
        .ok_or_else(|| intern("het portaal-event bindt op_moment niet aan $intake"))?;

    // De aanvrager, aangeduid met de velden van een portaalkanaal.
    let channels = d.kanalen_met(Routes::Portal);
    let gevraagd = input.applicant.get("channel").and_then(Value::as_str);
    let (kid, k) = match (gevraagd, channels.as_slice()) {
        (Some(g), _) => channels
            .iter()
            .find(|(id, _)| *id == g)
            .copied()
            .ok_or_else(|| {
                error(
                    StatusCode::BAD_REQUEST,
                    format!("aanvrager: '{g}' is geen kanaal van het portaal"),
                )
            })?,
        (None, [een]) => *een,
        (None, _) => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!(
                    "aanvrager: noem het kanaal ({})",
                    channels
                        .iter()
                        .map(|(id, _)| *id)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ))
        }
    };
    let fields = k
        .valideer(&input.applicant)
        .map_err(|e| error(StatusCode::BAD_REQUEST, format!("aanvrager: {e}")))?;

    // De ontvangst: niet na vandaag, en niet vóór de openstelling.
    let ontvangst = Tijdpunt::lees("ontvangen_op", &input.received_at)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let nu = (state.klok)();
    let dag = ontvangst.als_moment(*nu.offset()).date_naive();
    if dag > nu.date_naive() {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!(
                "ontvangen_op {} ligt na vandaag ({}): een ontvangst die nog moet komen, wordt niet ingevoerd",
                input.received_at,
                nu.date_naive()
            ),
        ));
    }
    if let Some(open) = opening(&state, &input.external, &nu)? {
        if dag < open {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!(
                    "ontvangen_op {} ligt vóór de openstelling van het tijdvak ({open})",
                    input.received_at
                ),
            ));
        }
    }

    let mut intake = kanaal::intake("counter", channels.iter().copied(), Some((kid, &fields)));
    if let Value::Object(m) = &mut intake {
        crate::gram::zet_pad(m, &path, Value::String(input.received_at.clone()));
        m.insert(
            "ingevoerd_door".into(),
            json!({"role": wie.role, "channel": wie.channel, "identity": wie.fields}),
        );
    }
    let verzoek = Vastlegverzoek {
        actor: d.actor.clone(),
        stream: stream.id.clone(),
        event: event.name.clone(),
        intake,
        external: input.external,
        refers_to: Default::default(),
        decision: None,
        root_grams: None,
    };
    let recorded = celclient::leg_vast(state.cell.as_ref(), state.cel_id(), &verzoek)
        .await
        .map_err(van_cel)?;
    Ok((StatusCode::CREATED, Json(recorded)))
}

/// De eerste dag waarop een aanvraag voor het gekozen tijdvak kan
/// binnenkomen, als het beleid die noemt (`aanbod.openstelling`, met het
/// tijdvak als parameter). Het tijdvak komt uit het veld van het concept
/// waaruit de toets het afleidt; zonder dat veld is de openstelling niet te
/// toetsen, en dat is een fout van de invoer.
fn opening(
    state: &ProcesState,
    external: &Map<String, Value>,
    nu: &chrono::DateTime<chrono::FixedOffset>,
) -> Result<Option<chrono::NaiveDate>, Error> {
    let Some(a) = state.proces.portal().and_then(|p| p.offer.as_ref()) else {
        return Ok(None);
    };
    let Some(output) = &a.opening else {
        return Ok(None);
    };
    let window = state
        .proces
        .window
        .as_ref()
        .ok_or_else(|| intern("aanbod.openstelling zonder tijdvak"))?;
    let value = window
        .field
        .as_ref()
        .and_then(|v| external.get(v))
        .filter(|w| !w.is_null())
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                format!(
                    "het tijdvak ({}) ontbreekt; zonder tijdvak is de openstelling niet te toetsen",
                    window.field.as_deref().unwrap_or(&window.parameter)
                ),
            )
        })?;
    let keuze = Choice {
        parameter: window.parameter.clone(),
        field: window.field.clone(),
        value: value.clone(),
    };
    mogelijkheid::start(
        &state.proces.service,
        &a.regulation,
        output,
        &keuze,
        &datum::reference_date(nu),
    )
    .map(Some)
    .map_err(intern)
}

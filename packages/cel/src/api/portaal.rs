//! Het portaal van een proces: het formulier, de toets, het aanbod en het
//! indienen, voor de ingelogde aanvrager.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::sessie::ingelogd;
use super::{error, intern, van_cel, Error, ProcesState};
use crate::celclient::{self, Vastlegverzoek};
use crate::datum::{self, Tijdpunt};
use crate::gram::Gram;
use crate::kanaal::{self, Routes, Sessie};
use crate::mogelijkheid;
use crate::reductie::{self, Lexostatus, Peil};
use crate::rijen;
use crate::stroom;
use crate::synthese;
use crate::toets;
use crate::transport::TransportFout;

fn portaal_event(state: &ProcesState) -> Result<(&stroom::Stroom, &stroom::Event), Error> {
    state.proces.portaal_event().ok_or_else(|| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen portaal geconfigureerd",
        )
    })
}

/// De velden van het aanvraagformulier: de `$external`-velden van het event
/// in de stroom van de cel, met labels en volgorde uit het formulier van het
/// proces.
pub(super) async fn formulier_route(
    State(state): State<ProcesState>,
) -> Result<Json<Value>, Error> {
    let (stream, event) = portaal_event(&state)?;
    let form = state.proces.form.as_ref();
    let fields = crate::formulier::fields(event, form).map_err(intern)?;
    Ok(Json(json!({
        "cell": state.cel_id(),
        "stream": stream.document,
        "event": event.name,
        "title": form.and_then(|f| f.title.clone()),
        "fields": fields,
    })))
}

#[derive(Deserialize)]
pub(super) struct Concept {
    #[serde(default)]
    external: Map<String, Value>,
    /// Alleen bij een event met verwijzingen: per naam het id van het gram
    /// waarnaar het nieuwe gram verwijst.
    #[serde(default)]
    refers_to: std::collections::BTreeMap<String, String>,
}

/// Het eigenaarpad van het kanaal van de gebruiker (`kanalen.<id>.eigenaar`,
/// onder `$intake`) en zijn waarde daar. Een kanaal zonder eigenaar maakt
/// niemand eigenaar.
fn eigenaar_van<'s>(state: &ProcesState, session: &'s Sessie) -> Option<(String, &'s str)> {
    let k = state.proces.definitie.channels.get(&session.channel)?;
    let path = k.eigenaar_pad(&session.channel)?;
    let value = session.fields.get(k.owner.as_ref()?)?;
    Some((path, value.as_str()))
}

/// Het verzoek aan de cel voor een concept van de aanvrager. Verwijst het
/// event naar een ander gram, dan moet de aanvrager de groep van dat gram
/// kennen (het gram waarnaar hij verwijst is dan zelf een wortel, zoals de
/// aanvraag). Of hij dat doet, zegt de cel: haar
/// [`crate::reductie::Zaakstand`] leidt af of er een gram in de groep ligt
/// waarvan het veld dat aan zijn eigenaarpad bindt, zijn waarde heeft. Het
/// proces leest daarvoor geen grammen. De cel controleert de rest.
async fn verzoek_voor(
    state: &ProcesState,
    session: &Sessie,
    concept: &Concept,
) -> Result<Vastlegverzoek, Error> {
    let (stream, event) = portaal_event(state)?;
    for z in concept.refers_to.values() {
        let bekend = match eigenaar_van(state, session) {
            None => false,
            Some((path, value)) => {
                // Een zaak die de cel niet kent, kent de aanvrager ook niet.
                match celclient::zaakstand(
                    state.cell.as_ref(),
                    state.cel_id(),
                    z,
                    Some((&path, value)),
                )
                .await
                {
                    Err(TransportFout::Antwoord { status: 404, .. }) => false,
                    anders => anders.map_err(van_cel)?.owner == Some(true),
                }
            }
        };
        if !bekend {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!("geen wortel '{z}' in de kroniek die u kent"),
            ));
        }
    }
    Ok(Vastlegverzoek {
        actor: state.proces.definitie.actor.clone(),
        stream: stream.id.clone(),
        event: event.name.clone(),
        intake: kanaal::intake(
            &event.intake,
            state.proces.definitie.kanalen_met(Routes::Portal),
            Some((&session.channel, &session.fields)),
        ),
        external: concept.external.clone(),
        refers_to: concept.refers_to.clone(),
        decision: None,
        root_grams: None,
    })
}

/// Een concept, op proef gereduceerd in de cel tot de toets-lexostatus en
/// samengevoegd met de bronnen. Een concept is geen feit: niets hiervan wordt
/// vastgelegd. Gedeeld door de toets en de aanvraagmogelijkheden.
struct Concepttoets<'a> {
    portal: &'a crate::config::Portal,
    def: &'a reductie::LexostatusDefinitie,
    gram: Gram,
    lexostatus: Lexostatus,
    combined: synthese::Samenvoeging,
    /// Wat de synthese per regel van de toets opleverde.
    rows: Vec<rijen::Uitslag>,
}

/// `peil`: waarop de bronnen hun kroniek reduceren (zie [`Peil`]); het
/// concept zelf reduceert de cel zoals het nu zou vastliggen.
async fn concepttoets<'a>(
    state: &'a ProcesState,
    session: &Sessie,
    concept: &Concept,
    peil: &Peil,
) -> Result<Concepttoets<'a>, Error> {
    let portal = state.proces.portal().ok_or_else(|| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen portaal geconfigureerd",
        )
    })?;
    let def = state
        .proces
        .cell
        .lexostatuses
        .lexostatus(&portal.assessment.lexostatus)
        .ok_or_else(|| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "toets-lexostatus ontbreekt",
            )
        })?;
    let verzoek = verzoek_voor(state, session, concept).await?;
    let celclient::Proefreductie { gram, lexostatus } = celclient::trial(
        state.cell.as_ref(),
        state.cel_id(),
        &portal.assessment.lexostatus,
        &verzoek,
        &serde_json::Map::new(),
    )
    .await
    .map_err(van_cel)?;
    // Synthese: de lexostatus van het concept plus die van de bronnen, en
    // daarna de synthese per regel, vóór de engine. Een invoer uit de wet
    // leest de regeling op de dag van het concept, zoals de toets.
    let mut combined = synthese::voeg_samen(&lexostatus, &state.sources, peil).await;
    let date = datum::peildatum_van(&gram.effective_at).map_err(intern)?;
    let law = rijen::Omgeving {
        service: &state.proces.service,
        date: &date,
        peil,
    };
    let rows = rijen::pas_toe(
        &state.toets_rijen,
        std::slice::from_ref(&lexostatus),
        &mut combined,
        law,
    )
    .await;
    Ok(Concepttoets {
        portal,
        def,
        gram,
        lexostatus,
        combined,
        rows,
    })
}

pub(super) async fn toets_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<Json<Value>, Error> {
    let session = ingelogd(&state, &headers)?;
    // De bronnen peilen op vandaag, de dag waarop de engine de wet leest.
    let vandaag = Peil::op(Tijdpunt::Datum((state.klok)().date_naive()));
    let c = concepttoets(&state, &session, &concept, &vandaag).await?;
    let date = datum::peildatum_van(&c.gram.effective_at).map_err(intern)?;
    let mut result = toets::assessment(
        &state.proces.service,
        &c.portal.assessment.regulation,
        &c.portal.assessment.output,
        &c.combined.parameters,
        reductie::absent(c.def, &c.lexostatus.parameters),
        &date,
    );
    if !result.to_assess {
        if let Some(reason) = c.combined.reason() {
            result.reason = Some(reason);
        }
    }
    Ok(Json(json!({
        "result": result,
        "lexostatus": c.lexostatus,
        // Wat naar de engine ging, en per parameter waar het vandaan kwam.
        "parameters": c.combined.parameters,
        "provenance": c.combined.provenance,
        "sources": c.combined.sources,
        "rows": c.rows,
    })))
}

/// Wat het beleid de ingelogde persoon aanbiedt (`portaal.aanbod`), per
/// tijdvak dat het beleid aanbiedt (`aanbod.tijdvakken`, een uitkomst van
/// dezelfde regeling, uitgerekend op de datum van vandaag). Het tijdvak is de
/// parameter van het aanbod-artikel met origin BELANGHEBBENDE en grondslag
/// Awb 4:2 lid 1; het beleid wordt uitgevoerd op een concept met alleen dat
/// tijdvak, plus wat het kanaal en de synthese weten. Uitkomst en termijn komen uit een run,
/// met trace. Een feit dat een bron niet leverde, maakt het aanbod niet te
/// bepalen. Niets wordt vastgelegd.
///
/// De bronnen peilen per tijdvak (zie [`peil_voor`]): voor een tijdvak dat
/// nog moet beginnen op de eerste dag ervan, zodat een feit dat vóór dat
/// tijdvak ingaat (een schrapping per 1 januari) meetelt en de registers niet
/// de stand van vandaag geven voor een jaar dat nog komt.
pub(super) async fn mogelijkheden_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let session = ingelogd(&state, &headers)?;
    let c0 = state
        .proces
        .portal()
        .and_then(|p| p.offer.clone())
        .ok_or_else(|| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "geen aanbod geconfigureerd",
            )
        })?;
    let nu = (state.klok)();
    let date = datum::reference_date(&nu);
    // Zonder tijdvak een run; met tijdvak een run per tijdvak dat het beleid
    // aanbiedt.
    let keuzes: Vec<Option<mogelijkheid::Choice>> = match (&state.proces.window, &c0.windows) {
        (Some(t), Some(u)) => {
            mogelijkheid::windows(&state.proces.service, &c0.regulation, u, &date)
                .map_err(intern)?
                .into_iter()
                .map(|w| {
                    Some(mogelijkheid::Choice {
                        parameter: t.parameter.clone(),
                        field: t.field.clone(),
                        value: w,
                    })
                })
                .collect()
        }
        _ => vec![None],
    };
    let mut uit = Vec::new();
    for keuze in keuzes {
        let mut external = Map::new();
        if let Some(k) = &keuze {
            if let Some(field) = &k.field {
                external.insert(field.clone(), k.value.clone());
            }
        }
        let concept = Concept {
            external,
            refers_to: Default::default(),
        };
        let start = match (&keuze, &c0.start) {
            (Some(k), Some(u)) => Some(
                mogelijkheid::start(&state.proces.service, &c0.regulation, u, k, &date)
                    .map_err(intern)?,
            ),
            _ => None,
        };
        let peil = peil_voor(&nu, start);
        let mut c = concepttoets(&state, &session, &concept, &peil).await?;
        // Leidt de toets-lexostatus het tijdvak niet af, dan gaat de keuze
        // zelf mee.
        if let Some(k) = &keuze {
            if !c.combined.parameters.contains_key(&k.parameter) {
                c.combined
                    .parameters
                    .insert(k.parameter.clone(), k.value.clone());
                c.combined
                    .provenance
                    .insert(k.parameter.clone(), synthese::Herkomst::Choice);
            }
        }
        let m = mogelijkheid::bepaal(
            &state.proces.service,
            keuze,
            &c0,
            &c.combined.parameters,
            &date,
        );
        uit.push(json!({
            "possibility": m,
            "as_of": peil.as_of.map(|t| t.to_string()),
            "parameters": c.combined.parameters,
            "provenance": c.combined.provenance,
            "sources": c.combined.sources,
        }));
    }
    Ok(Json(json!({
        "session": session,
        // De datum van de runtime, zodat de frontend "verstreken" niet op de
        // klok van de browser beoordeelt.
        "date": date,
        "possibilities": uit,
    })))
}

/// Het peil van de bronnen voor een aanbod: het begin van het gekozen
/// tijdvak als dat nog moet beginnen, anders vandaag. Het begin zegt het
/// beleid (`aanbod.begin`, zie [`mogelijkheid::begin`]); zonder begin, of
/// zonder tijdvak, peilt het aanbod op vandaag.
fn peil_voor(nu: &chrono::DateTime<chrono::FixedOffset>, start: Option<chrono::NaiveDate>) -> Peil {
    let vandaag = nu.date_naive();
    Peil::op(Tijdpunt::Datum(
        start.filter(|b| *b > vandaag).unwrap_or(vandaag),
    ))
}

/// Indienen: de cel bouwt het gram en legt het vast.
pub(super) async fn indienen(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<(StatusCode, Json<celclient::MetYaml>), Error> {
    let session = ingelogd(&state, &headers)?;
    let verzoek = verzoek_voor(&state, &session, &concept).await?;
    let recorded = celclient::leg_vast(state.cell.as_ref(), state.cel_id(), &verzoek)
        .await
        .map_err(van_cel)?;
    Ok((StatusCode::CREATED, Json(recorded)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Een tijdvak dat nog moet beginnen, peilt op zijn begin; een begin dat
    /// al voorbij is, en geen begin, op vandaag.
    #[test]
    fn het_aanbod_peilt_op_het_begin_van_een_komend_tijdvak() {
        let nu = datum::moment("2026-09-25T10:00:00+02:00").unwrap();
        let op = |b: Option<&str>| {
            let b = b.map(|b| chrono::NaiveDate::parse_from_str(b, "%Y-%m-%d").unwrap());
            peil_voor(&nu, b).as_of.unwrap().to_string()
        };
        assert_eq!(op(Some("2027-01-01")), "2027-01-01");
        assert_eq!(op(Some("2026-01-01")), "2026-09-25");
        assert_eq!(op(None), "2026-09-25");
    }
}

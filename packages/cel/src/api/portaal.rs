//! Het portaal van een proces: het formulier, de toets, het aanbod en het
//! indienen, voor de ingelogde aanvrager.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::sessie::ingelogd;
use super::{fout, intern, van_cel, Fout, ProcesState};
use crate::cel::Cel;
use crate::celclient::{self, Vastlegverzoek};
use crate::datum::{self, Tijdpunt};
use crate::eherkenning::Sessie;
use crate::gram::Gram;
use crate::mogelijkheid;
use crate::reductie::{self, Lexostatus, Peil};
use crate::rijen;
use crate::stroom::{self, Binding, Zaak};
use crate::synthese;
use crate::toets;
use crate::transport::TransportFout;

fn portaal_event(state: &ProcesState) -> Result<(&stroom::Stroom, &stroom::Event), Fout> {
    state.proces.portaal_event().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen portaal geconfigureerd",
        )
    })
}

/// De velden van het aanvraagformulier: de `$external`-velden van het event
/// in de stroom van de cel, met labels en volgorde uit het formulier van het
/// proces.
pub(super) async fn formulier_route(State(state): State<ProcesState>) -> Result<Json<Value>, Fout> {
    let (stroom, event) = portaal_event(&state)?;
    let formulier = state.proces.formulier.as_ref();
    let velden = crate::formulier::velden(event, formulier).map_err(intern)?;
    Ok(Json(json!({
        "cel": state.cel_id(),
        "stroom": stroom.document,
        "event": event.name,
        "titel": formulier.and_then(|f| f.titel.clone()),
        "velden": velden,
    })))
}

#[derive(Deserialize)]
pub(super) struct Concept {
    #[serde(default)]
    external: Map<String, Value>,
    /// Alleen bij een event met `zaak: volgt`: de zaak die het gram volgt.
    #[serde(default)]
    zaakkenmerk: Option<String>,
}

/// Of een gram van deze KvK is: het veld dat aan `$intake.eherkenning.kvk`
/// bindt, heeft dat nummer.
fn van_kvk(cel: &Cel, gram: &Gram, kvk: &str) -> bool {
    let Some((_, event)) = cel.event(&gram.stroom.id, &gram.name) else {
        return false;
    };
    event.bladeren().iter().any(|b| {
        b.binding == Binding::Intake("eherkenning.kvk".into())
            && gram.veld(&b.pad).and_then(Value::as_str) == Some(kvk)
    })
}

/// Het verzoek aan de cel voor een concept van de aanvrager. Volgt het event
/// een zaak, dan moet de aanvrager die zaak kennen: een gram van zijn KvK
/// met dat zaakkenmerk. De cel controleert de rest (zie
/// [`zaakkenmerk_voor`]).
async fn verzoek_voor(
    state: &ProcesState,
    sessie: &Sessie,
    concept: &Concept,
) -> Result<Vastlegverzoek, Fout> {
    let (stroom, event) = portaal_event(state)?;
    if let (Zaak::Volgt, Some(z)) = (event.zaak, &concept.zaakkenmerk) {
        // Een zaak die de cel niet kent, kent de aanvrager ook niet.
        let zaak = match celclient::lees_zaak(state.cel.as_ref(), state.cel_id(), z).await {
            Err(TransportFout::Antwoord { status: 404, .. }) => Vec::new(),
            anders => anders.map_err(van_cel)?,
        };
        let bekend = zaak
            .iter()
            .any(|g| van_kvk(&state.proces.cel, &g.gram, &sessie.kvk));
        if !bekend {
            return Err(fout(
                StatusCode::BAD_REQUEST,
                format!("geen zaak '{z}' in de kroniek"),
            ));
        }
    }
    Ok(Vastlegverzoek {
        actor: state.proces.definitie.actor.clone(),
        stroom: stroom.id.clone(),
        event: event.name.clone(),
        intake: sessie.intake(&event.intake),
        external: concept.external.clone(),
        zaakkenmerk: concept.zaakkenmerk.clone(),
        besluit: None,
    })
}

/// Een concept, op proef gereduceerd in de cel tot de toets-lexostatus en
/// samengevoegd met de bronnen. Een concept is geen feit: niets hiervan wordt
/// vastgelegd. Gedeeld door de toets en de aanvraagmogelijkheden.
struct Concepttoets<'a> {
    portaal: &'a crate::config::Portaal,
    def: &'a reductie::LexostatusDefinitie,
    gram: Gram,
    lexostatus: Lexostatus,
    samen: synthese::Samenvoeging,
    /// Wat de synthese per regel van de toets opleverde.
    rijen: Vec<rijen::Uitslag>,
}

/// `peil`: waarop de bronnen hun kroniek reduceren (zie [`Peil`]); het
/// concept zelf reduceert de cel zoals het nu zou vastliggen.
async fn concepttoets<'a>(
    state: &'a ProcesState,
    sessie: &Sessie,
    concept: &Concept,
    peil: &Peil,
) -> Result<Concepttoets<'a>, Fout> {
    let portaal = state.proces.portaal().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen portaal geconfigureerd",
        )
    })?;
    let def = state
        .proces
        .cel
        .lexostatussen
        .lexostatus(&portaal.toets.lexostatus)
        .ok_or_else(|| {
            fout(
                StatusCode::INTERNAL_SERVER_ERROR,
                "toets-lexostatus ontbreekt",
            )
        })?;
    let verzoek = verzoek_voor(state, sessie, concept).await?;
    let celclient::Proefreductie { gram, lexostatus } = celclient::proef(
        state.cel.as_ref(),
        state.cel_id(),
        &portaal.toets.lexostatus,
        &verzoek,
    )
    .await
    .map_err(van_cel)?;
    // Synthese: de lexostatus van het concept plus die van de bronnen, en
    // daarna de synthese per regel, vóór de engine. Een invoer uit de wet
    // leest de regeling op de dag van het concept, zoals de toets.
    let mut samen = synthese::voeg_samen(&lexostatus, &state.bronnen, peil).await;
    let datum = datum::peildatum_van(&gram.op_moment).map_err(intern)?;
    let wet = rijen::Omgeving {
        service: &state.proces.service,
        datum: &datum,
        peil,
    };
    let rijen = rijen::pas_toe(
        &state.toets_rijen,
        std::slice::from_ref(&lexostatus),
        &mut samen,
        wet,
    )
    .await;
    Ok(Concepttoets {
        portaal,
        def,
        gram,
        lexostatus,
        samen,
        rijen,
    })
}

pub(super) async fn toets_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<Json<Value>, Fout> {
    let sessie = ingelogd(&state, &headers)?;
    // De bronnen peilen op vandaag, de dag waarop de engine de wet leest.
    let vandaag = Peil::op(Tijdpunt::Datum((state.klok)().date_naive()));
    let c = concepttoets(&state, &sessie, &concept, &vandaag).await?;
    let datum = datum::peildatum_van(&c.gram.op_moment).map_err(intern)?;
    let mut uitslag = toets::toets(
        &state.proces.service,
        &c.portaal.toets.regeling,
        &c.portaal.toets.uitkomst,
        &c.samen.parameters,
        reductie::ontbreekt(c.def, &c.lexostatus.parameters),
        &datum,
    );
    if !uitslag.te_beoordelen {
        if let Some(reden) = c.samen.reden() {
            uitslag.reden = Some(reden);
        }
    }
    Ok(Json(json!({
        "uitslag": uitslag,
        "lexostatus": c.lexostatus,
        // Wat naar de engine ging, en per parameter waar het vandaan kwam.
        "parameters": c.samen.parameters,
        "herkomst": c.samen.herkomst,
        "bronnen": c.samen.bronnen,
        "rijen": c.rijen,
    })))
}

/// Wat het beleid de ingelogde persoon aanbiedt (`portaal.aanbod`), per
/// tijdvak dat het beleid aanbiedt (`aanbod.tijdvakken`, een uitkomst van
/// dezelfde regeling, uitgerekend op de datum van vandaag). Het tijdvak is de
/// parameter van het aanbod-artikel met origin BELANGHEBBENDE en grondslag
/// Awb 4:2 lid 1; het beleid wordt uitgevoerd op een concept met alleen dat
/// tijdvak, plus wat de eHerkenning en de synthese weten. Uitkomst en termijn komen uit een run,
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
) -> Result<Json<Value>, Fout> {
    let sessie = ingelogd(&state, &headers)?;
    let c0 = state
        .proces
        .portaal()
        .and_then(|p| p.aanbod.clone())
        .ok_or_else(|| {
            fout(
                StatusCode::INTERNAL_SERVER_ERROR,
                "geen aanbod geconfigureerd",
            )
        })?;
    let nu = (state.klok)();
    let datum = datum::peildatum(&nu);
    // Zonder tijdvak een run; met tijdvak een run per tijdvak dat het beleid
    // aanbiedt.
    let keuzes: Vec<Option<mogelijkheid::Keuze>> = match (&state.proces.tijdvak, &c0.tijdvakken) {
        (Some(t), Some(u)) => {
            mogelijkheid::tijdvakken(&state.proces.service, &c0.regeling, u, &datum)
                .map_err(intern)?
                .into_iter()
                .map(|w| {
                    Some(mogelijkheid::Keuze {
                        parameter: t.parameter.clone(),
                        veld: t.veld.clone(),
                        waarde: w,
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
            if let Some(veld) = &k.veld {
                external.insert(veld.clone(), k.waarde.clone());
            }
        }
        let concept = Concept {
            external,
            zaakkenmerk: None,
        };
        let begin = match (&keuze, &c0.begin) {
            (Some(k), Some(u)) => Some(
                mogelijkheid::begin(&state.proces.service, &c0.regeling, u, k, &datum)
                    .map_err(intern)?,
            ),
            _ => None,
        };
        let peil = peil_voor(&nu, begin);
        let mut c = concepttoets(&state, &sessie, &concept, &peil).await?;
        // Leidt de toets-lexostatus het tijdvak niet af, dan gaat de keuze
        // zelf mee.
        if let Some(k) = &keuze {
            if !c.samen.parameters.contains_key(&k.parameter) {
                c.samen
                    .parameters
                    .insert(k.parameter.clone(), k.waarde.clone());
                c.samen
                    .herkomst
                    .insert(k.parameter.clone(), synthese::Herkomst::Keuze);
            }
        }
        let m = mogelijkheid::bepaal(
            &state.proces.service,
            keuze,
            &c0,
            &c.samen.parameters,
            &datum,
        );
        uit.push(json!({
            "mogelijkheid": m,
            "peilmoment": peil.peilmoment.map(|t| t.to_string()),
            "parameters": c.samen.parameters,
            "herkomst": c.samen.herkomst,
            "bronnen": c.samen.bronnen,
        }));
    }
    Ok(Json(json!({
        "kvk": sessie.kvk,
        "persoon": sessie.persoon,
        // De datum van de runtime, zodat de frontend "verstreken" niet op de
        // klok van de browser beoordeelt.
        "datum": datum,
        "mogelijkheden": uit,
    })))
}

/// Het peil van de bronnen voor een aanbod: het begin van het gekozen
/// tijdvak als dat nog moet beginnen, anders vandaag. Het begin zegt het
/// beleid (`aanbod.begin`, zie [`mogelijkheid::begin`]); zonder begin, of
/// zonder tijdvak, peilt het aanbod op vandaag.
fn peil_voor(nu: &chrono::DateTime<chrono::FixedOffset>, begin: Option<chrono::NaiveDate>) -> Peil {
    let vandaag = nu.date_naive();
    Peil::op(Tijdpunt::Datum(
        begin.filter(|b| *b > vandaag).unwrap_or(vandaag),
    ))
}

/// Indienen: de cel bouwt het gram en legt het vast.
pub(super) async fn indienen(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<(StatusCode, Json<celclient::MetYaml>), Fout> {
    let sessie = ingelogd(&state, &headers)?;
    let verzoek = verzoek_voor(&state, &sessie, &concept).await?;
    let vastgelegd = celclient::leg_vast(state.cel.as_ref(), state.cel_id(), &verzoek)
        .await
        .map_err(van_cel)?;
    Ok((StatusCode::CREATED, Json(vastgelegd)))
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
            peil_voor(&nu, b).peilmoment.unwrap().to_string()
        };
        assert_eq!(op(Some("2027-01-01")), "2027-01-01");
        assert_eq!(op(Some("2026-01-01")), "2026-09-25");
        assert_eq!(op(None), "2026-09-25");
    }
}

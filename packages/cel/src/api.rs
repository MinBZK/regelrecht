//! De routes van een cel. De runtime biedt ze aan onder `/cellen/<id>`
//! (zie [`crate::runtime`]).
//!
//! Elke cel:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/kroniek` | de grammen, elk met YAML |
//! | `GET /api/lexostatus/{naam}?<input>=...` | een reductie, met de inputs als query |
//!
//! Een cel met een portaal (rol aanvrager, nep-eHerkenning) heeft daarnaast:
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/eherkenning/login` | `{kvk, persoon, machtiging}` naar een sessie |
//! | `GET /api/eherkenning/sessie` | wie is ingelogd |
//! | `POST /api/eherkenning/logout` | sessie beeindigen |
//! | `GET /api/stroom` | de stroomdefinitie en de velden van het formulier |
//! | `POST /api/aanvraag/toets` | concept naar gram in het geheugen, reductie, synthese, engine |
//! | `POST /api/aanvraag` | het gram vastleggen in de kroniek |
//!
//! Een cel met de rol behandelaar (nagebootste medewerkerslogin) heeft:
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/medewerker/login` | `{naam}` naar een sessie |
//! | `GET /api/medewerker/sessie` | wie is ingelogd |
//! | `POST /api/medewerker/logout` | sessie beeindigen |
//!
//! en met een `behandeling`, alleen voor de behandelaar:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/werkvoorraad` | de lijst-lexostatus van de werkvoorraad |
//! | `GET /api/zaken/{zaakkenmerk}` | de grammen van de zaak, het besluitformulier en een proefbesluit zonder oordelen |
//! | `POST /api/zaken/{zaakkenmerk}/proefbesluit` | `{formulier}` naar een proefbesluit; niets wordt vastgelegd |
//!
//! Heeft een cel rollen, dan zijn kroniek en lexostatus alleen voor wie is
//! ingelogd: de aanvrager ziet zijn eigen grammen, de behandelaar alle.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::besluit::{self, Weigering};
use crate::cel::Cel;
use crate::eherkenning::{Login, Sessie};
use crate::kroniek::Kroniek;
use crate::reductie;
use crate::sessie::{Gebruiker, Medewerker, Sessies, COOKIE};
use crate::stroom::{self, Binding, Gram, Indiening, Zaak};
use crate::synthese::{self, Bron};
use crate::toets;

/// Levert het moment waarop iets tot feit wordt gemaakt.
pub type Klok = Arc<dyn Fn() -> DateTime<FixedOffset> + Send + Sync>;

/// De klok van de cel: nu, in Nederlandse tijd.
pub fn systeemklok() -> Klok {
    Arc::new(|| {
        chrono::Utc::now()
            .with_timezone(&chrono_tz::Europe::Amsterdam)
            .fixed_offset()
    })
}

/// De toestand van een cel in de runtime.
#[derive(Clone)]
pub struct AppState {
    pub cel: Arc<Cel>,
    pub kroniek: Arc<Kroniek>,
    pub sessies: Arc<Sessies>,
    pub klok: Klok,
    /// De synthese-bronnen, met het transport dat de runtime koos.
    pub bronnen: Arc<Vec<Bron>>,
}

/// De routes van een cel, relatief aan `/cellen/<id>`.
pub fn router(state: AppState) -> Router {
    let mut r = Router::new()
        .route("/api/kroniek", get(kroniek_route))
        .route("/api/lexostatus/{naam}", get(lexostatus_route));
    let rollen = &state.cel.definitie.rollen;
    if rollen.aanvrager.is_some() {
        r = r
            .route("/api/eherkenning/login", post(login))
            .route("/api/eherkenning/sessie", get(sessie))
            .route("/api/eherkenning/logout", post(logout));
    }
    if state.cel.portaal().is_some() {
        r = r
            .route("/api/stroom", get(stroom_route))
            .route("/api/aanvraag/toets", post(toets_route))
            .route("/api/aanvraag", post(indienen));
    }
    if rollen.behandelaar.is_some() {
        r = r
            .route("/api/medewerker/login", post(medewerker_login))
            .route("/api/medewerker/sessie", get(medewerker_sessie))
            .route("/api/medewerker/logout", post(logout));
    }
    if state.cel.definitie.behandeling.is_some() {
        r = r
            .route("/api/werkvoorraad", get(werkvoorraad_route))
            .route("/api/zaken/{zaakkenmerk}", get(zaak_route))
            .route(
                "/api/zaken/{zaakkenmerk}/proefbesluit",
                post(proefbesluit_route),
            );
    }
    r.with_state(state)
}

/// Een fout als `{"fout": "..."}` met een status.
pub struct Fout(StatusCode, String);

impl IntoResponse for Fout {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"fout": self.1}))).into_response()
    }
}

fn fout(status: StatusCode, tekst: impl Into<String>) -> Fout {
    Fout(status, tekst.into())
}

fn gebruiker(state: &AppState, headers: &HeaderMap) -> Result<Gebruiker, Fout> {
    state
        .sessies
        .zoek(headers)
        .ok_or_else(|| fout(StatusCode::UNAUTHORIZED, "niet ingelogd"))
}

/// De ingelogde aanvrager; een behandelaar mag hier niet.
fn ingelogd(state: &AppState, headers: &HeaderMap) -> Result<Sessie, Fout> {
    gebruiker(state, headers)?
        .aanvrager()
        .cloned()
        .ok_or_else(|| fout(StatusCode::FORBIDDEN, "alleen voor de aanvrager"))
}

/// De ingelogde behandelaar; een aanvrager mag hier niet.
fn behandelaar(state: &AppState, headers: &HeaderMap) -> Result<Medewerker, Fout> {
    gebruiker(state, headers)?
        .behandelaar()
        .cloned()
        .ok_or_else(|| fout(StatusCode::FORBIDDEN, "alleen voor de behandelaar"))
}

/// Wiens grammen deze vrager mag zien. Zonder rollen is er geen login en ziet
/// iedereen alles (`None`); de behandelaar ziet ook alles; de aanvrager
/// alleen die van zijn KvK.
fn lezer(state: &AppState, headers: &HeaderMap) -> Result<Option<Sessie>, Fout> {
    if state.cel.definitie.rollen.is_leeg() {
        return Ok(None);
    }
    Ok(match gebruiker(state, headers)? {
        Gebruiker::Aanvrager(s) => Some(s),
        Gebruiker::Behandelaar(_) => None,
    })
}

/// De cookie geldt alleen onder het pad van deze cel.
fn cookie(state: &AppState, waarde: &str, extra: &str) -> String {
    format!(
        "{COOKIE}={waarde}; Path=/cellen/{}/; HttpOnly; SameSite=Strict{extra}",
        state.cel.id()
    )
}

async fn login(State(state): State<AppState>, Json(login): Json<Login>) -> Result<Response, Fout> {
    let sessie = login
        .valideer()
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let token = state.sessies.nieuw(Gebruiker::Aanvrager(sessie.clone()));
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(sessie)).into_response())
}

async fn sessie(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Sessie>, Fout> {
    ingelogd(&state, &headers).map(Json)
}

async fn medewerker_login(
    State(state): State<AppState>,
    Json(login): Json<Medewerker>,
) -> Result<Response, Fout> {
    let m = login
        .valideer()
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let token = state.sessies.nieuw(Gebruiker::Behandelaar(m.clone()));
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(m)).into_response())
}

async fn medewerker_sessie(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Medewerker>, Fout> {
    behandelaar(&state, &headers).map(Json)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    state.sessies.verwijder(&headers);
    let cookie = cookie(&state, "", "; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

async fn stroom_route(State(state): State<AppState>) -> Result<Json<Value>, Fout> {
    let (stroom, event) = portaal_event(&state)?;
    let velden = crate::formulier::velden(event, state.cel.formulier.as_ref());
    Ok(Json(json!({
        "stroom": stroom.document,
        "event": event.name,
        "titel": state.cel.formulier.as_ref().and_then(|f| f.titel.clone()),
        "velden": velden,
    })))
}

fn portaal_event(state: &AppState) -> Result<(&stroom::Stroom, &stroom::Event), Fout> {
    state.cel.portaal_event().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen portaal geconfigureerd",
        )
    })
}

#[derive(Deserialize)]
struct Concept {
    #[serde(default)]
    external: Map<String, Value>,
    /// Alleen bij een event met `zaak: volgt`: de zaak die het gram volgt.
    #[serde(default)]
    zaakkenmerk: Option<String>,
}

/// Het zaakkenmerk voor een gram van het portaal-event. `zaak: opent` geeft
/// een nieuw kenmerk; `volgt` neemt dat uit het concept, van een zaak die
/// de vrager in de kroniek heeft; `geen` geeft er geen.
fn zaakkenmerk_voor(
    state: &AppState,
    sessie: &Sessie,
    event: &stroom::Event,
    concept: &Concept,
) -> Result<Option<String>, Fout> {
    let meegegeven = concept.zaakkenmerk.clone();
    match event.zaak {
        Zaak::Opent if meegegeven.is_some() => Err(fout(
            StatusCode::BAD_REQUEST,
            format!(
                "event '{}' opent een zaak: de cel geeft het zaakkenmerk, het concept niet",
                event.name
            ),
        )),
        Zaak::Opent => Ok(Some(uuid::Uuid::new_v4().to_string())),
        Zaak::Volgt => {
            let Some(z) = meegegeven else {
                return Err(fout(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "event '{}' volgt een zaak: geef het zaakkenmerk mee",
                        event.name
                    ),
                ));
            };
            let mut bestaat = false;
            for chronicle in state.cel.kronieken() {
                let grammen = state
                    .kroniek
                    .lees(chronicle)
                    .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
                bestaat |= zichtbaar(state, Some(sessie), grammen)
                    .iter()
                    .any(|g| g.zaakkenmerk.as_deref() == Some(z.as_str()));
            }
            if bestaat {
                Ok(Some(z))
            } else {
                Err(fout(
                    StatusCode::BAD_REQUEST,
                    format!("geen zaak '{z}' in de kroniek"),
                ))
            }
        }
        Zaak::Geen => Ok(meegegeven),
    }
}

/// Bouw een gram uit een concept.
fn bouw(state: &AppState, sessie: &Sessie, concept: &Concept) -> Result<Gram, Fout> {
    let (stroom, event) = portaal_event(state)?;
    let zaakkenmerk = zaakkenmerk_voor(state, sessie, event, concept)?;
    let gram = stroom::bouw_gram(
        stroom,
        event,
        &Indiening {
            intake: &sessie.intake(&event.intake),
            external: &concept.external,
            op_moment: (state.klok)(),
            zaakkenmerk: zaakkenmerk.as_deref(),
        },
    )
    .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    gram.valideer().map_err(|f| {
        fout(
            StatusCode::BAD_REQUEST,
            format!("gram valideert niet: {}", f.join("; ")),
        )
    })?;
    Ok(gram)
}

/// Het gram als YAML, velden in de volgorde van de stroom.
pub fn als_yaml(cel: &Cel, gram: &Gram) -> String {
    let mut doc = match serde_yaml_ng::to_value(gram) {
        Ok(serde_yaml_ng::Value::Mapping(m)) => m,
        _ => return String::new(),
    };
    let event = cel
        .strommen
        .iter()
        .find(|s| s.id == gram.stroom.id)
        .and_then(|s| s.event(&gram.name));
    if let Some(event) = event {
        doc.insert(
            serde_yaml_ng::Value::String("fields".into()),
            serde_yaml_ng::Value::Mapping(event.geordend(&gram.fields)),
        );
    }
    serde_yaml_ng::to_string(&doc).unwrap_or_default()
}

async fn toets_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<Json<Value>, Fout> {
    let sessie = ingelogd(&state, &headers)?;
    // Een concept is geen feit: het gram blijft in het geheugen.
    let gram = bouw(&state, &sessie, &concept)?;
    let portaal = state.cel.portaal().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen portaal geconfigureerd",
        )
    })?;
    let def = state
        .cel
        .lexostatussen
        .lexostatus(&portaal.toets.lexostatus)
        .ok_or_else(|| {
            fout(
                StatusCode::INTERNAL_SERVER_ERROR,
                "toets-lexostatus ontbreekt",
            )
        })?;
    let lexostatus =
        reductie::leid_af(def, &gram).map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    // Synthese: de eigen lexostatus plus die van de bronnen. Niets hiervan
    // wordt vastgelegd.
    let samen = synthese::voeg_samen(&lexostatus, &state.bronnen).await;
    let datum = gram.op_moment.get(..10).unwrap_or_default().to_string();
    let mut uitslag = toets::toets(
        &state.cel.service,
        &portaal.toets.regeling,
        &portaal.toets.uitkomst,
        &samen.parameters,
        reductie::ontbreekt(def, &lexostatus.parameters),
        &datum,
    );
    if !uitslag.te_beoordelen {
        if let Some(reden) = samen.reden() {
            uitslag.reden = Some(reden);
        }
    }
    Ok(Json(json!({
        "uitslag": uitslag,
        "lexostatus": lexostatus,
        // Wat naar de engine ging, en per parameter waar het vandaan kwam.
        "parameters": samen.parameters,
        "herkomst": samen.herkomst,
        "bronnen": samen.bronnen,
    })))
}

async fn indienen(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<(StatusCode, Json<Value>), Fout> {
    let sessie = ingelogd(&state, &headers)?;
    let gram = bouw(&state, &sessie, &concept)?;
    state
        .kroniek
        .voeg_toe(&gram)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    tracing::info!(cel = %state.cel.id(), zaakkenmerk = gram.zaakkenmerk.as_deref().unwrap_or("-"), name = %gram.name, "gram vastgelegd");
    let yaml = als_yaml(&state.cel, &gram);
    Ok((
        StatusCode::CREATED,
        Json(json!({"gram": gram, "yaml": yaml})),
    ))
}

/// Of een gram van deze KvK is: het veld dat aan `$intake.eherkenning.kvk`
/// bindt, heeft dat nummer.
fn van_kvk(cel: &Cel, gram: &Gram, kvk: &str) -> bool {
    let Some(event) = cel
        .strommen
        .iter()
        .find(|s| s.id == gram.stroom.id)
        .and_then(|s| s.event(&gram.name))
    else {
        return false;
    };
    event.bladeren().iter().any(|b| {
        b.binding == Binding::Intake("eherkenning.kvk".into())
            && gram.veld(&b.pad).and_then(Value::as_str) == Some(kvk)
    })
}

/// De grammen die deze vrager mag zien.
fn zichtbaar(state: &AppState, sessie: Option<&Sessie>, grammen: Vec<Gram>) -> Vec<Gram> {
    match sessie {
        Some(s) => grammen
            .into_iter()
            .filter(|g| van_kvk(&state.cel, g, &s.kvk))
            .collect(),
        None => grammen,
    }
}

async fn kroniek_route(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Fout> {
    let sessie = lezer(&state, &headers)?;
    let mut uit = Vec::new();
    for chronicle in state.cel.kronieken() {
        let grammen = state
            .kroniek
            .lees(chronicle)
            .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        for gram in zichtbaar(&state, sessie.as_ref(), grammen) {
            let yaml = als_yaml(&state.cel, &gram);
            uit.push(json!({"gram": gram, "yaml": yaml}));
        }
    }
    Ok(Json(Value::Array(uit)))
}

async fn lexostatus_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(naam): Path<String>,
    Query(inputs): Query<Map<String, Value>>,
) -> Result<Json<reductie::Lexostatus>, Fout> {
    let sessie = lezer(&state, &headers)?;
    let def = state
        .cel
        .lexostatussen
        .lexostatus(&naam)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, format!("geen lexostatus '{naam}'")))?;
    for i in &def.inputs {
        if !inputs.get(&i.name).is_some_and(reductie::gevuld) {
            return Err(fout(
                StatusCode::BAD_REQUEST,
                format!("input '{}' ontbreekt", i.name),
            ));
        }
    }
    let grammen = state
        .kroniek
        .lees(&def.reduction.kroniek)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let grammen = zichtbaar(&state, sessie.as_ref(), grammen);
    reductie::reduceer(def, &inputs, &grammen)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?
        .map(Json)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, "geen gram voor deze vraag"))
}

/// De kroniek van de cel, alle grammen, over al haar kronieken.
fn alle_grammen(state: &AppState) -> Result<Vec<Gram>, Fout> {
    let mut uit = Vec::new();
    for chronicle in state.cel.kronieken() {
        uit.extend(
            state
                .kroniek
                .lees(chronicle)
                .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?,
        );
    }
    Ok(uit)
}

fn behandeling(state: &AppState) -> Result<&crate::config::Behandeling, Fout> {
    state.cel.definitie.behandeling.as_ref().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen behandeling geconfigureerd",
        )
    })
}

async fn werkvoorraad_route(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<reductie::Lexostatus>, Fout> {
    behandelaar(&state, &headers)?;
    let naam = &behandeling(&state)?.werkvoorraad;
    let def = state.cel.lexostatussen.lexostatus(naam).ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("geen lexostatus '{naam}'"),
        )
    })?;
    let grammen = state
        .kroniek
        .lees(&def.reduction.kroniek)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    reductie::reduceer(def, &Map::new(), &grammen)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map(Json)
        .ok_or_else(|| fout(StatusCode::INTERNAL_SERVER_ERROR, "geen werkvoorraad"))
}

/// De datum van vandaag volgens de klok van de cel: de peildatum van een
/// proefbesluit.
fn vandaag(state: &AppState) -> String {
    (state.klok)().format("%Y-%m-%d").to_string()
}

#[derive(Deserialize, Default)]
struct Besluitformulier {
    #[serde(default)]
    formulier: Map<String, Value>,
}

async fn proefbesluit_voor(
    state: &AppState,
    zaakkenmerk: &str,
    formulier: &Map<String, Value>,
) -> Result<besluit::Proefbesluit, Fout> {
    besluit::proefbesluit(
        &state.cel,
        &state.kroniek,
        &state.bronnen,
        zaakkenmerk,
        formulier,
        &vandaag(state),
    )
    .await
    .map_err(|w| match w {
        Weigering::OnbekendOordeel(t) => fout(StatusCode::BAD_REQUEST, t),
        Weigering::Cel(t) => fout(StatusCode::INTERNAL_SERVER_ERROR, t),
    })
}

/// De grammen van een zaak; een 404 als de kroniek de zaak niet kent.
fn zaakgrammen(state: &AppState, zaakkenmerk: &str) -> Result<Vec<Gram>, Fout> {
    let grammen: Vec<Gram> = alle_grammen(state)?
        .into_iter()
        .filter(|g| g.zaakkenmerk.as_deref() == Some(zaakkenmerk))
        .collect();
    if grammen.is_empty() {
        return Err(fout(
            StatusCode::NOT_FOUND,
            format!("geen zaak '{zaakkenmerk}' in de kroniek"),
        ));
    }
    Ok(grammen)
}

async fn zaak_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zaakkenmerk): Path<String>,
) -> Result<Json<Value>, Fout> {
    behandelaar(&state, &headers)?;
    let grammen = zaakgrammen(&state, &zaakkenmerk)?;
    let b = &behandeling(&state)?.besluit;
    let proef = proefbesluit_voor(&state, &zaakkenmerk, &Map::new()).await?;
    let grammen: Vec<Value> = grammen
        .iter()
        .map(|g| json!({"gram": g, "yaml": als_yaml(&state.cel, g)}))
        .collect();
    Ok(Json(json!({
        "zaakkenmerk": zaakkenmerk,
        "grammen": grammen,
        "besluit": {
            "regeling": b.regeling,
            "artikel": proef.artikel,
            "uitkomsten": b.uitkomsten,
            "formulier": besluit::formuliervelden(&state.cel, b),
            "stand_bij_besluit": b.stand_bij_besluit,
        },
        "proefbesluit": proef,
    })))
}

async fn proefbesluit_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zaakkenmerk): Path<String>,
    Json(invoer): Json<Besluitformulier>,
) -> Result<Json<besluit::Proefbesluit>, Fout> {
    behandelaar(&state, &headers)?;
    zaakgrammen(&state, &zaakkenmerk)?;
    proefbesluit_voor(&state, &zaakkenmerk, &invoer.formulier)
        .await
        .map(Json)
}

/// Wat `GET /api/cellen` over een cel zegt: wie ze is, of ze een portaal
/// heeft, welke lexostatussen ze aanbiedt en uit welke bronnen haar toets
/// samenvoegt.
pub fn beschrijving(state: &AppState) -> Value {
    let cel = &state.cel;
    let lexostatussen: Vec<Value> = cel
        .lexostatussen
        .lexostatus_definitions
        .iter()
        .map(|d| {
            json!({
                "name": d.name,
                "inputs": d.inputs,
                "lijst": d.is_lijst(),
                "parameters": if d.is_lijst() { Vec::new() } else { d.reduction.afleidingen.keys().collect::<Vec<_>>() },
                "kolommen": if d.is_lijst() { d.reduction.afleidingen.keys().collect::<Vec<_>>() } else { Vec::new() },
                "extra_velden": d.reduction.extra_velden.keys().collect::<Vec<_>>(),
            })
        })
        .collect();
    let synthese: Vec<Value> = state
        .bronnen
        .iter()
        .map(|b| {
            json!({
                "cel": b.definitie.cel,
                "lexostatus": b.definitie.lexostatus,
                "transport": b.transport.soort(),
                "parameters": b.definitie.parameters,
            })
        })
        .collect();
    json!({
        "id": cel.id(),
        "recording_actor": cel.definitie.recording_actor,
        "portaal": cel.portaal().is_some(),
        "rollen": {
            "aanvrager": cel.definitie.rollen.aanvrager.is_some(),
            "behandelaar": cel.definitie.rollen.behandelaar.is_some(),
        },
        "behandeling": cel.definitie.behandeling.as_ref().map(|b| json!({
            "werkvoorraad": b.werkvoorraad,
            "regeling": b.besluit.regeling,
            "uitkomsten": b.besluit.uitkomsten,
        })),
        "titel": cel.formulier.as_ref().and_then(|f| f.titel.clone()),
        "kronieken": cel.kronieken(),
        "lexostatussen": lexostatussen,
        "synthese": synthese,
    })
}

//! De routes van een cel en van een proces. De runtime biedt ze aan onder
//! `/cellen/<id>` en `/processen/<id>` (zie [`crate::runtime`]).
//!
//! Een cel legt vast, bewaart en reduceert. Elke cel:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/kroniek` | de grammen, elk met YAML |
//! | `GET /api/lexostatus/{naam}?<input>=...` | een reductie, met de inputs als query |
//! | `POST /api/lexostatus/{naam}/proef` | `{concept, inputs}`: bouwt het gram in het geheugen en reduceert de kroniek mét dat gram; legt niets vast |
//! | `POST /api/grammen` | `{actor, stroom, event, intake, external, zaakkenmerk?, besluit?}`: bouwt het gram, valideert het, controleert de actor en legt het vast |
//! | `GET /api/stroom` | de stroomdefinities van de cel, met hun hash |
//!
//! Een proces handelt: het informeert, concludeert en laat een cel
//! vastleggen. Elk proces:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/voorbeelden` | standaardgegevens per handeling (`voorbeelden` in `proces.yaml`), ook zonder login |
//!
//! Een proces met een portaal (rol aanvrager, nep-eHerkenning) heeft daarnaast:
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/eherkenning/login` | `{kvk, persoon}` naar een sessie |
//! | `GET /api/eherkenning/sessie` | wie is ingelogd |
//! | `POST /api/eherkenning/logout` | sessie beeindigen |
//! | `GET /api/formulier` | de velden van het aanvraagformulier, uit de stroom van de cel |
//! | `POST /api/aanvraag/toets` | proefreductie in de cel, synthese, engine |
//! | `POST /api/aanvraag` | de cel legt het gram vast |
//! | `GET /api/mogelijkheden` | wat het portaal aanbiedt volgens het beleid, per tijdvak, met trace |
//!
//! Een proces met de rol behandelaar (nagebootste medewerkerslogin) heeft:
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
//! | `GET /api/werkvoorraad` | de lijst-lexostatus van de werkvoorraad, uit de cel |
//! | `GET /api/zaken/{zaakkenmerk}` | de grammen van de zaak, het besluitformulier en een proefbesluit zonder oordelen |
//! | `POST /api/zaken/{zaakkenmerk}/proefbesluit` | `{formulier}` naar een proefbesluit; niets wordt vastgelegd |
//! | `POST /api/zaken/{zaakkenmerk}/besluit` | het besluit nemen; de cel legt het vast |
//!
//! Tussen proces en cel is geen beveiligingscontext: de routes van een cel
//! vragen geen login, net als een bron van een andere organisatie.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::besluit::{self, Weigering};
use crate::cel::Cel;
use crate::eherkenning::{Login, Sessie};
use crate::kroniek::Kroniek;
use crate::mogelijkheid;
use crate::proces::Proces;
use crate::reductie::{self, Lexostatus};
use crate::rijen::Rijen;
use crate::sessie::{Gebruiker, Medewerker, Sessies, COOKIE};
use crate::stroom::{self, Binding, GeladenRegeling, Gram, Indiening, Invoer, Receipt, Zaak};
use crate::synthese::{self, Bron};
use crate::toets;
use crate::transport::{Transport, TransportFout};

/// Levert het moment waarop iets tot feit wordt gemaakt.
pub type Klok = Arc<dyn Fn() -> DateTime<FixedOffset> + Send + Sync>;

/// De klok van de runtime: nu, in Nederlandse tijd.
pub fn systeemklok() -> Klok {
    Arc::new(|| {
        chrono::Utc::now()
            .with_timezone(&chrono_tz::Europe::Amsterdam)
            .fixed_offset()
    })
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

// ---------------------------------------------------------------------------
// De cel
// ---------------------------------------------------------------------------

/// De toestand van een cel in de runtime.
#[derive(Clone)]
pub struct CelState {
    pub cel: Arc<Cel>,
    pub kroniek: Arc<Kroniek>,
    pub klok: Klok,
}

/// De routes van een cel, relatief aan `/cellen/<id>`.
pub fn cel_router(state: CelState) -> Router {
    Router::new()
        .route("/api/kroniek", get(kroniek_route))
        .route("/api/lexostatus/{naam}", get(lexostatus_route))
        .route("/api/lexostatus/{naam}/proef", post(proef_route))
        .route("/api/grammen", post(grammen_route))
        .route("/api/stroom", get(stroom_route))
        .with_state(state)
}

/// Wat een proces de cel vraagt vast te leggen (`POST /api/grammen`), of op
/// proef te reduceren. De cel bouwt het gram uit haar stroom: het proces
/// geeft alleen de invoer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vastlegverzoek {
    /// Wie laat vastleggen. De cel weigert als dat niet de `recording_actor`
    /// van de stroom is.
    pub actor: String,
    pub stroom: String,
    pub event: String,
    /// Het ontvangstkanaal (`$intake.*`): wie indiende en langs welke weg.
    #[serde(default)]
    pub intake: Value,
    /// De inhoud (`$external.*`).
    #[serde(default)]
    pub external: Map<String, Value>,
    /// Bij `zaak: volgt` de zaak die het gram volgt. Bij `zaak: opent` geeft
    /// de cel het kenmerk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zaakkenmerk: Option<String>,
    /// Alleen bij een besluit dat het proces nam: wat het besluit tot besluit
    /// maakt. Het proces draait de engine, dus het proces stelt dit samen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub besluit: Option<Besluitvelden>,
}

/// De velden van een besluit op een gram, naast de stroomvorm (zie
/// [`crate::besluit::neem_besluit`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Besluitvelden {
    #[serde(default)]
    pub legal_character: Option<String>,
    #[serde(default)]
    pub decision_type: Option<String>,
    #[serde(default)]
    pub regulation: Option<String>,
    #[serde(default)]
    pub regulation_valid_from: Option<String>,
    #[serde(default)]
    pub competent_authority: Option<String>,
    #[serde(default)]
    pub inputs: BTreeMap<String, Invoer>,
    #[serde(default)]
    pub receipt: Option<Receipt>,
}

/// Het zaakkenmerk van een gram. `zaak: opent` geeft een nieuw kenmerk;
/// `volgt` neemt dat uit het verzoek, van een zaak die de kroniek kent;
/// `geen` geeft er geen.
fn zaakkenmerk_voor(
    state: &CelState,
    event: &stroom::Event,
    meegegeven: Option<String>,
) -> Result<Option<String>, Fout> {
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
            if alle_grammen(state)?
                .iter()
                .any(|g| g.zaakkenmerk.as_deref() == Some(z.as_str()))
            {
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

/// Bouw een gram uit een verzoek, zonder het vast te leggen. De actor moet de
/// `recording_actor` van de stroom zijn.
fn bouw(state: &CelState, v: &Vastlegverzoek) -> Result<Gram, Fout> {
    let (stroom, event) = state.cel.event(&v.stroom, &v.event).ok_or_else(|| {
        fout(
            StatusCode::BAD_REQUEST,
            format!(
                "cel '{}' heeft geen event '{}' in stroom '{}'",
                state.cel.id(),
                v.event,
                v.stroom
            ),
        )
    })?;
    if v.actor != stroom.recording_actor {
        return Err(fout(
            StatusCode::FORBIDDEN,
            format!(
                "actor '{}' legt niet vast in stroom '{}': de recording_actor is '{}'",
                v.actor, stroom.id, stroom.recording_actor
            ),
        ));
    }
    let zaakkenmerk = zaakkenmerk_voor(state, event, v.zaakkenmerk.clone())?;
    let mut gram = stroom::bouw_gram(
        stroom,
        event,
        &Indiening {
            intake: &v.intake,
            external: &v.external,
            op_moment: (state.klok)(),
            zaakkenmerk: zaakkenmerk.as_deref(),
        },
    )
    .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    if let Some(b) = &v.besluit {
        gram.legal_character = b.legal_character.clone();
        gram.decision_type = b.decision_type.clone();
        gram.regulation = b.regulation.clone();
        gram.regulation_valid_from = b.regulation_valid_from.clone();
        gram.competent_authority = b.competent_authority.clone();
        gram.inputs = b.inputs.clone();
        gram.receipt = b.receipt.clone();
    }
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
    if let Some((_, event)) = cel.event(&gram.stroom.id, &gram.name) {
        doc.insert(
            serde_yaml_ng::Value::String("fields".into()),
            serde_yaml_ng::Value::Mapping(event.geordend(&gram.fields)),
        );
    }
    serde_yaml_ng::to_string(&doc).unwrap_or_default()
}

/// Leg een gram vast. Antwoord: het gram, met YAML.
async fn grammen_route(
    State(state): State<CelState>,
    Json(verzoek): Json<Vastlegverzoek>,
) -> Result<(StatusCode, Json<Value>), Fout> {
    let gram = bouw(&state, &verzoek)?;
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

#[derive(Deserialize)]
struct Proefverzoek {
    concept: Vastlegverzoek,
    #[serde(default)]
    inputs: Map<String, Value>,
}

/// Een proefreductie: het gram van het concept in het geheugen, de kroniek
/// mét dat gram gereduceerd. Zonder input `zaakkenmerk` telt dat van het
/// concept. Een concept is geen feit: niets wordt vastgelegd.
async fn proef_route(
    State(state): State<CelState>,
    Path(naam): Path<String>,
    Json(verzoek): Json<Proefverzoek>,
) -> Result<Json<Value>, Fout> {
    let def = lexostatus_def(&state, &naam)?;
    let gram = bouw(&state, &verzoek.concept)?;
    let mut inputs = verzoek.inputs;
    if let Some(z) = &gram.zaakkenmerk {
        if def.inputs.iter().any(|i| i.name == "zaakkenmerk") && !inputs.contains_key("zaakkenmerk")
        {
            inputs.insert("zaakkenmerk".into(), Value::String(z.clone()));
        }
    }
    inputs_compleet(def, &inputs)?;
    let mut grammen = state
        .kroniek
        .lees(&def.reduction.kroniek)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    // Het concept als laatste: bij gelijk moment kiest `kies: laatste` het.
    grammen.push(gram.clone());
    let lexostatus = reductie::reduceer(def, &inputs, &grammen)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, "geen gram voor deze vraag"))?;
    Ok(Json(json!({"gram": gram, "lexostatus": lexostatus})))
}

fn lexostatus_def<'s>(
    state: &'s CelState,
    naam: &str,
) -> Result<&'s reductie::LexostatusDefinitie, Fout> {
    state
        .cel
        .lexostatussen
        .lexostatus(naam)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, format!("geen lexostatus '{naam}'")))
}

fn inputs_compleet(
    def: &reductie::LexostatusDefinitie,
    inputs: &Map<String, Value>,
) -> Result<(), Fout> {
    for i in &def.inputs {
        if !inputs.get(&i.name).is_some_and(reductie::gevuld) {
            return Err(fout(
                StatusCode::BAD_REQUEST,
                format!("input '{}' ontbreekt", i.name),
            ));
        }
    }
    Ok(())
}

/// De kroniek van de cel, alle grammen, over al haar kronieken.
fn alle_grammen(state: &CelState) -> Result<Vec<Gram>, Fout> {
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

async fn kroniek_route(State(state): State<CelState>) -> Result<Json<Value>, Fout> {
    let uit: Vec<Value> = alle_grammen(&state)?
        .iter()
        .map(|g| json!({"gram": g, "yaml": als_yaml(&state.cel, g)}))
        .collect();
    Ok(Json(Value::Array(uit)))
}

async fn lexostatus_route(
    State(state): State<CelState>,
    Path(naam): Path<String>,
    Query(inputs): Query<Map<String, Value>>,
) -> Result<Json<Lexostatus>, Fout> {
    let def = lexostatus_def(&state, &naam)?;
    inputs_compleet(def, &inputs)?;
    let grammen = state
        .kroniek
        .lees(&def.reduction.kroniek)
        .map_err(|e| fout(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    reductie::reduceer(def, &inputs, &grammen)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?
        .map(Json)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, "geen gram voor deze vraag"))
}

/// De stroomdefinities van de cel, elk met de hash die in haar grammen en in
/// het receipt van een besluit staat.
async fn stroom_route(State(state): State<CelState>) -> Json<Value> {
    let strommen: Vec<Value> = state
        .cel
        .strommen
        .iter()
        .map(|s| json!({"id": s.id, "sha256": s.sha256, "stroom": s.document}))
        .collect();
    Json(json!({"cel": state.cel.id(), "strommen": strommen}))
}

/// Wat `GET /api/cellen` over een cel zegt: wie ze is, welke kronieken ze
/// bijhoudt en welke lexostatussen ze aanbiedt.
pub fn cel_beschrijving(state: &CelState) -> Value {
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
    json!({
        "id": cel.id(),
        "recording_actor": cel.definitie.recording_actor,
        "kronieken": cel.kronieken(),
        "lexostatussen": lexostatussen,
    })
}

// ---------------------------------------------------------------------------
// Vragen van een proces aan een cel
// ---------------------------------------------------------------------------

/// Een route van een cel.
pub fn celpad(cel: &str, route: &str) -> String {
    format!("/cellen/{cel}/api/{route}")
}

/// Een fout van de cel als antwoord van het proces: dezelfde status en
/// dezelfde tekst. Een cel die niet antwoordt is een fout van de runtime.
fn van_cel(f: TransportFout) -> Fout {
    match f {
        TransportFout::Antwoord {
            status,
            fout: tekst,
        } => Fout(
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            tekst,
        ),
        TransportFout::Onbereikbaar(r) => fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("de cel is onbereikbaar: {r}"),
        ),
    }
}

/// De grammen in de kroniek van een cel (`GET kroniek`), elk met YAML.
pub async fn lees_kroniek(cel: &dyn Transport, id: &str) -> Result<Vec<Value>, TransportFout> {
    let v = cel.haal(&celpad(id, "kroniek")).await?;
    Ok(v.as_array().cloned().unwrap_or_default())
}

/// Het gram van een regel uit [`lees_kroniek`].
pub fn gram_van(item: &Value) -> Option<Gram> {
    serde_json::from_value(item.get("gram")?.clone()).ok()
}

// ---------------------------------------------------------------------------
// Het proces
// ---------------------------------------------------------------------------

/// De toestand van een proces in de runtime.
#[derive(Clone)]
pub struct ProcesState {
    pub proces: Arc<Proces>,
    /// Het transport naar de cel waarin het proces vastlegt (intern).
    pub cel: Arc<dyn Transport>,
    pub sessies: Arc<Sessies>,
    pub klok: Klok,
    /// De synthese-bronnen die geen lexostatus van de zaak zijn, met het
    /// transport dat de runtime koos.
    pub bronnen: Arc<Vec<Bron>>,
    /// De synthese per regel van het besluit, met haar bronnen.
    pub rijen: Arc<Vec<Rijen>>,
    /// De geladen regelingen, voor het receipt van een besluit.
    pub regelingen: Arc<Vec<GeladenRegeling>>,
}

impl ProcesState {
    fn cel_id(&self) -> &str {
        self.proces.cel.id()
    }
}

/// De routes van een proces, relatief aan `/processen/<id>`.
pub fn proces_router(state: ProcesState) -> Router {
    let mut r = Router::new().route("/api/voorbeelden", get(voorbeelden_route));
    let rollen = &state.proces.definitie.rollen;
    if rollen.aanvrager.is_some() {
        r = r
            .route("/api/eherkenning/login", post(login))
            .route("/api/eherkenning/sessie", get(sessie))
            .route("/api/eherkenning/logout", post(logout));
    }
    if state.proces.portaal().is_some() {
        r = r
            .route("/api/formulier", get(formulier_route))
            .route("/api/aanvraag/toets", post(toets_route))
            .route("/api/aanvraag", post(indienen))
            .route("/api/mogelijkheden", get(mogelijkheden_route));
    }
    if rollen.behandelaar.is_some() {
        r = r
            .route("/api/medewerker/login", post(medewerker_login))
            .route("/api/medewerker/sessie", get(medewerker_sessie))
            .route("/api/medewerker/logout", post(logout));
    }
    if state.proces.definitie.behandeling.is_some() {
        r = r
            .route("/api/werkvoorraad", get(werkvoorraad_route))
            .route("/api/zaken/{zaakkenmerk}", get(zaak_route))
            .route(
                "/api/zaken/{zaakkenmerk}/proefbesluit",
                post(proefbesluit_route),
            )
            .route("/api/zaken/{zaakkenmerk}/besluit", post(besluit_route));
    }
    r.with_state(state)
}

fn gebruiker(state: &ProcesState, headers: &HeaderMap) -> Result<Gebruiker, Fout> {
    state
        .sessies
        .zoek(headers)
        .ok_or_else(|| fout(StatusCode::UNAUTHORIZED, "niet ingelogd"))
}

/// De ingelogde aanvrager; een behandelaar mag hier niet.
fn ingelogd(state: &ProcesState, headers: &HeaderMap) -> Result<Sessie, Fout> {
    gebruiker(state, headers)?
        .aanvrager()
        .cloned()
        .ok_or_else(|| fout(StatusCode::FORBIDDEN, "alleen voor de aanvrager"))
}

/// De ingelogde behandelaar; een aanvrager mag hier niet.
fn behandelaar(state: &ProcesState, headers: &HeaderMap) -> Result<Medewerker, Fout> {
    gebruiker(state, headers)?
        .behandelaar()
        .cloned()
        .ok_or_else(|| fout(StatusCode::FORBIDDEN, "alleen voor de behandelaar"))
}

/// De cookie geldt alleen onder het pad van dit proces.
fn cookie(state: &ProcesState, waarde: &str, extra: &str) -> String {
    format!(
        "{COOKIE}={waarde}; Path=/processen/{}/; HttpOnly; SameSite=Strict{extra}",
        state.proces.id()
    )
}

async fn login(
    State(state): State<ProcesState>,
    Json(login): Json<Login>,
) -> Result<Response, Fout> {
    let sessie = login
        .valideer()
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let token = state.sessies.nieuw(Gebruiker::Aanvrager(sessie.clone()));
    let cookie = cookie(&state, &token, "");
    Ok(([(header::SET_COOKIE, cookie)], Json(sessie)).into_response())
}

async fn sessie(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Sessie>, Fout> {
    ingelogd(&state, &headers).map(Json)
}

async fn medewerker_login(
    State(state): State<ProcesState>,
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
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Medewerker>, Fout> {
    behandelaar(&state, &headers).map(Json)
}

async fn logout(State(state): State<ProcesState>, headers: HeaderMap) -> Response {
    state.sessies.verwijder(&headers);
    let cookie = cookie(&state, "", "; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

/// De voorbeelden van het proces. Ook zonder login: de inlogvoorbeelden zijn
/// er juist voor het inloggen.
async fn voorbeelden_route(
    State(state): State<ProcesState>,
) -> Json<crate::voorbeelden::Voorbeelden> {
    Json(state.proces.voorbeelden.clone())
}

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
async fn formulier_route(State(state): State<ProcesState>) -> Result<Json<Value>, Fout> {
    let (stroom, event) = portaal_event(&state)?;
    let formulier = state.proces.formulier.as_ref();
    let velden = crate::formulier::velden(event, formulier);
    Ok(Json(json!({
        "cel": state.cel_id(),
        "stroom": stroom.document,
        "event": event.name,
        "titel": formulier.and_then(|f| f.titel.clone()),
        "velden": velden,
    })))
}

#[derive(Deserialize)]
struct Concept {
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
        let kroniek = lees_kroniek(state.cel.as_ref(), state.cel_id())
            .await
            .map_err(van_cel)?;
        let bekend = kroniek.iter().filter_map(gram_van).any(|g| {
            g.zaakkenmerk.as_deref() == Some(z.as_str())
                && van_kvk(&state.proces.cel, &g, &sessie.kvk)
        });
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
}

async fn concepttoets<'a>(
    state: &'a ProcesState,
    sessie: &Sessie,
    concept: &Concept,
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
    let antwoord = state
        .cel
        .stuur(
            &celpad(
                state.cel_id(),
                &format!("lexostatus/{}/proef", portaal.toets.lexostatus),
            ),
            &json!({"concept": verzoek, "inputs": {}}),
        )
        .await
        .map_err(van_cel)?;
    let onleesbaar = |e: serde_json::Error| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("de proefreductie van de cel is onleesbaar: {e}"),
        )
    };
    let gram: Gram = serde_json::from_value(antwoord.get("gram").cloned().unwrap_or_default())
        .map_err(onleesbaar)?;
    let lexostatus: Lexostatus =
        serde_json::from_value(antwoord.get("lexostatus").cloned().unwrap_or_default())
            .map_err(onleesbaar)?;
    // Synthese: de lexostatus van het concept plus die van de bronnen.
    let samen = synthese::voeg_samen(&lexostatus, &state.bronnen).await;
    Ok(Concepttoets {
        portaal,
        def,
        gram,
        lexostatus,
        samen,
    })
}

async fn toets_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<Json<Value>, Fout> {
    let sessie = ingelogd(&state, &headers)?;
    let c = concepttoets(&state, &sessie, &concept).await?;
    let datum = c.gram.op_moment.get(..10).unwrap_or_default().to_string();
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
    })))
}

/// Wat het beleid de ingelogde persoon aanbiedt (`portaal.aanbod`), per
/// tijdvak uit `aanbod.keuzes`. Het tijdvak is de parameter van het
/// aanbod-artikel met origin BELANGHEBBENDE en grondslag Awb 4:2 lid 1; het
/// beleid wordt uitgevoerd op een concept met alleen dat tijdvak, plus wat de
/// eHerkenning en de synthese weten. Uitkomst en termijn komen uit een run,
/// met trace. Een feit dat een bron niet leverde, maakt het aanbod niet te
/// bepalen. Niets wordt vastgelegd.
async fn mogelijkheden_route(
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
    let datum = nu.format("%Y-%m-%d").to_string();
    let jaar = i64::from(chrono::Datelike::year(&nu));
    // Zonder tijdvak een run; met tijdvak een run per keuze.
    let keuzes: Vec<Option<mogelijkheid::Keuze>> = match (&state.proces.tijdvak, &c0.keuzes) {
        (Some(t), Some(k)) => k
            .waarden(jaar)
            .into_iter()
            .map(|w| {
                Some(mogelijkheid::Keuze {
                    parameter: t.parameter.clone(),
                    veld: t.veld.clone(),
                    waarde: json!(w),
                })
            })
            .collect(),
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
        let mut c = concepttoets(&state, &sessie, &concept).await?;
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

/// Indienen: de cel bouwt het gram en legt het vast.
async fn indienen(
    State(state): State<ProcesState>,
    headers: HeaderMap,
    Json(concept): Json<Concept>,
) -> Result<(StatusCode, Json<Value>), Fout> {
    let sessie = ingelogd(&state, &headers)?;
    let verzoek = verzoek_voor(&state, &sessie, &concept).await?;
    let antwoord = state
        .cel
        .stuur(
            &celpad(state.cel_id(), "grammen"),
            &serde_json::to_value(&verzoek).unwrap_or_default(),
        )
        .await
        .map_err(van_cel)?;
    Ok((StatusCode::CREATED, Json(antwoord)))
}

fn behandeling(state: &ProcesState) -> Result<&crate::config::Behandeling, Fout> {
    state.proces.definitie.behandeling.as_ref().ok_or_else(|| {
        fout(
            StatusCode::INTERNAL_SERVER_ERROR,
            "geen behandeling geconfigureerd",
        )
    })
}

/// De werkvoorraad: de lijst-lexostatus uit de cel.
async fn werkvoorraad_route(
    State(state): State<ProcesState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Fout> {
    behandelaar(&state, &headers)?;
    let w = &behandeling(&state)?.werkvoorraad;
    let v = state
        .cel
        .haal(&synthese::pad(&w.cel, &w.lexostatus, &Map::new()))
        .await
        .map_err(|f| fout(StatusCode::INTERNAL_SERVER_ERROR, f.to_string()))?;
    Ok(Json(v))
}

/// De datum van vandaag volgens de klok van de runtime: de peildatum van een
/// proefbesluit.
fn vandaag(state: &ProcesState) -> String {
    (state.klok)().format("%Y-%m-%d").to_string()
}

#[derive(Deserialize, Default)]
struct Besluitformulier {
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
async fn besluit_route(
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

/// De grammen van een zaak, elk met YAML, uit de kroniek van de cel; een 404
/// als de kroniek de zaak niet kent.
async fn zaakgrammen(state: &ProcesState, zaakkenmerk: &str) -> Result<Vec<Value>, Fout> {
    let grammen: Vec<Value> = lees_kroniek(state.cel.as_ref(), state.cel_id())
        .await
        .map_err(van_cel)?
        .into_iter()
        .filter(|i| i["gram"]["zaakkenmerk"].as_str() == Some(zaakkenmerk))
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
            "formulier": besluit::formuliervelden(&state.proces.service, b),
            "stand_bij_besluit": b.stand_bij_besluit,
        },
        "proefbesluit": proef,
    })))
}

async fn proefbesluit_route(
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

/// Wat `GET /api/processen` over een proces zegt: wie handelt, in welke cel,
/// met welke rollen, of er een portaal en een behandeling is, en uit welke
/// bronnen de synthese samenvoegt.
pub fn proces_beschrijving(state: &ProcesState) -> Value {
    let p = &state.proces;
    let d = &p.definitie;
    let synthese: Vec<Value> = d
        .zaakbronnen()
        .map(|b| {
            json!({
                "cel": b.cel,
                "lexostatus": b.lexostatus,
                "zaak": true,
                "transport": state.cel.soort(),
                "parameters": Vec::<String>::new(),
            })
        })
        .chain(state.bronnen.iter().map(|b| {
            json!({
                "cel": b.definitie.cel,
                "lexostatus": b.definitie.lexostatus,
                "zaak": false,
                "transport": b.transport.soort(),
                "parameters": b.definitie.parameters,
            })
        }))
        .collect();
    json!({
        "id": p.id(),
        "actor": d.actor,
        "cel": p.cel.id(),
        "portaal": p.portaal().is_some(),
        "rollen": {
            "aanvrager": d.rollen.aanvrager.is_some(),
            "behandelaar": d.rollen.behandelaar.is_some(),
        },
        "behandeling": d.behandeling.as_ref().map(|b| json!({
            "werkvoorraad": b.werkvoorraad.lexostatus,
            "regeling": b.besluit.regeling,
            "uitkomsten": b.besluit.uitkomsten,
        })),
        "titel": p.formulier.as_ref().and_then(|f| f.titel.clone()),
        "synthese": synthese,
    })
}

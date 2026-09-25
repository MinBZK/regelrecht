//! De routes van een cel: vastleggen, bewaren en reduceren. Relatief aan
//! `/cellen/<id>`; zie de tabel in [`crate::api`].

use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{fout, intern, Fout, Klok};
use crate::cel::Cel;
use crate::celclient::Vastlegverzoek;
use crate::kroniek::{Kroniek, Vastgelegd};
use crate::reductie::{self, Lexostatus};
use crate::stroom::{self, Gram, Indiening, Zaak};
use crate::transport::{RuntimeToken, RUNTIME_TOKEN_HEADER};

/// De toestand van een cel in de runtime.
#[derive(Clone)]
pub struct CelState {
    pub cel: Arc<Cel>,
    pub kroniek: Arc<Kroniek>,
    pub klok: Klok,
    /// Wie dit token meestuurt, is een proces van deze runtime; alleen die
    /// mag vastleggen of op proef reduceren.
    pub runtime_token: RuntimeToken,
}

/// De routes van een cel, relatief aan `/cellen/<id>`. Lezen is open;
/// vastleggen en op proef reduceren vragen het runtime-token (zie
/// [`alleen_de_runtime`]).
pub fn cel_router(state: CelState) -> Router {
    let schrijven = Router::new()
        .route("/api/lexostatus/{naam}/proef", post(proef_route))
        .route("/api/grammen", post(grammen_route))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            alleen_de_runtime,
        ));
    Router::new()
        .route("/api/kroniek", get(kroniek_route))
        .route("/api/zaken/{zaakkenmerk}", get(zaak_van_cel_route))
        .route("/api/lexostatus/{naam}", get(lexostatus_route))
        .route("/api/stroom", get(stroom_route))
        .merge(schrijven)
        .with_state(state)
}

/// Laat een verzoek alleen door als het het token van de runtime draagt:
/// zonder token 401, met een ander token 403. Zo legt alleen een proces van
/// deze runtime vast, en niet iedereen die de poort bereikt.
async fn alleen_de_runtime(
    State(state): State<CelState>,
    verzoek: Request,
    verder: Next,
) -> Result<Response, Fout> {
    match verzoek.headers().get(RUNTIME_TOKEN_HEADER) {
        None => Err(fout(
            StatusCode::UNAUTHORIZED,
            "alleen een proces van deze runtime legt vast of reduceert op proef: het runtime-token ontbreekt",
        )),
        Some(t) if state.runtime_token.klopt(t.as_bytes()) => Ok(verder.run(verzoek).await),
        Some(_) => Err(fout(
            StatusCode::FORBIDDEN,
            "alleen een proces van deze runtime legt vast of reduceert op proef: het runtime-token klopt niet",
        )),
    }
}

/// Het zaakkenmerk van een gram. `zaak: opent` geeft een nieuw kenmerk;
/// `volgt` neemt dat uit het verzoek (of de kroniek die zaak kent, toetst
/// [`toets_zaak`]); `geen` geeft er geen.
fn zaakkenmerk_voor(
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
            Ok(Some(z))
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
    let zaakkenmerk = zaakkenmerk_voor(event, v.zaakkenmerk.clone())?;
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
pub fn als_yaml(cel: &Cel, gram: &Gram) -> Result<String, String> {
    let niet = |e: String| format!("gram '{}' is niet als YAML te schrijven: {e}", gram.name);
    let mut doc = match serde_yaml_ng::to_value(gram) {
        Ok(serde_yaml_ng::Value::Mapping(m)) => m,
        Ok(_) => return Err(niet("geen mapping".into())),
        Err(e) => return Err(niet(e.to_string())),
    };
    if let Some((_, event)) = cel.event(&gram.stroom.id, &gram.name) {
        doc.insert(
            serde_yaml_ng::Value::String("fields".into()),
            serde_yaml_ng::Value::Mapping(event.geordend(&gram.fields)),
        );
    }
    serde_yaml_ng::to_string(&doc).map_err(|e| niet(e.to_string()))
}

/// Of een gram vastlegbaar is in de zaak die het draagt, gegeven wat de
/// kronieken van de cel al bevatten. Dat beslist de cel, niet het proces
/// (paper: de cel bepaalt welke feiten vastlegbaar zijn).
///
/// - Een gram dat een zaak volgt, volgt een zaak die de kroniek kent.
/// - Een zaak doorloopt elke stage één keer: de stage-decretogrammen van één
///   besluit delen een zaakkenmerk, elk als eigen elementair gram (RFC-022
///   par. 1.2, RFC-008). Een tweede gram met dezelfde stage is een wijziging
///   van wat al vastligt, en die hoort in een eigen stap.
fn toets_zaak(gram: &Gram, bestaand: &[&Gram]) -> Result<(), Fout> {
    let Some(z) = gram.zaakkenmerk.as_deref() else {
        return Ok(());
    };
    let mut zaak = bestaand
        .iter()
        .filter(|g| g.zaakkenmerk.as_deref() == Some(z))
        .peekable();
    if gram.zaak == Zaak::Volgt && zaak.peek().is_none() {
        return Err(fout(
            StatusCode::BAD_REQUEST,
            format!("geen zaak '{z}' in de kroniek"),
        ));
    }
    if let Some(stage) = gram.stage.as_deref() {
        if let Some(eerder) = zaak.find(|g| g.stage.as_deref() == Some(stage)) {
            return Err(fout(
                StatusCode::CONFLICT,
                format!(
                    "in zaak {z} ligt al een gram met stage {stage} ('{}'); een zaak doorloopt elke stage één keer (RFC-022 par. 1.2)",
                    eerder.name
                ),
            ));
        }
    }
    Ok(())
}

/// Leg een gram vast. Antwoord: het gram, met YAML. De toets op de zaak en
/// het schrijven gebeuren onder één slot, zodat twee gelijktijdige verzoeken
/// niet allebei dezelfde stage vastleggen. Het schrijven wacht op de schijf,
/// dus het draait buiten de async-draden.
async fn grammen_route(
    State(state): State<CelState>,
    Json(verzoek): Json<Vastlegverzoek>,
) -> Result<(StatusCode, Json<Value>), Fout> {
    let gram = bouw(&state, &verzoek)?;
    let (kroniek, cel, g) = (state.kroniek.clone(), state.cel.clone(), gram.clone());
    tokio::task::spawn_blocking(move || {
        kroniek.voeg_toe_mits(&g, &cel.kronieken(), |bestaand| toets_zaak(&g, bestaand))
    })
    .await
    .map_err(|e| intern(format!("het vastleggen brak af: {e}")))?
    .map_err(intern)??;
    tracing::info!(cel = %state.cel.id(), zaakkenmerk = gram.zaakkenmerk.as_deref().unwrap_or("-"), name = %gram.name, "gram vastgelegd");
    let yaml = als_yaml(&state.cel, &gram).map_err(intern)?;
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
    if let Some(z) = &gram.zaakkenmerk {
        let zaak = state
            .kroniek
            .lees_zaak(&state.cel.kronieken(), z)
            .map_err(intern)?;
        toets_zaak(&gram, &zaak.iter().map(|v| &v.gram).collect::<Vec<_>>())?;
    }
    let mut inputs = verzoek.inputs;
    if let Some(z) = &gram.zaakkenmerk {
        if def.inputs.iter().any(|i| i.name == "zaakkenmerk") && !inputs.contains_key("zaakkenmerk")
        {
            inputs.insert("zaakkenmerk".into(), Value::String(z.clone()));
        }
    }
    inputs_compleet(def, &inputs)?;
    let kroniek = state.kroniek.lees(&def.reduction.kroniek).map_err(intern)?;
    // Het concept als laatste: bij gelijk moment kiest `kies: laatste` het.
    let grammen = kroniek
        .iter()
        .map(|v| &v.gram)
        .chain(std::iter::once(&gram));
    let lexostatus = reductie::reduceer(def, &inputs, grammen)
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

/// De grammen met hun YAML. De YAML van een gram wordt een keer gemaakt en
/// daarna bewaard.
fn met_yaml(state: &CelState, grammen: &[Arc<Vastgelegd>]) -> Result<Value, Fout> {
    let mut uit = Vec::with_capacity(grammen.len());
    for v in grammen {
        let yaml = v.yaml(|g| als_yaml(&state.cel, g)).map_err(intern)?;
        uit.push(json!({"gram": v.gram, "yaml": yaml}));
    }
    Ok(Value::Array(uit))
}

/// De kroniek van de cel: alle grammen, over al haar kronieken.
async fn kroniek_route(State(state): State<CelState>) -> Result<Json<Value>, Fout> {
    let grammen = state.kroniek.alle(&state.cel.kronieken()).map_err(intern)?;
    Ok(Json(met_yaml(&state, &grammen)?))
}

/// De grammen van één zaak, over alle kronieken van de cel. Het filteren op
/// de zaak gebeurt hier, in de cel; een proces krijgt alleen de zaak die het
/// vraagt.
async fn zaak_van_cel_route(
    State(state): State<CelState>,
    Path(zaakkenmerk): Path<String>,
) -> Result<Json<Value>, Fout> {
    let grammen = state
        .kroniek
        .lees_zaak(&state.cel.kronieken(), &zaakkenmerk)
        .map_err(intern)?;
    if grammen.is_empty() {
        return Err(fout(
            StatusCode::NOT_FOUND,
            format!("geen zaak '{zaakkenmerk}' in de kroniek"),
        ));
    }
    Ok(Json(met_yaml(&state, &grammen)?))
}

async fn lexostatus_route(
    State(state): State<CelState>,
    Path(naam): Path<String>,
    Query(inputs): Query<Map<String, Value>>,
) -> Result<Json<Lexostatus>, Fout> {
    let def = lexostatus_def(&state, &naam)?;
    inputs_compleet(def, &inputs)?;
    let grammen = state.kroniek.lees(&def.reduction.kroniek).map_err(intern)?;
    reductie::reduceer(def, &inputs, grammen.iter().map(|v| &v.gram))
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

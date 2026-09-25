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
use crate::datum;
use crate::gram::Gram;
use crate::kroniek::{Kroniek, Vastgelegd};
use crate::reductie::{self, Lexostatus, Peil};
use crate::stroom::{self, Indiening, Zaak};
use crate::transport::{LeesToken, RuntimeToken, LEES_TOKEN_HEADER, RUNTIME_TOKEN_HEADER};

/// De toestand van een cel in de runtime.
#[derive(Clone)]
pub struct CelState {
    pub cel: Arc<Cel>,
    pub kroniek: Arc<Kroniek>,
    pub klok: Klok,
    /// Wie dit token meestuurt, is een proces van deze runtime; alleen die
    /// mag vastleggen of op proef reduceren.
    pub runtime_token: RuntimeToken,
    /// Wie dit token meestuurt, mag lezen (een andere runtime met hetzelfde
    /// `CEL_LEES_TOKEN`). Zonder leest alleen de eigen runtime.
    pub lees_token: Option<LeesToken>,
}

/// De routes van een cel, relatief aan `/cellen/<id>`. Vastleggen en op
/// proef reduceren vragen het runtime-token (zie [`alleen_de_runtime`]);
/// lezen (de kroniek, een zaak, een lexostatus) het runtime-token of het
/// leestoken (zie [`alleen_lezers`]), want de grammen dragen de identiteit
/// en de intake van wie indiende. Alleen de stroomdefinities zijn open: die
/// zeggen niets over iemand.
pub fn cel_router(state: CelState) -> Router {
    let schrijven = Router::new()
        .route("/api/lexostatus/{naam}/proef", post(proef_route))
        .route("/api/grammen", post(grammen_route))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            alleen_de_runtime,
        ));
    let lezen = Router::new()
        .route("/api/kroniek", get(kroniek_route))
        .route("/api/zaken/{zaakkenmerk}", get(zaak_van_cel_route))
        .route("/api/lexostatus/{naam}", get(lexostatus_route))
        .route_layer(middleware::from_fn_with_state(state.clone(), alleen_lezers));
    Router::new()
        .route("/api/stroom", get(stroom_route))
        .merge(lezen)
        .merge(schrijven)
        .with_state(state)
}

/// Of een verzoek een token draagt dat toegang geeft: het runtime-token, of
/// bij lezen ook het leestoken. Zonder token 401, met een verkeerd 403.
fn toegang(state: &CelState, verzoek: &Request, lezen: bool) -> Result<(), Fout> {
    let h = verzoek.headers();
    let runtime = h.get(RUNTIME_TOKEN_HEADER);
    let lees = h.get(LEES_TOKEN_HEADER).filter(|_| lezen);
    if runtime.is_some_and(|t| state.runtime_token.klopt(t.as_bytes()))
        || lees.is_some_and(|t| {
            state
                .lees_token
                .as_ref()
                .is_some_and(|l| l.klopt(t.as_bytes()))
        })
    {
        return Ok(());
    }
    let wat = if lezen {
        "alleen een proces van deze runtime, of een runtime met het leestoken, leest een kroniek, een zaak of een lexostatus"
    } else {
        "alleen een proces van deze runtime legt vast of reduceert op proef"
    };
    let token = if lezen {
        "het runtime-token of het leestoken"
    } else {
        "het runtime-token"
    };
    Err(if runtime.is_none() && lees.is_none() {
        fout(
            StatusCode::UNAUTHORIZED,
            format!("{wat}: {token} ontbreekt"),
        )
    } else {
        fout(StatusCode::FORBIDDEN, format!("{wat}: {token} klopt niet"))
    })
}

/// Laat een verzoek alleen door als het het token van de runtime draagt:
/// zonder token 401, met een ander token 403. Zo legt alleen een proces van
/// deze runtime vast, en niet iedereen die de poort bereikt.
async fn alleen_de_runtime(
    State(state): State<CelState>,
    verzoek: Request,
    verder: Next,
) -> Result<Response, Fout> {
    toegang(&state, &verzoek, false)?;
    Ok(verder.run(verzoek).await)
}

/// Laat een leesverzoek alleen door met het runtime-token of het leestoken.
/// Een behandelaar of beheerder leest via een proces (zie
/// [`super::proces`], de inzage), niet rechtstreeks.
async fn alleen_lezers(
    State(state): State<CelState>,
    verzoek: Request,
    verder: Next,
) -> Result<Response, Fout> {
    toegang(&state, &verzoek, true)?;
    Ok(verder.run(verzoek).await)
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
            vastgelegd_op: (state.klok)(),
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
        gram.handelende_actor = b.handelende_actor.clone();
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

/// Of een gram past in de zaak die het draagt, gegeven wat de kronieken van
/// de cel al bevatten. De cel dwingt de vorm af, nooit de inhoud: wat een
/// handeling waard is, concludeert het proces voor het handelt. Welke feiten
/// vastlegbaar zijn, laat de paper open (P:110, een vraag voor verder
/// onderzoek: eisen aan de vorm zonder de inhoud te beperken); deze grens is
/// een eigen keuze (RFC-044 par. 1).
///
/// - Een gram dat een zaak volgt, volgt een zaak die de kroniek kent, en ligt
///   rechtens niet voor die zaak (zie [`niet_voor_de_zaak`]).
/// - Een zaak doorloopt elke stage één keer: de stage-grammen van één
///   besluit delen een zaakkenmerk, elk als eigen elementair gram (RFC-022
///   par. 1.2, RFC-008). Een tweede gram met dezelfde stage is een wijziging
///   van wat al vastligt, en die hoort in een eigen stap.
/// - Zegt het verzoek hoeveel grammen de zaak had toen het proces haar las
///   (`zaak_grammen`), dan legt de cel alleen vast als dat nog zo is. Wat het
///   proces uitrekende (zoals wat er nog te betalen is), gold voor de zaak
///   zoals die toen was; twee gelijktijdige betalingen komen zo niet allebei
///   door.
fn toets_zaak(gram: &Gram, bestaand: &[&Gram], verwacht: Option<usize>) -> Result<(), Fout> {
    let Some(z) = gram.zaakkenmerk.as_deref() else {
        return Ok(());
    };
    let zaak: Vec<&&Gram> = bestaand
        .iter()
        .filter(|g| g.zaakkenmerk.as_deref() == Some(z))
        .collect();
    if gram.zaak == Zaak::Volgt && zaak.is_empty() {
        return Err(fout(
            StatusCode::BAD_REQUEST,
            format!("geen zaak '{z}' in de kroniek"),
        ));
    }
    niet_voor_de_zaak(gram, &zaak)?;
    if let Some(n) = verwacht {
        if zaak.len() != n {
            return Err(fout(
                StatusCode::CONFLICT,
                format!(
                    "zaak {z} veranderde sinds het proces haar las ({n} grammen, nu {}); reken de handeling opnieuw uit",
                    zaak.len()
                ),
            ));
        }
    }
    let mut zaak = zaak.into_iter();
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

/// Een gram dat een zaak volgt, ligt rechtens niet voor de zaak: de dag van
/// zijn `op_moment` ligt niet voor die van het laatste `op_moment` in de zaak.
/// Een zaak loopt vooruit in de tijd: een besluit van voor de aanvraag, of
/// een bekendmaking van voor het besluit, is geen feit van deze zaak. Het
/// gaat om de dag, omdat een gebonden moment vaak een datum is (het begin
/// van die dag) en het feit ervoor op dezelfde dag later kan zijn
/// vastgelegd. Een ongebonden `op_moment` is het moment van vastleggen en
/// ligt daarom nooit voor de zaak.
fn niet_voor_de_zaak(gram: &Gram, zaak: &[&&Gram]) -> Result<(), Fout> {
    if gram.zaak != Zaak::Volgt {
        return Ok(());
    }
    let dag =
        datum::peildatum_van(&gram.op_moment).map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    let mut laatste: Option<(String, &str)> = None;
    for g in zaak {
        let d = datum::peildatum_van(&g.op_moment).map_err(intern)?;
        if laatste.as_ref().is_none_or(|(l, _)| d > *l) {
            laatste = Some((d, g.name.as_str()));
        }
    }
    match laatste {
        Some((l, naam)) if dag < l => Err(fout(
            StatusCode::CONFLICT,
            format!(
                "op_moment {dag} ligt voor de zaak: het laatste feit erin ('{naam}') geldt op {l}; een zaak loopt vooruit in de tijd"
            ),
        )),
        _ => Ok(()),
    }
}

/// Leg een gram vast. Antwoord: het gram, met YAML. Het stempelen
/// (`vastgelegd_op`), de toets op de zaak en het schrijven gebeuren onder één
/// slot: twee gelijktijdige verzoeken leggen niet allebei dezelfde stage vast,
/// en de volgorde in het bestand is die van `vastgelegd_op`. Het schrijven
/// wacht op de schijf, dus het draait buiten de async-draden.
async fn grammen_route(
    State(state): State<CelState>,
    Json(verzoek): Json<Vastlegverzoek>,
) -> Result<(StatusCode, Json<Value>), Fout> {
    let gram = bouw(&state, &verzoek)?;
    let (kroniek, cel, klok) = (state.kroniek.clone(), state.cel.clone(), state.klok.clone());
    let verwacht = verzoek.zaak_grammen;
    let gram = tokio::task::spawn_blocking(move || {
        kroniek.leg_vast_mits(
            gram,
            &cel.kronieken(),
            || klok(),
            |g, bestaand| toets_zaak(g, bestaand, verwacht),
        )
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
    /// De inputs van de lexostatus, en zo nodig het peil (`peilmoment`,
    /// `bekend_op`), zoals bij `GET lexostatus`.
    #[serde(default)]
    inputs: Map<String, Value>,
}

/// Een proefreductie: het gram van het concept in het geheugen, de kroniek
/// mét dat gram gereduceerd. Zonder input `zaakkenmerk` telt dat van het
/// concept. Een concept is geen feit: niets wordt vastgelegd. Het concept
/// telt als vastgelegd op de klok van nu; met een peil geldt voor het concept
/// hetzelfde als voor elk ander gram.
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
        toets_zaak(
            &gram,
            &zaak.iter().map(|v| &v.gram).collect::<Vec<_>>(),
            None,
        )?;
    }
    let mut inputs = verzoek.inputs;
    let peil = Peil::uit_query(&mut inputs).map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    if let Some(z) = &gram.zaakkenmerk {
        if def.inputs.iter().any(|i| i.name == "zaakkenmerk") && !inputs.contains_key("zaakkenmerk")
        {
            inputs.insert("zaakkenmerk".into(), Value::String(z.clone()));
        }
    }
    inputs_compleet(def, &inputs)?;
    let kroniek = state.kroniek.lees(&def.reduction.kroniek).map_err(intern)?;
    // Het concept als laatste: bij gelijke momenten kiest `kies: laatste`
    // het. Een concept met een eerder op_moment (een eerdere ontvangst) is
    // niet vanzelf het laatste.
    let grammen = kroniek
        .iter()
        .map(|v| &v.gram)
        .chain(std::iter::once(&gram));
    let lexostatus = reductie::reduceer_op(def, &inputs, grammen, &peil)
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

/// Een lexostatus: de kroniek gereduceerd, met de inputs als query. Met
/// `peilmoment` en/of `bekend_op` (een datum of een moment) op een eerder
/// moment: zie [`Peil`]. De lexostatus [`reductie::ZAAKSTAND`] biedt de
/// runtime zelf aan, voor elke cel met een zaak (zie [`zaakstand`]).
async fn lexostatus_route(
    State(state): State<CelState>,
    Path(naam): Path<String>,
    Query(mut inputs): Query<Map<String, Value>>,
) -> Result<Json<Lexostatus>, Fout> {
    let peil = Peil::uit_query(&mut inputs).map_err(|e| fout(StatusCode::BAD_REQUEST, e))?;
    if naam == reductie::ZAAKSTAND && state.cel.heeft_zaken() {
        return zaakstand(&state, &inputs, &peil).map(Json);
    }
    let def = lexostatus_def(&state, &naam)?;
    inputs_compleet(def, &inputs)?;
    let grammen = state.kroniek.lees(&def.reduction.kroniek).map_err(intern)?;
    reductie::reduceer_op(def, &inputs, grammen.iter().map(|v| &v.gram), &peil)
        .map_err(|e| fout(StatusCode::BAD_REQUEST, e))?
        .map(Json)
        .ok_or_else(|| fout(StatusCode::NOT_FOUND, "geen gram voor deze vraag"))
}

/// De stand van een zaak (zie [`reductie::Zaakstand`]): de cel filtert de
/// grammen van de zaak en leidt af wat een proces erover vraagt. Input
/// `zaakkenmerk`; met `eigenaar_pad` (een `$intake`-pad zonder `$intake.`)
/// en `eigenaar` ook of iemand met die waarde de zaak kent. 404 als de cel
/// de zaak niet kent.
fn zaakstand(
    state: &CelState,
    inputs: &Map<String, Value>,
    peil: &Peil,
) -> Result<Lexostatus, Fout> {
    let tekst = |k: &str| {
        inputs
            .get(k)
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
    };
    let z = tekst("zaakkenmerk")
        .ok_or_else(|| fout(StatusCode::BAD_REQUEST, "input 'zaakkenmerk' ontbreekt"))?;
    let eigenaar = match (tekst(reductie::EIGENAAR_PAD), tekst(reductie::EIGENAAR)) {
        (Some(p), Some(w)) => Some((p, w)),
        (None, None) => None,
        _ => {
            return Err(fout(
                StatusCode::BAD_REQUEST,
                "vraag naar de eigenaar met eigenaar_pad en eigenaar samen",
            ))
        }
    };
    let grammen = state
        .kroniek
        .lees_zaak(&state.cel.kronieken(), z)
        .map_err(intern)?;
    let cel = &state.cel;
    let bindt = |g: &Gram, pad: &str| -> Vec<String> {
        let Some((_, event)) = cel.event(&g.stroom.id, &g.name) else {
            return Vec::new();
        };
        event
            .bladeren()
            .into_iter()
            .filter(|b| b.binding == stroom::Binding::Intake(pad.to_string()))
            .map(|b| b.pad)
            .collect()
    };
    reductie::reduceer_zaak(grammen.iter().map(|v| &v.gram), peil, eigenaar, bindt)
        .map_err(intern)?
        .ok_or_else(|| {
            fout(
                StatusCode::NOT_FOUND,
                format!("geen zaak '{z}' in de kroniek"),
            )
        })?
        .als_lexostatus(z, peil)
        .map_err(intern)
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
    let mut lexostatussen: Vec<Value> = cel
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
    if cel.heeft_zaken() {
        // De stand van een zaak biedt de runtime aan, niet de configuratie.
        lexostatussen.push(json!({
            "name": reductie::ZAAKSTAND,
            "inputs": [{"name": "zaakkenmerk", "type": "string"}],
            "lijst": false,
            "parameters": [],
            "kolommen": [],
            "extra_velden": ["grammen", "events", "laatste_op_moment", "stages", "eigenaar"],
            "runtime": true,
        }));
    }
    json!({
        "id": cel.id(),
        "recording_actor": cel.definitie.recording_actor,
        "kronieken": cel.kronieken(),
        "lexostatussen": lexostatussen,
    })
}

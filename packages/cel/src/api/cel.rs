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

use super::{error, intern, Error, Klok};
use crate::cel::Cell;
use crate::celclient::Vastlegverzoek;
use crate::datum;
use crate::gram::Gram;
use crate::kroniek::{Kroniek, Vastgelegd, Zicht};
use crate::lexostatus_engine;
use crate::reductie::{self, Lexostatus, Peil, Reductieroute};
use crate::stroom::{self, Decision, Indiening};
use crate::transport::{LeesToken, RuntimeToken, LEES_TOKEN_HEADER, RUNTIME_TOKEN_HEADER};

/// De toestand van een cel in de runtime.
#[derive(Clone)]
pub struct CelState {
    pub cell: Arc<Cell>,
    pub chronicle: Arc<Kroniek>,
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
        .route("/api/lexostatus/{name}/trial", post(proef_route))
        .route("/api/grams", post(grammen_route))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            alleen_de_runtime,
        ));
    let lezen = Router::new()
        .route("/api/chronicle", get(kroniek_route))
        .route("/api/cases/{root}", get(zaak_van_cel_route))
        .route("/api/lexostatus/{name}", get(lexostatus_route))
        .route_layer(middleware::from_fn_with_state(state.clone(), alleen_lezers));
    Router::new()
        .route("/api/stream", get(stroom_route))
        .merge(lezen)
        .merge(schrijven)
        .with_state(state)
}

/// Of een verzoek een token draagt dat toegang geeft: het runtime-token, of
/// bij lezen ook het leestoken. Zonder token 401, met een verkeerd 403.
fn toegang(state: &CelState, verzoek: &Request, lezen: bool) -> Result<(), Error> {
    let h = verzoek.headers();
    let runtime = h.get(RUNTIME_TOKEN_HEADER);
    let lees = h.get(LEES_TOKEN_HEADER);
    if runtime.is_some_and(|t| state.runtime_token.klopt(t.as_bytes()))
        || lees.filter(|_| lezen).is_some_and(|t| {
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
        error(
            StatusCode::UNAUTHORIZED,
            format!("{wat}: {token} ontbreekt"),
        )
    } else {
        error(StatusCode::FORBIDDEN, format!("{wat}: {token} klopt niet"))
    })
}

/// Laat een verzoek alleen door als het het token van de runtime draagt:
/// zonder token 401, met een ander token 403. Zo legt alleen een proces van
/// deze runtime vast, en niet iedereen die de poort bereikt.
async fn alleen_de_runtime(
    State(state): State<CelState>,
    verzoek: Request,
    verder: Next,
) -> Result<Response, Error> {
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
) -> Result<Response, Error> {
    toegang(&state, &verzoek, true)?;
    Ok(verder.run(verzoek).await)
}

/// Bouw een gram uit een verzoek, zonder het vast te leggen. De actor moet de
/// `recording_actor` van de stroom zijn. Of de verwijzingen bestaan en
/// passen, toetst [`toets_verwijzingen`] onder het slot; het id geeft de cel
/// daar ook.
fn bouw(state: &CelState, v: &Vastlegverzoek) -> Result<Gram, Error> {
    let (stream, event) = state.cell.event(&v.stream, &v.event).ok_or_else(|| {
        error(
            StatusCode::BAD_REQUEST,
            format!(
                "cel '{}' heeft geen event '{}' in stroom '{}'",
                state.cell.id(),
                v.event,
                v.stream
            ),
        )
    })?;
    if v.actor != stream.recording_actor {
        return Err(error(
            StatusCode::FORBIDDEN,
            format!(
                "actor '{}' legt niet vast in stroom '{}': de recording_actor is '{}'",
                v.actor, stream.id, stream.recording_actor
            ),
        ));
    }
    let mut gram = stroom::bouw_gram(
        stream,
        event,
        &Indiening {
            intake: &v.intake,
            external: &v.external,
            recorded_at: (state.klok)(),
            refers_to: &v.refers_to,
        },
    )
    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    if let Some(b) = &v.decision {
        gram.legal_character = b.legal_character.clone();
        gram.decision_type = b.decision_type.clone();
        gram.regulation = b.regulation.clone();
        gram.regulation_valid_from = b.regulation_valid_from.clone();
        gram.competent_authority = b.competent_authority.clone();
        gram.acting_actor = b.acting_actor.clone();
        gram.inputs = b.inputs.clone();
        gram.receipt = b.receipt.clone();
    }
    Ok(gram)
}

/// Valideer een gebouwd gram tegen `gram.json`; een 400 als het niet past.
fn valideer(cell: &Cell, gram: &Gram) -> Result<(), Error> {
    gram.valideer().map_err(|f| {
        error(
            StatusCode::BAD_REQUEST,
            format!("gram valideert niet: {}", f.join("; ")),
        )
    })?;
    reductie::datums_in_orde(&cell.lexostatuses.lexostatus_definitions, gram)
        .map_err(|f| error(StatusCode::BAD_REQUEST, f))
}

/// Het gram als YAML, velden in de volgorde van de stroom.
pub fn als_yaml(cell: &Cell, gram: &Gram) -> Result<String, String> {
    let niet = |e: String| format!("gram '{}' is niet als YAML te schrijven: {e}", gram.name);
    let mut doc = match serde_yaml_ng::to_value(gram) {
        Ok(serde_yaml_ng::Value::Mapping(m)) => m,
        Ok(_) => return Err(niet("geen mapping".into())),
        Err(e) => return Err(niet(e.to_string())),
    };
    if let Some((_, event)) = cell.event(&gram.stream.id, &gram.name) {
        doc.insert(
            serde_yaml_ng::Value::String("fields".into()),
            serde_yaml_ng::Value::Mapping(event.geordend(&gram.fields)),
        );
    }
    serde_yaml_ng::to_string(&doc).map_err(|e| niet(e.to_string()))
}

/// Of een gram past bij de grammen waarnaar het verwijst, gegeven wat de
/// cel onder haar slot ziet (`zicht`: de doelen en de groep van de wortel).
/// De cel dwingt de vorm af, nooit de inhoud: wat een handeling waard is,
/// concludeert het proces voor het handelt. Welke feiten vastlegbaar zijn,
/// laat de paper open (P:110); deze grens is een eigen keuze (RFC-044 par. 1
/// en 4), per verwijzing in plaats van per zaak.
///
/// - Elk gram waarnaar verwezen wordt, ligt in de kronieken van de cel, past
///   bij wat de verwijzing mag aanwijzen (`naar`: een artikel dat het
///   vestigt, een event of een stage), en alle doelen hebben dezelfde wortel.
/// - Het gram ligt rechtens niet voor een gram waarnaar het verwijst: de dag
///   van zijn `op_moment` ligt niet voor die van het doel (zwakker dan de
///   oude regel "niet voor het laatste feit in de zaak": een betaling op het
///   voorschot mag later vastgelegd worden dan de vaststelling).
/// - Een besluit (stage BESLUIT, geen wijziging) van hetzelfde event dat
///   naar hetzelfde gram verwijst, ligt er ten hoogste een keer: een ander
///   besluit over dezelfde aanvraag vraagt een eigen grondslag (`wijzigt`).
///   Een ander gram met een stage (zoals de bekendmaking) ligt ten hoogste
///   een keer per stage en per doel: een besluit wordt een keer bekendgemaakt.
/// - Zegt het verzoek hoeveel grammen de groep had toen het proces haar las
///   (`wortel_grammen`), dan legt de cel alleen vast als dat nog zo is.
fn toets_verwijzingen(
    cell: &Cell,
    gram: &Gram,
    zicht: &Zicht<'_>,
    verwacht: Option<usize>,
) -> Result<(), Error> {
    if let Some(f) = &zicht.wortelfout {
        return Err(error(StatusCode::BAD_REQUEST, f.clone()));
    }
    let event = cell.event(&gram.stream.id, &gram.name).map(|(_, e)| e);
    for (name, id) in &gram.refers_to {
        let Some(doel) = zicht.doelen.get(id) else {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!("geen gram '{id}' in de kroniek ({name})"),
            ));
        };
        if let Some(v) = event.and_then(|e| e.refers_to.get(name)) {
            let doel_event = cell.event(&doel.stream.id, &doel.name).map(|(_, e)| e);
            if !v.to.past(doel, doel_event) {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "'{name}' wijst naar {}, maar gram {id} is '{}'",
                        v.to, doel.name
                    ),
                ));
            }
        }
        niet_voor(gram, doel)?;
    }
    // Een gram zonder verwijzing is zijn eigen wortel: het heeft (nog) geen
    // groep om te vergelijken.
    if let Some(n) = verwacht.filter(|_| !gram.refers_to.is_empty()) {
        if zicht.group.len() != n {
            return Err(error(
                StatusCode::CONFLICT,
                format!(
                    "de groep van wortel {} veranderde sinds het proces haar las ({n} grammen, nu {}); reken de handeling opnieuw uit",
                    gram.root.as_deref().unwrap_or("-"),
                    zicht.group.len()
                ),
            ));
        }
    }
    let Some(stage) = gram.stage.as_deref() else {
        return Ok(());
    };
    let role = event.and_then(|e| e.decision);
    // Hetzelfde doel onder dezelfde naam: een ander besluit onder `besluit`
    // bij dezelfde `aanvraag` is een ander feit.
    let deelt = |g: &Gram| {
        g.refers_to
            .iter()
            .any(|(name, d)| gram.refers_to.get(name) == Some(d))
    };
    if role.is_some_and(Decision::is_besluit) {
        if role == Some(Decision::Opens) {
            if let Some(eerder) = zicht
                .group
                .iter()
                .find(|g| g.name == gram.name && g.stream.id == gram.stream.id && deelt(g))
            {
                return Err(error(
                    StatusCode::CONFLICT,
                    format!(
                        "er ligt al een besluit '{}' ({}) dat naar hetzelfde gram verwijst; een ander besluit hierover vraagt een eigen grondslag, een event met een verwijzing wijzigt",
                        eerder.name, eerder.id
                    ),
                ));
            }
        }
        return Ok(());
    }
    if let Some(eerder) = zicht
        .group
        .iter()
        .find(|g| g.stage.as_deref() == Some(stage) && deelt(g))
    {
        let doel = eerder
            .refers_to
            .iter()
            .find(|(name, d)| gram.refers_to.get(*name) == Some(d))
            .map_or("-", |(_, d)| d.as_str());
        return Err(error(
            StatusCode::CONFLICT,
            format!(
                "bij gram {doel} ligt al een gram met stage {stage} ('{}'); een besluit doorloopt elke stage één keer (RFC-022 par. 1.2)",
                eerder.name
            ),
        ));
    }
    Ok(())
}

/// Een gram ligt rechtens niet voor een gram waarnaar het verwijst: de dag
/// van zijn `op_moment` ligt niet voor die van het doel. Het gaat om de dag,
/// omdat een gebonden moment vaak een datum is (het begin van die dag) en
/// het doel op dezelfde dag later kan zijn vastgelegd. Een ongebonden
/// `op_moment` is het moment van vastleggen en ligt daarom nooit ervoor.
fn niet_voor(gram: &Gram, doel: &Gram) -> Result<(), Error> {
    let dag =
        datum::peildatum_van(&gram.effective_at).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let d = datum::peildatum_van(&doel.effective_at).map_err(intern)?;
    if dag < d {
        return Err(error(
            StatusCode::CONFLICT,
            format!(
                "op_moment {dag} ligt voor het gram waarnaar het verwijst ('{}', {d}); wat volgt, loopt vooruit in de tijd",
                doel.name
            ),
        ));
    }
    Ok(())
}

/// Leg een gram vast. Antwoord: het gram, met YAML. Het stempelen
/// (`vastgelegd_op`), de toets op de zaak en het schrijven gebeuren onder één
/// slot: twee gelijktijdige verzoeken leggen niet allebei dezelfde stage vast,
/// en de volgorde in het bestand is die van `vastgelegd_op`. Het schrijven
/// wacht op de schijf, dus het draait buiten de async-draden.
async fn grammen_route(
    State(state): State<CelState>,
    Json(verzoek): Json<Vastlegverzoek>,
) -> Result<(StatusCode, Json<Value>), Error> {
    let gram = bouw(&state, &verzoek)?;
    let (chronicle, cell, klok) = (
        state.chronicle.clone(),
        state.cell.clone(),
        state.klok.clone(),
    );
    let verwacht = verzoek.root_grams;
    let gram = tokio::task::spawn_blocking(move || {
        chronicle.leg_vast_mits(
            gram,
            &cell.chronicles(),
            || klok(),
            |f| error(StatusCode::BAD_REQUEST, f),
            |g, zicht| {
                toets_verwijzingen(&cell, g, zicht, verwacht)?;
                valideer(&cell, g)
            },
        )
    })
    .await
    .map_err(|e| intern(format!("het vastleggen brak af: {e}")))?
    .map_err(intern)??;
    tracing::info!(cell = %state.cell.id(), id = %gram.gram.id, root = gram.gram.root.as_deref().unwrap_or("-"), name = %gram.gram.name, "gram vastgelegd");
    // De YAML komt in het vastgelegde gram, zodat een latere lezing haar niet
    // opnieuw maakt.
    let yaml = gram.yaml(|g| als_yaml(&state.cell, g)).map_err(intern)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"gram": gram.gram, "yaml": yaml})),
    ))
}

#[derive(Deserialize)]
struct Proefverzoek {
    draft: Vastlegverzoek,
    /// De inputs van de lexostatus, en zo nodig het peil (`peilmoment`,
    /// `bekend_op`), zoals bij `GET lexostatus`.
    #[serde(default)]
    inputs: Map<String, Value>,
}

/// Een proefreductie: het gram van het concept in het geheugen, de kroniek
/// mét dat gram gereduceerd. Zonder input `wortel` telt die van het
/// concept. Een concept is geen feit: niets wordt vastgelegd. Het concept
/// telt als vastgelegd op de klok van nu; met een peil geldt voor het concept
/// hetzelfde als voor elk ander gram.
async fn proef_route(
    State(state): State<CelState>,
    Path(name): Path<String>,
    Json(verzoek): Json<Proefverzoek>,
) -> Result<Json<Value>, Error> {
    let def = lexostatus_def(&state, &name)?;
    let mut gram = bouw(&state, &verzoek.draft)?;
    let zicht = state
        .chronicle
        .zicht_voor(&state.cell.chronicles(), &mut gram)
        .map_err(intern)?;
    toets_verwijzingen(&state.cell, &gram, &zicht.zicht(), None)?;
    valideer(&state.cell, &gram)?;
    let mut inputs = verzoek.inputs;
    let peil = Peil::uit_query(&mut inputs).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    if let Some(w) = &gram.root {
        if def.inputs.iter().any(|i| i.name == WORTEL) && !inputs.contains_key(WORTEL) {
            inputs.insert(WORTEL.into(), Value::String(w.clone()));
        }
    }
    inputs_compleet(def, &inputs)?;
    let chronicle = grammen_voor(&state, def, &inputs)?;
    // Het concept als laatste: bij gelijke momenten kiest `kies: laatste`
    // het. Een concept met een eerder op_moment (een eerdere ontvangst) is
    // niet vanzelf het laatste.
    let grams = chronicle
        .iter()
        .map(|v| &v.gram)
        .chain(std::iter::once(&gram));
    let lexostatus = reduceer(&state, def, &inputs, grams, &peil, false)?;
    Ok(Json(json!({"gram": gram, "lexostatus": lexostatus})))
}

fn lexostatus_def<'s>(
    state: &'s CelState,
    name: &str,
) -> Result<&'s reductie::LexostatusDefinitie, Error> {
    state
        .cell
        .lexostatuses
        .lexostatus(name)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("geen lexostatus '{name}'")))
}

/// De naam van de input en de filtersleutel van de wortel.
pub const WORTEL: &str = "root";

/// De grammen die een reductie leest: de kroniek van de definitie, en als
/// haar filter op de wortel van een input filtert (en zij geen lijst is),
/// alleen de grammen van die wortel, uit de index per wortel. Het filter
/// zelf past de reductie daarna toe; dit scheelt alleen het lezen van de
/// rest van de kroniek.
fn grammen_voor(
    state: &CelState,
    def: &reductie::LexostatusDefinitie,
    inputs: &Map<String, Value>,
) -> Result<Vec<Arc<Vastgelegd>>, Error> {
    let r = &def.reduction;
    let root = r
        .filter
        .get(WORTEL)
        .filter(|_| r.group_by.is_none())
        .and_then(|v| match v.strip_prefix('$') {
            Some(input) => inputs.get(input).and_then(Value::as_str),
            None => Some(v.as_str()),
        });
    match root {
        Some(w) => state.chronicle.lees_wortel(&[r.chronicle.as_str()], w),
        None => state.chronicle.lees(&r.chronicle),
    }
    .map_err(intern)
}

fn inputs_compleet(
    def: &reductie::LexostatusDefinitie,
    inputs: &Map<String, Value>,
) -> Result<(), Error> {
    for i in &def.inputs {
        if !inputs.get(&i.name).is_some_and(reductie::filled) {
            return Err(error(
                StatusCode::BAD_REQUEST,
                format!("input '{}' ontbreekt", i.name),
            ));
        }
    }
    Ok(())
}

/// De grammen met hun YAML. De YAML van een gram wordt een keer gemaakt en
/// daarna bewaard.
fn met_yaml(state: &CelState, grams: &[Arc<Vastgelegd>]) -> Result<Value, Error> {
    let mut uit = Vec::with_capacity(grams.len());
    for v in grams {
        let yaml = v.yaml(|g| als_yaml(&state.cell, g)).map_err(intern)?;
        uit.push(json!({"gram": v.gram, "yaml": yaml}));
    }
    Ok(Value::Array(uit))
}

/// De kroniek van de cel: alle grammen, over al haar kronieken.
async fn kroniek_route(State(state): State<CelState>) -> Result<Json<Value>, Error> {
    let grams = state
        .chronicle
        .alle(&state.cell.chronicles())
        .map_err(intern)?;
    Ok(Json(met_yaml(&state, &grams)?))
}

/// De grammen met één wortel, over alle kronieken van de cel. Het filteren
/// gebeurt hier, in de cel; een proces krijgt alleen de groep die het vraagt.
async fn zaak_van_cel_route(
    State(state): State<CelState>,
    Path(root): Path<String>,
) -> Result<Json<Value>, Error> {
    let grams = state
        .chronicle
        .lees_wortel(&state.cell.chronicles(), &root)
        .map_err(intern)?;
    if grams.is_empty() {
        return Err(error(
            StatusCode::NOT_FOUND,
            format!("geen wortel '{root}' in de kroniek"),
        ));
    }
    Ok(Json(met_yaml(&state, &grams)?))
}

/// Een lexostatus: de kroniek gereduceerd, met de inputs als query. Met
/// `peilmoment` en/of `bekend_op` (een datum of een moment) op een eerder
/// moment: zie [`Peil`]. De lexostatus [`reductie::ZAAKSTAND`] biedt de
/// runtime zelf aan, voor elke cel met een zaak (zie [`zaakstand`]).
async fn lexostatus_route(
    State(state): State<CelState>,
    Path(name): Path<String>,
    Query(mut inputs): Query<Map<String, Value>>,
) -> Result<Json<Lexostatus>, Error> {
    let peil = Peil::uit_query(&mut inputs).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    // Alleen met de engine-route is `engine_trace` geen input.
    let met_trace = state.cell.route.is_some() && inputs.remove(ENGINE_TRACE).is_some();
    if name == reductie::ZAAKSTAND && state.cell.heeft_zaken() {
        // Geen reductie van een lexostatus-definitie maar code van de
        // runtime; met de engine-route zegt de lexostatus dat ook.
        return zaakstand(&state, &inputs, &peil)
            .map(|l| Lexostatus {
                reduction: state.cell.route.as_ref().map(|_| Reductieroute {
                    route: "runtime".into(),
                    regulation: None,
                    reason: Some(ZAAKSTAND_REDEN.into()),
                    duration_us: 0,
                    dsl_duration_us: None,
                    trace_text: None,
                }),
                ..l
            })
            .map(Json);
    }
    let def = lexostatus_def(&state, &name)?;
    inputs_compleet(def, &inputs)?;
    let grams = grammen_voor(&state, def, &inputs)?;
    reduceer(
        &state,
        def,
        &inputs,
        grams.iter().map(|v| &v.gram),
        &peil,
        met_trace,
    )
    .map(Json)
}

/// De query-parameter die bij de engine-route de trace van de engine-run
/// vraagt (`?engine_trace=1`); geen input van de lexostatus.
pub const ENGINE_TRACE: &str = "engine_trace";

/// Waarom de zaakstand niet via de engine gaat, in de route.
const ZAAKSTAND_REDEN: &str = "de stand van een zaak biedt de runtime zelf aan";

/// De dag waarop de engine de regeling van een lexostatus leest: die van het
/// peilmoment, zonder peilmoment vandaag.
fn engine_datum(state: &CelState, peil: &Peil) -> String {
    peil.as_of
        .and_then(|t| datum::datum_van(&t.to_string()))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| datum::reference_date(&(state.klok)()))
}

/// Reduceer een lexostatus, op de ene plek waar de cel dat doet: langs de
/// reductie-DSL, of, in een runtime met de engine-route, langs de route van
/// de cel (zie [`lexostatus_engine`]). 404: de lexostatus kiest een gram en
/// er is er geen.
fn reduceer<'g>(
    state: &CelState,
    def: &reductie::LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
    peil: &Peil,
    met_trace: bool,
) -> Result<Lexostatus, Error> {
    match &state.cell.route {
        None => reductie::reduceer_op(def, inputs, grams, peil),
        Some(route) => lexostatus_engine::reduceer_lexostatus(
            route,
            def,
            inputs,
            grams,
            peil,
            &engine_datum(state, peil),
            met_trace,
        ),
    }
    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?
    .ok_or_else(|| error(StatusCode::NOT_FOUND, "geen gram voor deze vraag"))
}

/// De stand van de groep rond een wortel (zie [`reductie::Zaakstand`]): de
/// cel filtert de grammen van de wortel en leidt af wat een proces erover
/// vraagt. Input `wortel`; met `eigenaar_pad` (een `$intake`-pad zonder
/// `$intake.`) en `eigenaar` ook of iemand met die waarde de groep kent. 404
/// als de cel de wortel niet kent.
fn zaakstand(
    state: &CelState,
    inputs: &Map<String, Value>,
    peil: &Peil,
) -> Result<Lexostatus, Error> {
    let tekst = |k: &str| {
        inputs
            .get(k)
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
    };
    let z =
        tekst(WORTEL).ok_or_else(|| error(StatusCode::BAD_REQUEST, "input 'wortel' ontbreekt"))?;
    let owner = match (tekst(reductie::EIGENAAR_PAD), tekst(reductie::EIGENAAR)) {
        (Some(p), Some(w)) => Some((p, w)),
        (None, None) => None,
        _ => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "vraag naar de eigenaar met eigenaar_pad en eigenaar samen",
            ))
        }
    };
    let grams = state
        .chronicle
        .lees_wortel(&state.cell.chronicles(), z)
        .map_err(intern)?;
    let cell = &state.cell;
    let bindt = |g: &Gram, path: &str| -> Vec<String> {
        let Some((_, event)) = cell.event(&g.stream.id, &g.name) else {
            return Vec::new();
        };
        event
            .bladeren()
            .into_iter()
            .filter(|b| b.binding == stroom::Binding::Intake(path.to_string()))
            .map(|b| b.path)
            .collect()
    };
    reductie::reduceer_zaak(grams.iter().map(|v| &v.gram), peil, owner, bindt)
        .map_err(intern)?
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                format!("geen wortel '{z}' in de kroniek"),
            )
        })?
        .als_lexostatus(z, peil)
        .map_err(intern)
}

/// De stroomdefinities van de cel, elk met de hash die in haar grammen en in
/// het receipt van een besluit staat.
async fn stroom_route(State(state): State<CelState>) -> Json<Value> {
    let streams: Vec<Value> = state
        .cell
        .streams
        .iter()
        .map(|s| json!({"id": s.id, "sha256": s.sha256, "stream": s.document}))
        .collect();
    Json(json!({"cell": state.cell.id(), "streams": streams}))
}

/// Wat `GET /api/cellen` over een cel zegt: wie ze is, welke kronieken ze
/// bijhoudt en welke lexostatussen ze aanbiedt.
pub fn cel_beschrijving(state: &CelState) -> Value {
    let cell = &state.cell;
    let mut lexostatuses: Vec<Value> = cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .map(|d| {
            let mut l = json!({
                "name": d.name,
                "inputs": d.inputs,
                "list": d.is_lijst(),
                "parameters": if d.is_lijst() { Vec::new() } else { d.reduction.derivations.keys().collect::<Vec<_>>() },
                "columns": if d.is_lijst() { d.reduction.derivations.keys().collect::<Vec<_>>() } else { Vec::new() },
                "extra_fields": d.reduction.extra_fields.keys().collect::<Vec<_>>(),
            });
            let r = reductie_van(cell, &d.name);
            if !r.is_null() {
                l["reduction"] = r;
            }
            l
        })
        .collect();
    if cell.heeft_zaken() {
        // De stand van een zaak biedt de runtime aan, niet de configuratie.
        lexostatuses.push(json!({
            "name": reductie::ZAAKSTAND,
            "inputs": [{"name": WORTEL, "type": "string"}],
            "list": false,
            "parameters": [],
            "columns": [],
            "extra_fields": ["grams", "events", "latest_effective_at", "stages", "owner"],
            "runtime": true,
        }));
        if cell.route.is_some() {
            if let Some(l) = lexostatuses.last_mut() {
                l["reduction"] = json!({"route": "runtime", "reason": ZAAKSTAND_REDEN});
            }
        }
    }
    let mut uit = json!({
        "id": cell.id(),
        "recording_actor": cell.definitie.recording_actor,
        "chronicles": cell.chronicles(),
        "lexostatuses": lexostatuses,
    });
    // Alleen met de engine-route: zonder blijft de beschrijving gelijk.
    if let Some(route) = &cell.route {
        uit["reduction"] = json!(if route.vergelijk { "compare" } else { "engine" });
    }
    uit
}

/// Langs welke route de cel een lexostatus reduceert, voor de beschrijving:
/// null zonder engine-route.
fn reductie_van(cell: &Cell, name: &str) -> Value {
    match cell.route.as_ref().and_then(|r| r.wijzen.get(name)) {
        None => Value::Null,
        Some(lexostatus_engine::Wijze::Engine {
            regulation,
            article,
        }) => {
            json!({"route": "engine", "regulation": article.as_ref().unwrap_or(regulation)})
        }
        Some(lexostatus_engine::Wijze::Dsl { reason }) => json!({"route": "dsl", "reason": reason}),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::{testgram, testvolger};

    const Z: &str = "00000000-0000-4000-8000-000000000001";
    const DAG: &str = "2025-03-12T10:00:00+01:00";

    /// Een cel met de fictieve afnemer: aanvraag, verloop, besluit,
    /// bekendmaking en betaling.
    fn cell() -> Cell {
        let map = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let service = Arc::new(
            crate::regelingen::laad(&map.join("regulation"))
                .unwrap()
                .service,
        );
        Cell::laad(&map.join("cells/afnemer"), service).unwrap()
    }

    fn als(name: &str, stream: &str, g: &mut Gram) {
        g.name = name.into();
        g.stream.id = stream.into();
    }

    fn application() -> Gram {
        let mut g = testgram(Z);
        als("aanvraag_ontvangen", "test_afnemer_aanvragen", &mut g);
        g.stage = Some("AANVRAAG".into());
        g.effective_at = "2025-03-01T10:00:00+01:00".into();
        g
    }

    fn volger(name: &str, verwijzing: &str, doel: &Gram, stage: Option<&str>) -> Gram {
        let mut g = testvolger(verwijzing, doel);
        als(name, "test_afnemer_zaakverloop", &mut g);
        g.stage = stage.map(str::to_string);
        g.effective_at = DAG.into();
        g
    }

    fn assessment(
        c: &Cell,
        g: &Gram,
        doelen: &[&Gram],
        group: &[&Gram],
        verwacht: Option<usize>,
    ) -> Result<(), (u16, String)> {
        let zicht = Zicht {
            doelen: doelen.iter().map(|d| (d.id.clone(), *d)).collect(),
            group: group.to_vec(),
            wortelfout: None,
        };
        toets_verwijzingen(c, g, &zicht, verwacht).map_err(|Error(s, t)| (s.as_u16(), t))
    }

    #[test]
    fn een_verwijzing_naar_een_onbekend_gram_weigert() {
        let c = cell();
        let a = application();
        let b = volger("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let (status, f) = assessment(&c, &b, &[], &[&a], None).unwrap_err();
        assert_eq!(status, 400);
        assert!(f.contains("geen gram"), "{f}");
    }

    /// Een betaling die naar de aanvraag verwijst in plaats van naar het
    /// besluit, weigert de cel: `besluit` wijst naar een gram met stage
    /// BESLUIT.
    #[test]
    fn een_verwijzing_naar_het_verkeerde_gram_weigert() {
        let c = cell();
        let a = application();
        let betaling = volger("betaling_verricht", "decision", &a, None);
        let (status, f) = assessment(&c, &betaling, &[&a], &[&a], None).unwrap_err();
        assert_eq!(status, 400);
        assert!(f.contains("stage BESLUIT"), "{f}");
        let b = volger("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let betaling = volger("betaling_verricht", "decision", &b, None);
        assessment(&c, &betaling, &[&b], &[&a, &b], None).unwrap();
    }

    /// De optimistische toets: legt het proces vast op een groep met meer
    /// (of minder) grammen dan het las, dan weigert de cel (409).
    #[test]
    fn een_groep_die_veranderde_sinds_het_lezen_weigert() {
        let c = cell();
        let a = application();
        let b = volger("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let (status, f) = assessment(&c, &b, &[&a], &[&a], Some(2)).unwrap_err();
        assert_eq!(status, 409);
        assert!(f.contains("veranderde sinds het proces haar las"), "{f}");
        assessment(&c, &b, &[&a], &[&a], Some(1)).unwrap();
    }

    #[test]
    fn een_gram_ligt_niet_voor_het_gram_waarnaar_het_verwijst() {
        let c = cell();
        let a = application();
        let mut b = volger("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        b.effective_at = "2025-02-28T10:00:00+01:00".into();
        let (status, f) = assessment(&c, &b, &[&a], &[&a], None).unwrap_err();
        assert_eq!(status, 409);
        assert!(
            f.contains("ligt voor het gram waarnaar het verwijst"),
            "{f}"
        );
    }

    /// Een tweede besluit van hetzelfde event op dezelfde aanvraag weigert de
    /// cel; een besluit wordt een keer bekendgemaakt, een ander besluit heeft
    /// zijn eigen bekendmaking.
    #[test]
    fn een_besluit_en_een_stage_een_keer_per_doel() {
        let c = cell();
        let a = application();
        let b1 = volger("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let nog_een = volger("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        let (status, f) = assessment(&c, &nog_een, &[&a], &[&a, &b1], None).unwrap_err();
        assert_eq!(status, 409);
        assert!(
            f.contains("er ligt al een besluit 'besluit_genomen'"),
            "{f}"
        );
        let bm1 = volger(
            "besluit_bekendgemaakt",
            "decision",
            &b1,
            Some("BEKENDMAKING"),
        );
        let bm2 = volger(
            "besluit_bekendgemaakt",
            "decision",
            &b1,
            Some("BEKENDMAKING"),
        );
        let (status, f) = assessment(&c, &bm2, &[&b1], &[&a, &b1, &bm1], None).unwrap_err();
        assert_eq!(status, 409);
        assert!(f.contains(&format!("bij gram {}", b1.id)), "{f}");
        let mut b2 = volger("besluit_genomen", "on_application", &a, Some("BESLUIT"));
        b2.id = uuid::Uuid::now_v7().to_string();
        let bm = volger(
            "besluit_bekendgemaakt",
            "decision",
            &b2,
            Some("BEKENDMAKING"),
        );
        assessment(&c, &bm, &[&b2], &[&a, &b1, &bm1, &b2], None).unwrap();
    }
}

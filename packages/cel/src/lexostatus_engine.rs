//! Experiment A: een lexostatus als gewone engine-run in plaats van de
//! reductie-DSL ([`crate::reductie`]).
//!
//! De lexostatus is dan een artikel (een engine-regeling) met de kroniek als
//! parameter `grammen`: een lijst grammen. Deze module doet alleen wat de
//! engine niet kan: elk gram een `volgorde` geven, zijn plaats in de tijd (de
//! engine kent geen tijdstip met tijdzone), zodat "het laatste gram" het gram
//! met de hoogste volgorde is, en de datum van zijn momenten (`op_datum`,
//! `vastgelegd_datum`). De rest staat in de regeling.
//!
//! De runtime kiest de route per deployment (`CEL_REDUCTIE`, zie
//! [`crate::config::Reductiemodus`]). Met `engine` noemt een koppelbestand
//! (`CEL_ENGINE_KOPPELING`) per cel en per lexostatus de regeling, of zegt
//! bewust dat een lexostatus langs de DSL gaat, met een reden:
//!
//! ```yaml
//! cellen:
//!   <cel-id>:
//!     <lexostatus>: <pad naar de regeling, relatief aan dit bestand>
//!     <lexostatus>: {dsl: <waarom niet via de engine>}
//! ```
//!
//! Een lexostatus zonder koppeling houdt de runtime tegen: geen stille
//! terugval (zie [`laad_koppeling`]). De regeling heeft een uitkomst per
//! afleiding en per extra veld, met dezelfde naam. Kiest de lexostatus zelf
//! een gram (`kies: laatste` naast `filter`), dan heeft de regeling ook de
//! uitkomst `laatste`: de volgorde van dat gram, of null (geen gram, dus geen
//! lexostatus). Een uitkomst null is een afleiding waarover de kroniek niets
//! zegt, tenzij de afleiding met `geen_gram: null` zegt dat null de waarde is.
//! Zie het verslag in de superpowers-specs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use regelrecht_engine::LawExecutionService;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::datum;
use crate::gram::Gram;
use crate::laden;
use crate::reductie::{self, Lexostatus, LexostatusDefinitie, Lexostatussen, Peil, Reductieroute};
use crate::toets;

/// De uitkomst met de volgorde van het gram dat de lexostatus kiest.
pub const LAATSTE: &str = "laatste";

/// De plaats in de tijd van elk gram, in de volgorde van `grammen`: zoals
/// `kies: laatste` die leest ([`Gram::tijdvolgorde`]: eerst `op_moment`, dan
/// `vastgelegd_op`, en bij gelijke tijden de volgorde van de kroniek).
fn in_de_tijd(grammen: &[&Gram]) -> Result<Vec<usize>, String> {
    for g in grammen {
        // Een ongeldig moment is een fout, geen plaats achteraan.
        g.moment()?;
        g.vastgelegd()?;
    }
    let mut volgorde: Vec<usize> = (0..grammen.len()).collect();
    // Stabiel: bij gelijke tijden blijft de volgorde van de kroniek.
    volgorde.sort_by(|&a, &b| {
        grammen[a]
            .tijdvolgorde(grammen[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut plaats = vec![0; grammen.len()];
    for (p, i) in volgorde.into_iter().enumerate() {
        plaats[i] = p;
    }
    Ok(plaats)
}

/// Een gram als element van de parameter `grammen`: `volgorde`, elk kenmerk
/// waarop een DSL-filter kan selecteren ([`reductie::GRAM_SLEUTELS`], null
/// als het gram het niet heeft), de
/// momenten en hun datum, en `fields`.
fn als_element(g: &Gram, volgorde: usize) -> Result<Value, String> {
    let mut o = Map::new();
    o.insert("volgorde".into(), json!(volgorde));
    // Een kenmerk dat het gram niet heeft, is null: een filter erop is dan
    // onwaar, zoals in de DSL, en geen ontbrekend feit.
    for sleutel in reductie::GRAM_SLEUTELS {
        o.insert((*sleutel).into(), json!(g.kenmerk(sleutel).flatten()));
    }
    o.insert("op_moment".into(), json!(g.op_moment));
    o.insert("op_datum".into(), json!(datum::peildatum(&g.moment()?)));
    o.insert("vastgelegd_op".into(), json!(g.vastgelegd_op));
    o.insert(
        "vastgelegd_datum".into(),
        json!(datum::peildatum(&g.vastgelegd()?)),
    );
    o.insert("fields".into(), Value::Object(g.fields.clone()));
    Ok(Value::Object(o))
}

/// De grammen als parameter voor de engine, in de volgorde van de kroniek
/// (zie [`als_element`]). Zo is "het laatste gram" het gram met de hoogste
/// volgorde, en blijft een verzameling (`verzamel`) in de volgorde van de
/// kroniek.
fn als_parameter(grammen: &[&Gram]) -> Result<Value, String> {
    let plaats = in_de_tijd(grammen)?;
    grammen
        .iter()
        .zip(plaats)
        .map(|(g, p)| als_element(g, p))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

/// De grammen van `kroniek` als parameter voor de engine (zie
/// [`als_parameter`]).
pub fn als_kroniek<'g>(
    grammen: impl IntoIterator<Item = &'g Gram>,
    kroniek: &str,
) -> Result<Value, String> {
    let door: Vec<&Gram> = grammen
        .into_iter()
        .filter(|g| g.chronicle == kroniek)
        .collect();
    als_parameter(&door)
}

/// Evalueer `uitkomsten` van `regeling` met de inputs en de grammen als
/// parameter `grammen`: de waarden (ook null) en, als gevraagd, de trace.
fn evalueer(
    service: &LawExecutionService,
    regeling: &str,
    uitkomsten: &[&str],
    inputs: &Map<String, Value>,
    grammen: Value,
    datum: &str,
    met_trace: bool,
) -> Result<(BTreeMap<String, Value>, Option<String>), String> {
    let mut parameters: BTreeMap<String, Value> =
        inputs.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    parameters.insert("grammen".into(), grammen);
    let e = if met_trace {
        toets::evalueer_met_trace(service, regeling, uitkomsten, &parameters, datum)
    } else {
        toets::evalueer(service, regeling, uitkomsten, &parameters, datum)
    };
    if let Some(f) = e.fout {
        return Err(f);
    }
    if !e.mist.is_empty() {
        return Err(format!("de engine mist {:?}", e.mist));
    }
    Ok((e.waarden, e.trace_text))
}

/// Reduceer via de engine: evalueer `uitkomsten` van `regeling` met de
/// inputs en de kroniek als parameter `grammen`. Een uitkomst null blijft
/// weg, zoals een parameter waarover de kroniek niets zegt in de reductie.
pub fn reduceer<'g>(
    service: &LawExecutionService,
    regeling: &str,
    uitkomsten: &[&str],
    inputs: &Map<String, Value>,
    grammen: impl IntoIterator<Item = &'g Gram>,
    kroniek: &str,
    datum: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let (waarden, _) = evalueer(
        service,
        regeling,
        uitkomsten,
        inputs,
        als_kroniek(grammen, kroniek)?,
        datum,
        false,
    )?;
    Ok(waarden.into_iter().filter(|(_, v)| !v.is_null()).collect())
}

/// De uitkomsten die de regeling van een lexostatus moet hebben: een per
/// afleiding en per extra veld, [`LAATSTE`] als de lexostatus zelf een gram
/// kiest, en `laatste_<naam>` bij een afleiding met `geen_gram` (zie
/// [`hulp_van`]).
pub fn uitkomsten_van(def: &LexostatusDefinitie) -> Vec<String> {
    let r = &def.reduction;
    let mut uit: Vec<String> = r
        .afleidingen
        .keys()
        .chain(r.extra_velden.keys())
        .cloned()
        .collect();
    if r.kies.is_some() {
        uit.push(LAATSTE.into());
    }
    for (naam, a) in r.afleidingen.iter().chain(&r.extra_velden) {
        if a.geen_gram().is_some() {
            uit.push(hulp_van(naam));
        }
    }
    uit
}

/// De hulpuitkomst van een afleiding met `geen_gram`: de volgorde van het
/// gram dat zij kiest, of null. Zo ziet de cel het verschil tussen "geen
/// gram" (dan `geen_gram`) en "een gram zonder dat veld" (dan niets), dat
/// een uitkomst null alleen niet draagt.
pub fn hulp_van(afleiding: &str) -> String {
    format!("{LAATSTE}_{afleiding}")
}

/// Hoe een cel een lexostatus reduceert in een runtime met de engine-route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wijze {
    /// Een engine-run van deze regeling. `artikel`: bij een lexostatus uit
    /// de wet het lezende artikel; de regeling is dan in het geheugen
    /// gemaakt uit zijn `leest` ([`crate::engine_regeling`]).
    Engine {
        regeling: String,
        artikel: Option<String>,
    },
    /// Bewust langs de reductie-DSL, met de reden uit het koppelbestand.
    Dsl { reden: String },
}

/// De engine-route van een cel: de regelingen van haar lexostatussen en per
/// lexostatus de [`Wijze`].
pub struct CelRoute {
    /// De lexostatus-regelingen van alle cellen, los van het corpus.
    pub service: Arc<LawExecutionService>,
    pub wijzen: BTreeMap<String, Wijze>,
    /// Ook langs de DSL reduceren en elk verschil een fout maken.
    pub vergelijk: bool,
}

/// Het koppelbestand, zoals het op schijf staat.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Koppelbestand {
    cellen: BTreeMap<String, BTreeMap<String, Koppeling>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Koppeling {
    Regeling(String),
    Dsl { dsl: String },
}

/// Lees het koppelbestand en controleer het tegen de cellen van de runtime
/// (id en lexostatussen): elke lexostatus van elke cel heeft een koppeling,
/// elke koppeling een lexostatus, elke regeling laadt en heeft de uitkomsten
/// van [`uitkomsten_van`], en een lijst-lexostatus (`groepeer`) gaat niet
/// via de engine. Een lexostatus uit de wet ([`crate::wet`]) heeft geen
/// koppeling nodig: haar regeling maakt de runtime uit het lezende artikel
/// ([`crate::engine_regeling`]); het koppelbestand kan haar alleen bewust op
/// `dsl` zetten. `corpus` geeft de namen van de regelingen voor de
/// `legal_basis` van zo'n regeling. Elke fout komt terug, niet alleen de
/// eerste.
pub fn laad_koppeling(
    pad: &Path,
    vergelijk: bool,
    cellen: &[(&str, &Lexostatussen)],
    corpus: &LawExecutionService,
) -> Result<BTreeMap<String, CelRoute>, Vec<String>> {
    let namen: BTreeMap<String, String> = corpus
        .resolver()
        .list_laws()
        .into_iter()
        .filter_map(|id| {
            let law = corpus.resolver().get_law(id)?;
            Some((
                id.to_string(),
                law.name.clone().unwrap_or_else(|| id.to_string()),
            ))
        })
        .collect();
    let bron = pad.display().to_string();
    let bestand: Koppelbestand = laden::laad(pad, laden::yaml)?;
    let map = pad.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut service = LawExecutionService::new();
    let mut geladen: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut fouten = Vec::new();
    let mut wijzen_per_cel: BTreeMap<String, BTreeMap<String, Wijze>> = BTreeMap::new();
    for id in bestand.cellen.keys() {
        if !cellen.iter().any(|(c, _)| c == id) {
            fouten.push(format!("{bron}: cel '{id}' draait niet in deze runtime"));
        }
    }
    for (id, lexostatussen) in cellen {
        let leeg = BTreeMap::new();
        let koppelingen = match bestand.cellen.get(*id) {
            Some(k) => k,
            // Alleen lexostatussen uit de wet: die hebben geen koppeling nodig.
            None if lexostatussen
                .lexostatus_definitions
                .iter()
                .all(|d| d.wet.is_some()) =>
            {
                &leeg
            }
            None => {
                fouten.push(format!(
                    "{bron}: cel '{id}' heeft geen koppeling; zet elke lexostatus op een regeling of op dsl"
                ));
                continue;
            }
        };
        for naam in koppelingen.keys() {
            if lexostatussen.lexostatus(naam).is_none() {
                fouten.push(format!("{bron}: cel '{id}' heeft geen lexostatus '{naam}'"));
            }
        }
        let mut wijzen = BTreeMap::new();
        for def in &lexostatussen.lexostatus_definitions {
            let waar = format!("{bron}: cel '{id}', lexostatus '{}'", def.name);
            let wijze = match koppelingen.get(&def.name) {
                None if def.wet.is_some() => {
                    let regeling = format!(
                        "lexostatus_{}_{}",
                        id,
                        def.name
                            .chars()
                            .map(|c| if c.is_ascii_alphanumeric() {
                                c.to_ascii_lowercase()
                            } else {
                                '_'
                            })
                            .collect::<String>()
                    );
                    let geladen = crate::engine_regeling::regeling(def, &regeling, &namen)
                        .and_then(|tekst| service.load_law(&tekst).map_err(|e| e.to_string()));
                    match geladen {
                        Ok(r) => Wijze::Engine {
                            regeling: r,
                            artikel: Some(def.name.clone()),
                        },
                        Err(e) => {
                            fouten.push(format!(
                                "{waar}: uit de wet, maar niet naar de engine te vertalen: {e}; zet haar bewust op dsl"
                            ));
                            continue;
                        }
                    }
                }
                Some(Koppeling::Regeling(_)) if def.wet.is_some() => {
                    fouten.push(format!(
                        "{waar}: komt uit de wet; haar engine-regeling maakt de runtime uit het artikel, dus geen bestand (alleen dsl met een reden kan)"
                    ));
                    continue;
                }
                None => {
                    fouten.push(format!(
                        "{waar}: geen koppeling (een regeling, of dsl met een reden)"
                    ));
                    continue;
                }
                Some(Koppeling::Dsl { dsl }) => Wijze::Dsl { reden: dsl.clone() },
                Some(Koppeling::Regeling(bestand)) => {
                    if def.is_lijst() {
                        fouten.push(format!(
                            "{waar}: een lijst-lexostatus (groepeer) kan niet via de engine; zet haar op dsl"
                        ));
                        continue;
                    }
                    let regelingpad = map.join(bestand);
                    let regeling = match geladen.get(&regelingpad) {
                        Some(r) => r.clone(),
                        None => {
                            let r = laden::lees(&regelingpad).and_then(|(tekst, b)| {
                                let id =
                                    service.load_law(&tekst).map_err(|e| format!("{b}: {e}"))?;
                                // Twee bestanden met dezelfde $id: de tweede
                                // zou de eerste stil vervangen.
                                match geladen.iter().find(|(_, i)| **i == id) {
                                    Some((ander, _)) => Err(format!(
                                        "{b}: regeling '{id}' staat ook in {}",
                                        ander.display()
                                    )),
                                    None => Ok(id),
                                }
                            });
                            match r {
                                Ok(r) => {
                                    geladen.insert(regelingpad.clone(), r.clone());
                                    r
                                }
                                Err(e) => {
                                    fouten.push(format!("{waar}: {e}"));
                                    continue;
                                }
                            }
                        }
                    };
                    let Some(info) = service.get_law_info(&regeling) else {
                        fouten.push(format!("{waar}: regeling '{regeling}' is niet te lezen"));
                        continue;
                    };
                    // De hulpuitkomsten mogen niet heten als een afleiding.
                    let r = &def.reduction;
                    for naam in r.afleidingen.keys().chain(r.extra_velden.keys()) {
                        if naam == LAATSTE || naam.starts_with(&format!("{LAATSTE}_")) {
                            fouten.push(format!(
                                "{waar}: afleiding '{naam}' botst met een hulpuitkomst van de engine-route ({LAATSTE}, {LAATSTE}_<naam>)"
                            ));
                        }
                    }
                    for u in uitkomsten_van(def) {
                        if !info.outputs.contains(&u) {
                            fouten.push(format!(
                                "{waar}: regeling '{regeling}' heeft geen uitkomst '{u}'"
                            ));
                        }
                    }
                    Wijze::Engine {
                        regeling,
                        artikel: None,
                    }
                }
            };
            wijzen.insert(def.name.clone(), wijze);
        }
        wijzen_per_cel.insert((*id).to_string(), wijzen);
    }
    if !fouten.is_empty() {
        return Err(fouten);
    }
    let service = Arc::new(service);
    Ok(wijzen_per_cel
        .into_iter()
        .map(|(id, wijzen)| {
            (
                id,
                CelRoute {
                    service: service.clone(),
                    wijzen,
                    vergelijk,
                },
            )
        })
        .collect())
}

/// Reduceer een lexostatus langs de route van de cel, op een peil, met de
/// route in de lexostatus ([`Lexostatus::reductie`]). `None`: de
/// lexostatus kiest een gram en er is er geen (zoals
/// [`reductie::reduceer_op`]). `datum` is de dag waarop de engine de
/// regeling leest; `met_trace` vraagt de trace van de engine-run.
pub fn reduceer_lexostatus<'g>(
    route: &CelRoute,
    def: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grammen: impl IntoIterator<Item = &'g Gram>,
    peil: &Peil,
    datum: &str,
    met_trace: bool,
) -> Result<Option<Lexostatus>, String> {
    let grammen: Vec<&Gram> = grammen.into_iter().collect();
    let t = Instant::now();
    let regeling = match route.wijzen.get(&def.name) {
        None => return Err(format!("lexostatus '{}' heeft geen koppeling", def.name)),
        Some(Wijze::Dsl { reden }) => {
            let l = reductie::reduceer_op(def, inputs, grammen.iter().copied(), peil)?;
            let duur_us = micro(t);
            return Ok(l.map(|l| Lexostatus {
                reductie: Some(Reductieroute {
                    route: "dsl".into(),
                    regeling: None,
                    reden: Some(reden.clone()),
                    duur_us,
                    dsl_duur_us: None,
                    trace_text: None,
                }),
                ..l
            }));
        }
        Some(Wijze::Engine { regeling, artikel }) => (regeling, artikel),
    };
    let (regeling, artikel) = regeling;
    let engine = via_engine(
        route, regeling, def, inputs, &grammen, peil, datum, met_trace,
    )?;
    let duur_us = micro(t);
    let dsl_duur_us = if route.vergelijk {
        let t = Instant::now();
        let dsl = reductie::reduceer_op(def, inputs, grammen.iter().copied(), peil)?;
        let d = micro(t);
        let zelfde = match (&engine, &dsl) {
            (Some((e, _)), Some(d)) => inhoud(e) == inhoud(d),
            (None, None) => true,
            _ => false,
        };
        if !zelfde {
            return Err(format!(
                "lexostatus '{}': de engine ({regeling}) en de DSL verschillen: engine {}, dsl {}",
                def.name,
                json!(engine.as_ref().map(|(e, _)| inhoud(e))),
                json!(dsl.as_ref().map(inhoud)),
            ));
        }
        Some(d)
    } else {
        None
    };
    Ok(engine.map(|(l, trace_text)| Lexostatus {
        reductie: Some(Reductieroute {
            route: "engine".into(),
            regeling: Some(artikel.clone().unwrap_or_else(|| regeling.clone())),
            reden: None,
            duur_us,
            dsl_duur_us,
            trace_text,
        }),
        ..l
    }))
}

fn micro(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// Wat een lexostatus zegt, zonder route: om engine en DSL te vergelijken.
fn inhoud(l: &Lexostatus) -> Value {
    json!({
        "zaakkenmerk": l.zaakkenmerk,
        "op_moment": l.op_moment,
        "vastgelegd_op": l.vastgelegd_op,
        "parameters": l.parameters,
        "extra_velden": l.extra_velden,
        "niet_afgeleid": l.niet_afgeleid,
    })
}

/// De engine-run van een lexostatus: de grammen van haar kroniek bij het
/// peil als parameter, en de uitkomsten terug in de vorm van de DSL.
#[allow(clippy::too_many_arguments)]
fn via_engine(
    route: &CelRoute,
    regeling: &str,
    def: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grammen: &[&Gram],
    peil: &Peil,
    datum: &str,
    met_trace: bool,
) -> Result<Option<(Lexostatus, Option<String>)>, String> {
    let r = &def.reduction;
    if def.is_lijst() {
        return Err(format!(
            "lexostatus '{}' is een lijst en kan niet via de engine",
            def.name
        ));
    }
    let mut door = Vec::new();
    for g in grammen {
        if g.chronicle == r.kroniek && peil.laat_door(g)? {
            door.push(*g);
        }
    }
    let namen = uitkomsten_van(def);
    let uitkomsten: Vec<&str> = namen.iter().map(String::as_str).collect();
    let (waarden, trace_text) = evalueer(
        &route.service,
        regeling,
        &uitkomsten,
        inputs,
        als_parameter(&door)?,
        datum,
        met_trace,
    )?;
    let gekozen = match (r.kies, waarden.get(LAATSTE)) {
        (None, _) => None,
        (Some(_), None | Some(Value::Null)) => return Ok(None),
        (Some(_), Some(v)) => {
            let v = v
                .as_u64()
                .ok_or_else(|| format!("uitkomst '{LAATSTE}' is geen volgorde: {v}"))?;
            let plaats = in_de_tijd(&door)?;
            let i = plaats
                .iter()
                .position(|p| u64::try_from(*p).ok() == Some(v))
                .ok_or_else(|| format!("uitkomst '{LAATSTE}' {v} is geen gram"))?;
            Some(door[i])
        }
    };
    let mut l = Lexostatus {
        zaakkenmerk: gekozen.and_then(|g| g.zaakkenmerk.clone()),
        op_moment: gekozen.map(|g| g.op_moment.clone()),
        vastgelegd_op: gekozen.map(|g| g.vastgelegd_op.clone()),
        peilmoment: peil.peilmoment.map(|t| t.to_string()),
        bekend_op: peil.bekend_op.map(|t| t.to_string()),
        ..Lexostatus::leeg(&def.name)
    };
    for (naam, a, extra) in r
        .afleidingen
        .iter()
        .map(|(n, a)| (n, a, false))
        .chain(r.extra_velden.iter().map(|(n, a)| (n, a, true)))
    {
        // Null is "de kroniek zegt er niets over", tenzij de afleiding zegt
        // hoe zij afwezigheid leest (`geen_gram`) en er geen gram is.
        let w = match waarden.get(naam) {
            Some(Value::Null) | None => a
                .geen_gram()
                .filter(|_| waarden.get(&hulp_van(naam)).is_none_or(Value::is_null)),
            Some(w) => Some(w),
        }
        .cloned();
        match (w, extra) {
            (Some(w), false) => {
                l.parameters.insert(naam.clone(), w);
            }
            (Some(w), true) => {
                l.extra_velden.insert(naam.clone(), w);
            }
            (None, false) => l.niet_afgeleid.push(naam.clone()),
            (None, true) => {}
        }
    }
    Ok(Some((l, trace_text)))
}

/// Een kroniek als databron voor een lexostatus-regeling: de input `grammen`
/// (`source: {}`) van die ene regeling krijgt de grammen van de kroniek. Zo
/// werkt `source` (RFC-022 §4.2) tussen lexostatussen zonder dat de afnemer
/// de grammen ziet; welke regeling bij welke cel-kroniek hoort, is
/// deploymentconfiguratie. In dit experiment een momentopname; in een
/// runtime zou de bron de kroniek live lezen.
pub struct KroniekBron {
    naam: String,
    regeling: String,
    grammen: regelrecht_engine::Value,
}

impl KroniekBron {
    pub fn new<'g>(
        regeling: &str,
        grammen: impl IntoIterator<Item = &'g Gram>,
        kroniek: &str,
    ) -> Result<Self, String> {
        Ok(Self {
            naam: format!("kroniek:{kroniek}"),
            regeling: regeling.to_string(),
            grammen: regelrecht_engine::Value::from(&als_kroniek(grammen, kroniek)?),
        })
    }
}

impl regelrecht_engine::DataSource for KroniekBron {
    fn name(&self) -> &str {
        &self.naam
    }
    fn priority(&self) -> i32 {
        10
    }
    fn source_type(&self) -> &str {
        "kroniek"
    }
    fn has_field(&self, field: &str) -> bool {
        field == "grammen"
    }
    fn get(
        &self,
        field: &str,
        _criteria: &BTreeMap<String, regelrecht_engine::Value>,
    ) -> Option<regelrecht_engine::Value> {
        (field == "grammen").then(|| self.grammen.clone())
    }
    fn fields(&self) -> Vec<&str> {
        vec!["grammen"]
    }
    fn law_scope(&self) -> Option<&str> {
        Some(&self.regeling)
    }
    fn key_fields(&self) -> Option<&[String]> {
        Some(&[])
    }
}

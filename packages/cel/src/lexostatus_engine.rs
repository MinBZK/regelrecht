//! Experiment A: a lexostatus as an ordinary engine run instead of the
//! reduction DSL ([`crate::reduction`]).
//!
//! The lexostatus is then an article (an engine regulation) with the chronicle
//! as parameter `grams`: a list of grams. This module does only what the
//! engine cannot: give each gram a `sequence`, its place in time (the
//! engine knows no point in time with a time zone), so that "the latest gram"
//! is the gram with the highest sequence, and the date of its moments
//! (`effective_date`, `recorded_date`). The rest is in the regulation.
//!
//! The runtime chooses the route per deployment (`CELL_REDUCTION`, see
//! [`crate::config::ReductionMode`]). With `engine` a binding file
//! (`CELL_ENGINE_BINDING`) names the regulation per cell and per lexostatus, or
//! says deliberately that a lexostatus goes along the DSL, with a reason:
//!
//! ```yaml
//! cells:
//!   <cell-id>:
//!     <lexostatus>: <path to the regulation, relative to this file>
//!     <lexostatus>: {dsl: <why not via the engine>}
//! ```
//!
//! A lexostatus without a binding stops the runtime: no silent
//! fallback (see [`load_binding`]). The regulation has an output per
//! derivation and per extra field, with the same name. If the lexostatus itself
//! picks a gram (`pick: latest` next to `filter`), the regulation also has the
//! output `latest`: the sequence of that gram, or null (no gram, so no
//! lexostatus). An output null is a derivation the chronicle says nothing
//! about, unless the derivation says with `no_gram: null` that null is the value.
//! See the report in the superpowers-specs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use regelrecht_engine::LawExecutionService;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::assessment;
use crate::date;
use crate::gram::Gram;
use crate::load;
use crate::reduction::{
    self, AsOf, Lexostatus, LexostatusDefinition, Lexostatuses, ReductionRoute,
};

/// The output with the sequence of the gram the lexostatus picks.
pub const LATEST: &str = "latest";

/// The place in time of each gram, in the order of `grams`: as
/// `pick: latest` reads it ([`Gram::time_order`]: first `effective_at`, then
/// `recorded_at`, and for equal times the order of the chronicle).
fn in_the_time(grams: &[&Gram]) -> Result<Vec<usize>, String> {
    for g in grams {
        // An invalid moment is an error, not a place at the back.
        g.moment()?;
        g.recorded()?;
    }
    let mut order: Vec<usize> = (0..grams.len()).collect();
    // Stable: for equal times the order of the chronicle stays.
    order.sort_by(|&a, &b| {
        grams[a]
            .time_order(grams[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut place = vec![0; grams.len()];
    for (p, i) in order.into_iter().enumerate() {
        place[i] = p;
    }
    Ok(place)
}

/// A gram as an element of the parameter `grams`: `sequence`, every attribute
/// a DSL filter can select on ([`reduction::GRAM_KEYS`], null
/// if the gram does not have it), the
/// moments and their date, and `fields`.
fn as_element(g: &Gram, order: usize) -> Result<Value, String> {
    let mut o = Map::new();
    o.insert("sequence".into(), json!(order));
    // An attribute the gram does not have is null: a filter on it is then
    // false, as in the DSL, and not a missing fact.
    for key in reduction::GRAM_KEYS {
        o.insert((*key).into(), json!(g.attribute(key).flatten()));
    }
    // The references, per name: a filter `refers_to.<name>` reads them.
    o.insert("refers_to".into(), json!(g.refers_to));
    o.insert("effective_at".into(), json!(g.effective_at));
    o.insert(
        "effective_date".into(),
        json!(date::reference_date(&g.moment()?)),
    );
    o.insert("recorded_at".into(), json!(g.recorded_at));
    o.insert(
        "recorded_date".into(),
        json!(date::reference_date(&g.recorded()?)),
    );
    o.insert("fields".into(), Value::Object(g.fields.clone()));
    Ok(Value::Object(o))
}

/// The grams as a parameter for the engine, in the order of the chronicle
/// (see [`as_element`]). This way "the latest gram" is the gram with the highest
/// sequence, and a collection (`collect`) stays in the order of the
/// chronicle.
fn as_parameter(grams: &[&Gram]) -> Result<Value, String> {
    let place = in_the_time(grams)?;
    grams
        .iter()
        .zip(place)
        .map(|(g, p)| as_element(g, p))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

/// The grams of `chronicle` as a parameter for the engine (see
/// [`as_parameter`]).
pub fn as_chronicle<'g>(
    grams: impl IntoIterator<Item = &'g Gram>,
    chronicle: &str,
) -> Result<Value, String> {
    let through: Vec<&Gram> = grams
        .into_iter()
        .filter(|g| g.chronicle == chronicle)
        .collect();
    as_parameter(&through)
}

/// Evaluate `outputs` of `regulation` with the inputs and the grams as
/// parameter `grams`: the values (null too) and, if asked, the trace.
fn evaluate(
    service: &LawExecutionService,
    regulation: &str,
    outputs: &[&str],
    inputs: &Map<String, Value>,
    grams: Value,
    date: &str,
    with_trace: bool,
) -> Result<(BTreeMap<String, Value>, Option<String>), String> {
    let mut parameters: BTreeMap<String, Value> =
        inputs.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    parameters.insert("grams".into(), grams);
    let e = if with_trace {
        assessment::evaluate_with_trace(service, regulation, outputs, &parameters, date)
    } else {
        assessment::evaluate(service, regulation, outputs, &parameters, date)
    };
    if let Some(f) = e.error {
        return Err(f);
    }
    if !e.missing.is_empty() {
        return Err(format!("the engine is missing {:?}", e.missing));
    }
    Ok((e.values, e.trace_text))
}

/// Reduce via the engine: evaluate `outputs` of `regulation` with the
/// inputs and the chronicle as parameter `grams`. An output null is left
/// out, like a parameter the chronicle says nothing about in the reduction.
pub fn reduce<'g>(
    service: &LawExecutionService,
    regulation: &str,
    outputs: &[&str],
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
    chronicle: &str,
    date: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let (values, _) = evaluate(
        service,
        regulation,
        outputs,
        inputs,
        as_chronicle(grams, chronicle)?,
        date,
        false,
    )?;
    Ok(values.into_iter().filter(|(_, v)| !v.is_null()).collect())
}

/// The outputs the regulation of a lexostatus must have: one per
/// derivation and per extra field, [`LATEST`] if the lexostatus itself picks
/// a gram, and `latest_<name>` for a derivation with `no_gram` (see
/// [`helper_of`]).
pub fn outputs_of(def: &LexostatusDefinition) -> Vec<String> {
    let r = &def.reduction;
    let mut out: Vec<String> = r
        .derivations
        .keys()
        .chain(r.extra_fields.keys())
        .cloned()
        .collect();
    if r.pick.is_some() {
        out.push(LATEST.into());
    }
    for (name, a) in r.derivations.iter().chain(&r.extra_fields) {
        if a.no_gram().is_some() {
            out.push(helper_of(name));
        }
    }
    out
}

/// The helper output of a derivation with `no_gram`: the sequence of the
/// gram it picks, or null. This way the cell sees the difference between "no
/// gram" (then `no_gram`) and "a gram without that field" (then nothing), which
/// an output null alone does not carry.
pub fn helper_of(derivation: &str) -> String {
    format!("{LATEST}_{derivation}")
}

/// How a cell reduces a lexostatus in a runtime with the engine route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// An engine run of this regulation. `article`: for a lexostatus from
    /// the law the reading article; the regulation is then made in memory
    /// from its `reads` ([`crate::engine_regulation`]).
    Engine {
        regulation: String,
        article: Option<String>,
    },
    /// Deliberately along the reduction DSL, with the reason from the binding file.
    Dsl { reason: String },
}

/// The engine route of a cell: the regulations of its lexostatuses and per
/// lexostatus the [`Mode`].
pub struct CellRoute {
    /// The lexostatus regulations of all cells, separate from the corpus.
    pub service: Arc<LawExecutionService>,
    pub modes: BTreeMap<String, Mode>,
    /// Also reduce along the DSL and make every difference an error.
    pub compare: bool,
}

/// The binding file, as it is on disk.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingFile {
    cells: BTreeMap<String, BTreeMap<String, Binding>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Binding {
    Regulation(String),
    Dsl { dsl: String },
}

/// Read the binding file and check it against the cells of the runtime
/// (id and lexostatuses): every lexostatus of every cell has a binding,
/// every binding a lexostatus, every regulation loads and has the outputs
/// of [`outputs_of`], and a list lexostatus (`group_by`) does not go
/// via the engine. A lexostatus from the law ([`crate::law`]) needs no
/// binding: the runtime makes its regulation from the reading article
/// ([`crate::engine_regulation`]); the binding file can only deliberately set it to
/// `dsl`. `corpus` gives the names of the regulations for the
/// `legal_basis` of such a regulation. Every error is returned, not only the
/// first.
pub fn load_binding(
    path: &Path,
    compare: bool,
    cells: &[(&str, &Lexostatuses)],
    corpus: &LawExecutionService,
) -> Result<BTreeMap<String, CellRoute>, Vec<String>> {
    let names: BTreeMap<String, String> = corpus
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
    let source = path.display().to_string();
    let file: BindingFile = load::load(path, load::yaml)?;
    let map = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut service = LawExecutionService::new();
    let mut loaded: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut modes_per_cell: BTreeMap<String, BTreeMap<String, Mode>> = BTreeMap::new();
    for id in file.cells.keys() {
        if !cells.iter().any(|(c, _)| c == id) {
            errors.push(format!(
                "{source}: cell '{id}' does not run in this runtime"
            ));
        }
    }
    for (id, lexostatuses) in cells {
        let empty = BTreeMap::new();
        let bindings = match file.cells.get(*id) {
            Some(k) => k,
            // Only lexostatuses from the law: those need no binding.
            None if lexostatuses
                .lexostatus_definitions
                .iter()
                .all(|d| d.law.is_some()) =>
            {
                &empty
            }
            None => {
                errors.push(format!(
                    "{source}: cell '{id}' has no binding; set every lexostatus to a regulation or to dsl"
                ));
                continue;
            }
        };
        for name in bindings.keys() {
            if lexostatuses.lexostatus(name).is_none() {
                errors.push(format!("{source}: cell '{id}' has no lexostatus '{name}'"));
            }
        }
        let mut modes = BTreeMap::new();
        for def in &lexostatuses.lexostatus_definitions {
            let where_ = format!("{source}: cell '{id}', lexostatus '{}'", def.name);
            let mode = match bindings.get(&def.name) {
                None if def.law.is_some() => {
                    let regulation = format!(
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
                    let loaded = crate::engine_regulation::regulation(def, &regulation, &names)
                        .and_then(|text| service.load_law(&text).map_err(|e| e.to_string()));
                    match loaded {
                        Ok(r) => Mode::Engine {
                            regulation: r,
                            article: Some(def.name.clone()),
                        },
                        Err(e) => {
                            errors.push(format!(
                                "{where_}: from the law, but cannot be translated to the engine: {e}; set it deliberately to dsl"
                            ));
                            continue;
                        }
                    }
                }
                Some(Binding::Regulation(_)) if def.law.is_some() => {
                    errors.push(format!(
                        "{where_}: comes from the law; the runtime makes its engine regulation from the article, so no file (only dsl with a reason is possible)"
                    ));
                    continue;
                }
                None => {
                    errors.push(format!(
                        "{where_}: no binding (a regulation, or dsl with a reason)"
                    ));
                    continue;
                }
                Some(Binding::Dsl { dsl }) => Mode::Dsl {
                    reason: dsl.clone(),
                },
                Some(Binding::Regulation(file)) => {
                    if def.is_list() {
                        errors.push(format!(
                            "{where_}: a list lexostatus (group_by) cannot go via the engine; set it to dsl"
                        ));
                        continue;
                    }
                    let regulation_path = map.join(file);
                    let regulation = match loaded.get(&regulation_path) {
                        Some(r) => r.clone(),
                        None => {
                            let r = load::read(&regulation_path).and_then(|(text, b)| {
                                let id =
                                    service.load_law(&text).map_err(|e| format!("{b}: {e}"))?;
                                // Two files with the same $id: the second
                                // would silently replace the first.
                                match loaded.iter().find(|(_, i)| **i == id) {
                                    Some((other, _)) => Err(format!(
                                        "{b}: regulation '{id}' is also in {}",
                                        other.display()
                                    )),
                                    None => Ok(id),
                                }
                            });
                            match r {
                                Ok(r) => {
                                    loaded.insert(regulation_path.clone(), r.clone());
                                    r
                                }
                                Err(e) => {
                                    errors.push(format!("{where_}: {e}"));
                                    continue;
                                }
                            }
                        }
                    };
                    let Some(info) = service.get_law_info(&regulation) else {
                        errors.push(format!(
                            "{where_}: regulation '{regulation}' cannot be read"
                        ));
                        continue;
                    };
                    // The helper outputs may not have the name of a derivation.
                    let r = &def.reduction;
                    for name in r.derivations.keys().chain(r.extra_fields.keys()) {
                        if name == LATEST || name.starts_with(&format!("{LATEST}_")) {
                            errors.push(format!(
                                "{where_}: derivation '{name}' clashes with a helper output of the engine route ({LATEST}, {LATEST}_<name>)"
                            ));
                        }
                    }
                    for u in outputs_of(def) {
                        if !info.outputs.contains(&u) {
                            errors.push(format!(
                                "{where_}: regulation '{regulation}' has no output '{u}'"
                            ));
                        }
                    }
                    Mode::Engine {
                        regulation,
                        article: None,
                    }
                }
            };
            modes.insert(def.name.clone(), mode);
        }
        modes_per_cell.insert((*id).to_string(), modes);
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let service = Arc::new(service);
    Ok(modes_per_cell
        .into_iter()
        .map(|(id, modes)| {
            (
                id,
                CellRoute {
                    service: service.clone(),
                    modes,
                    compare,
                },
            )
        })
        .collect())
}

/// Reduce a lexostatus along the route of the cell, at an as-of moment, with the
/// route in the lexostatus ([`Lexostatus::reduction`]). `None`: the
/// lexostatus picks a gram and there is none (like
/// [`reduction::reduce_at`]). `date` is the day on which the engine reads the
/// regulation; `with_trace` asks for the trace of the engine run.
pub fn reduce_lexostatus<'g>(
    route: &CellRoute,
    def: &LexostatusDefinition,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
    as_of: &AsOf,
    date: &str,
    with_trace: bool,
) -> Result<Option<Lexostatus>, String> {
    let grams: Vec<&Gram> = grams.into_iter().collect();
    let t = Instant::now();
    let regulation = match route.modes.get(&def.name) {
        None => return Err(format!("lexostatus '{}' has no binding", def.name)),
        Some(Mode::Dsl { reason }) => {
            let l = reduction::reduce_at(def, inputs, grams.iter().copied(), as_of)?;
            let duration_us = micro(t);
            return Ok(l.map(|l| Lexostatus {
                reduction: Some(ReductionRoute {
                    route: "dsl".into(),
                    regulation: None,
                    reason: Some(reason.clone()),
                    duration_us,
                    dsl_duration_us: None,
                    trace_text: None,
                }),
                ..l
            }));
        }
        Some(Mode::Engine {
            regulation,
            article,
        }) => (regulation, article),
    };
    let (regulation, article) = regulation;
    let engine = via_engine(
        route, regulation, def, inputs, &grams, as_of, date, with_trace,
    )?;
    let duration_us = micro(t);
    let dsl_duration_us = if route.compare {
        let t = Instant::now();
        let dsl = reduction::reduce_at(def, inputs, grams.iter().copied(), as_of)?;
        let d = micro(t);
        let same = match (&engine, &dsl) {
            (Some((e, _)), Some(d)) => content(e) == content(d),
            (None, None) => true,
            _ => false,
        };
        if !same {
            return Err(format!(
                "lexostatus '{}': the engine ({regulation}) and the DSL differ: engine {}, dsl {}",
                def.name,
                json!(engine.as_ref().map(|(e, _)| content(e))),
                json!(dsl.as_ref().map(content)),
            ));
        }
        Some(d)
    } else {
        None
    };
    Ok(engine.map(|(l, trace_text)| Lexostatus {
        reduction: Some(ReductionRoute {
            route: "engine".into(),
            regulation: Some(article.clone().unwrap_or_else(|| regulation.clone())),
            reason: None,
            duration_us,
            dsl_duration_us,
            trace_text,
        }),
        ..l
    }))
}

fn micro(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// What a lexostatus says, without route: to compare engine and DSL.
fn content(l: &Lexostatus) -> Value {
    json!({
        "root": l.root,
        "effective_at": l.effective_at,
        "recorded_at": l.recorded_at,
        "parameters": l.parameters,
        "extra_fields": l.extra_fields,
        "not_derived": l.not_derived,
    })
}

/// The engine run of a lexostatus: the grams of its chronicle at the
/// as-of moment as a parameter, and the outputs back in the shape of the DSL.
#[allow(clippy::too_many_arguments)]
fn via_engine(
    route: &CellRoute,
    regulation: &str,
    def: &LexostatusDefinition,
    inputs: &Map<String, Value>,
    grams: &[&Gram],
    as_of: &AsOf,
    date: &str,
    with_trace: bool,
) -> Result<Option<(Lexostatus, Option<String>)>, String> {
    let r = &def.reduction;
    if def.is_list() {
        return Err(format!(
            "lexostatus '{}' is a list and cannot go via the engine",
            def.name
        ));
    }
    let mut through = Vec::new();
    for g in grams {
        if g.chronicle == r.chronicle && as_of.let_through(g)? {
            through.push(*g);
        }
    }
    let names = outputs_of(def);
    let outputs: Vec<&str> = names.iter().map(String::as_str).collect();
    let (values, trace_text) = evaluate(
        &route.service,
        regulation,
        &outputs,
        inputs,
        as_parameter(&through)?,
        date,
        with_trace,
    )?;
    let chosen = match (r.pick, values.get(LATEST)) {
        (None, _) => None,
        (Some(_), None | Some(Value::Null)) => return Ok(None),
        (Some(_), Some(v)) => {
            let v = v
                .as_u64()
                .ok_or_else(|| format!("output '{LATEST}' is not a sequence: {v}"))?;
            let place = in_the_time(&through)?;
            let i = place
                .iter()
                .position(|p| u64::try_from(*p).ok() == Some(v))
                .ok_or_else(|| format!("output '{LATEST}' {v} is not a gram"))?;
            Some(through[i])
        }
    };
    let mut l = Lexostatus {
        root: chosen.and_then(|g| g.root.clone()),
        effective_at: chosen.map(|g| g.effective_at.clone()),
        recorded_at: chosen.map(|g| g.recorded_at.clone()),
        as_of: as_of.as_of.map(|t| t.to_string()),
        known_at: as_of.known_at.map(|t| t.to_string()),
        ..Lexostatus::empty(&def.name)
    };
    for (name, a, extra) in r
        .derivations
        .iter()
        .map(|(n, a)| (n, a, false))
        .chain(r.extra_fields.iter().map(|(n, a)| (n, a, true)))
    {
        // Null is "the chronicle says nothing about it", unless the derivation
        // says how it reads absence (`no_gram`) and there is no gram.
        let w = match values.get(name) {
            Some(Value::Null) | None => a
                .no_gram()
                .filter(|_| values.get(&helper_of(name)).is_none_or(Value::is_null)),
            Some(w) => Some(w),
        }
        .cloned();
        match (w, extra) {
            (Some(w), false) => {
                l.parameters.insert(name.clone(), w);
            }
            (Some(w), true) => {
                l.extra_fields.insert(name.clone(), w);
            }
            (None, false) => l.not_derived.push(name.clone()),
            (None, true) => {}
        }
    }
    Ok(Some((l, trace_text)))
}

/// A chronicle as a data source for a lexostatus regulation: the input `grams`
/// (`source: {}`) of that one regulation gets the grams of the chronicle. This
/// way `source` (RFC-022 §4.2) works between lexostatuses without the consumer
/// seeing the grams; which regulation belongs to which cell chronicle is
/// deployment configuration. In this experiment a snapshot; in a
/// runtime the source would read the chronicle live.
pub struct ChronicleSource {
    name: String,
    regulation: String,
    grams: regelrecht_engine::Value,
}

impl ChronicleSource {
    pub fn new<'g>(
        regulation: &str,
        grams: impl IntoIterator<Item = &'g Gram>,
        chronicle: &str,
    ) -> Result<Self, String> {
        Ok(Self {
            name: format!("chronicle:{chronicle}"),
            regulation: regulation.to_string(),
            grams: regelrecht_engine::Value::from(&as_chronicle(grams, chronicle)?),
        })
    }
}

impl regelrecht_engine::DataSource for ChronicleSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn priority(&self) -> i32 {
        10
    }
    fn source_type(&self) -> &str {
        "chronicle"
    }
    fn has_field(&self, field: &str) -> bool {
        field == "grams"
    }
    fn get(
        &self,
        field: &str,
        _criteria: &BTreeMap<String, regelrecht_engine::Value>,
    ) -> Option<regelrecht_engine::Value> {
        (field == "grams").then(|| self.grams.clone())
    }
    fn fields(&self) -> Vec<&str> {
        vec!["grams"]
    }
    fn law_scope(&self) -> Option<&str> {
        Some(&self.regulation)
    }
    fn key_fields(&self) -> Option<&[String]> {
        Some(&[])
    }
}

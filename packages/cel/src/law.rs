//! The law as the source of the gram shape and the reduction (proposal
//! "lexostatus in de wet", 28-09-2026).
//!
//! An article can say two things in `produces.extensions.chronolex`
//! (RFC-022 §3.2: one namespace per integration):
//!
//! - `establishes`: the facts this article brings into being, as an event of a
//!   chronicle: the type, the subtype, the stage, which gram it refers to
//!   (`refers_to`, with a name from the law text: a decision `on_application`,
//!   a payment to the `decision`), the legal basis, why the `effective_at`
//!   counts in law and which fields the gram carries. Or it extends an event
//!   that another article establishes (`extends`), adding fields and legal
//!   basis: this is how the Awb establishes the application and a specific
//!   law adds its content.
//! - `reads`: how the article reads its own parameters from the chronicle, in
//!   the vocabulary of the reduction ([`crate::reduction`]). That is a
//!   lexostatus "from the requested perspective" (position paper): the
//!   perspective is the article that asks. It is named after the article
//!   (`<regulation>#<article>`).
//!
//! A stream names per event which articles establish it (`establishes:`) and
//! holds only the registration: intake, the binding of each field, the source
//! of `effective_at`. [`establish`] fills in the rest from the law and checks
//! the fields against the law. [`lexostatuses`] turns every reading article
//! whose facts lie in the cell into a lexostatus definition, which then goes
//! through the same checks and the same reduction as a definition from
//! `cell.yaml`.

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_engine::{Article, LawExecutionService};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::reduction::{
    Derived, Filter, InputDefinition, LawSupplement, LexostatusDefinition, Pick, Reduction,
};
use crate::schema::{self, Kind};
use crate::stream::{Reference, Stream};

/// The namespace in `produces.extensions` (RFC-022 §3.2).
pub const NAMESPACE: &str = "chronolex";

/// The `produces.extensions.chronolex` block of an article.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chronolex {
    #[serde(default)]
    pub establishes: Vec<Establishment>,
    #[serde(default)]
    pub reads: Option<Reads>,
}

/// One reading, or more: an article can read differently per paragraph
/// (paragraph 2 reads the default verdict, paragraphs 3 to 5 the
/// application). Each reading becomes a lexostatus of its own.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Reads {
    One(Reading),
    Many(Vec<Reading>),
}

impl Reads {
    pub fn readings(&self) -> Vec<&Reading> {
        match self {
            Reads::One(l) => vec![l],
            Reads::Many(v) => v.iter().collect(),
        }
    }
}

/// A fact that an article establishes, or the extension of a fact that
/// another article establishes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Establishment {
    /// The event this article establishes (the name in the chronicle).
    #[serde(default)]
    pub event: Option<String>,
    /// The event that another article establishes and that this article extends.
    #[serde(default)]
    pub extends: Option<String>,
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    /// Which gram a gram of this event refers to, per name from the law text
    /// (Wpp 107 "besluit op de aanvraag": `on_application`; Awb 3:41
    /// "bekendmaking van besluiten": `decision`), and what that gram must be.
    #[serde(default)]
    pub refers_to: BTreeMap<String, Reference>,
    /// The legal basis the gram carries. Without one: the article itself for
    /// an event, nothing for an extension.
    #[serde(default)]
    pub legal_basis: Option<Vec<String>>,
    #[serde(default)]
    pub effective_at: Option<EffectiveAtLaw>,
    #[serde(default)]
    pub fields: Option<Fields>,
    /// Name bridge: under which name another article reads a field of this
    /// gram (`<name at the reader>: <field of the gram>`). This is how Awb 4:52
    /// reads "het vastgestelde bedrag" without knowing the name of the output
    /// of the specific law.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

impl Establishment {
    /// The references as the stream would name them, for the document of the
    /// stream (`GET /api/stream`); `None` without a reference.
    fn refers_to_as_json(&self) -> Option<Value> {
        if self.refers_to.is_empty() {
            return None;
        }
        let mut out = Map::new();
        for (name, v) in &self.refers_to {
            let to = match &v.to {
                crate::stream::To::Article(a) | crate::stream::To::Event(a) => {
                    Value::String(a.clone())
                }
                crate::stream::To::Stage(s) => serde_json::json!({"stage": s}),
            };
            out.insert(
                name.clone(),
                serde_json::json!({"to": to, "required": v.required}),
            );
        }
        Some(Value::Object(out))
    }
}

/// Why the `effective_at` counts in law, and which piece of data it is. The
/// source (who provides it, `$external` or `$intake`) is in the stream: that
/// is registration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveAtLaw {
    /// The parameter of this article that is the moment (such as the decision date).
    #[serde(default)]
    pub parameter: Option<String>,
    /// The field of the gram that is the moment (such as the day of payment).
    #[serde(default)]
    pub field: Option<String>,
    pub legal_basis: Vec<String>,
}

/// The fields of a gram: a list of field paths, or a keyword.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Fields {
    /// `outputs`: the outputs of this article (a decision carries what the
    /// decision outputs). `stage`: what the stage of the procedure requires
    /// (`requires`, RFC-008) and the outputs of the hooks on that stage
    /// (RFC-007), such as the objection period of Awb 6:8 on publication.
    Keyword(String),
    /// Field paths under `fields` (`content.subsidiejaar`); a path covers
    /// everything below it.
    List(Vec<String>),
    /// Field paths with their type, as Awb 4:87 names the amount of a
    /// payment: `{bedrag: {type: amount, unit: eurocent}}`.
    Typed(BTreeMap<String, FieldType>),
}

/// The type of a field of a gram, as the establishing article names it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldType {
    #[serde(rename = "type")]
    pub type_: regelrecht_law_model::ParameterType,
    #[serde(default)]
    pub unit: Option<String>,
}

/// How an article reads its parameters from the chronicle.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
    /// The paragraph that carries this reading, if an article has more
    /// readings; the lexostatus is then named `<regulation>#<article> lid <n>`.
    #[serde(default)]
    pub paragraph: Option<Value>,
    /// Which gram the article reads, if it picks one (with `pick`).
    #[serde(default)]
    pub from: Option<Map<String, Value>>,
    #[serde(default)]
    pub pick: Option<Pick>,
    /// `this`: the article reads the grams of the decision it is asked for.
    /// The runtime gives a source of the group only the root; it therefore
    /// reads per root (a root with one decision gives the same).
    #[serde(default)]
    pub decision: Option<String>,
    /// Per parameter of this article the derivation, optionally with `from`
    /// (which grams) and `legal_basis` (without: this article).
    pub parameters: BTreeMap<String, Value>,
}

/// A lexostatus from the law: which article reads it, and the types of the
/// parameters as that article declares them (for the engine route).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LawReading {
    pub article: String,
    pub types: BTreeMap<String, String>,
    /// The article reads per decision (`decision: this`).
    pub decision_this: bool,
}

/// An article with a chronolex block.
pub struct LawArticle<'s> {
    /// `<regulation>#<article>`.
    pub reference: String,
    pub article: &'s Article,
    pub chronolex: Chronolex,
}

/// The chronolex block of an article, if it has one.
fn block(article: &Article) -> Option<&Value> {
    article
        .get_execution_spec()?
        .produces
        .as_ref()?
        .extensions
        .as_ref()?
        .get(NAMESPACE)
}

/// Every article in the corpus with a chronolex block, by reference. A block
/// that cannot be read is an error naming the article.
pub fn articles(
    service: &LawExecutionService,
) -> Result<BTreeMap<String, LawArticle<'_>>, Vec<String>> {
    let resolver = service.resolver();
    let mut out = BTreeMap::new();
    let mut errors = Vec::new();
    for id in resolver.list_laws() {
        let Some(law) = resolver.get_law(id) else {
            continue;
        };
        for article in &law.articles {
            let Some(b) = block(article) else { continue };
            let reference = format!("{id}#{}", article.number);
            match serde_json::from_value::<Chronolex>(b.clone()) {
                Ok(chronolex) => {
                    out.insert(
                        reference.clone(),
                        LawArticle {
                            reference,
                            article,
                            chronolex,
                        },
                    );
                }
                Err(e) => errors.push(format!(
                    "{reference}: produces.extensions.{NAMESPACE} cannot be read: {e}"
                )),
            }
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// The outputs of an article.
fn outputs(article: &Article) -> Vec<String> {
    article
        .get_execution_spec()
        .and_then(|e| e.output.as_ref())
        .into_iter()
        .flatten()
        .map(|o| o.name.clone())
        .collect()
}

/// The parameters of an article with their type (`date`, `number`, ...).
fn parameter_types(article: &Article) -> BTreeMap<String, String> {
    article
        .get_execution_spec()
        .and_then(|e| e.parameters.as_ref())
        .into_iter()
        .flatten()
        .map(|p| {
            let t = serde_json::to_value(p.param_type)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            (p.name.clone(), t)
        })
        .collect()
}

/// What a stage requires: `requires` of that stage in a procedure (RFC-008)
/// and the outputs of the articles with a hook on that stage (RFC-007).
fn stage_fields(service: &LawExecutionService, stage: &str) -> Vec<String> {
    let resolver = service.resolver();
    let mut out: Vec<String> = Vec::new();
    let mut add = |n: String| {
        if !out.contains(&n) {
            out.push(n);
        }
    };
    for id in resolver.list_laws() {
        let Some(law) = resolver.get_law(id) else {
            continue;
        };
        for p in law.procedure.iter().flatten() {
            for s in p.stages.iter().filter(|s| s.name == stage) {
                for r in s.requires.iter().flatten() {
                    add(r.name.clone());
                }
            }
        }
    }
    for id in resolver.list_laws() {
        let Some(law) = resolver.get_law(id) else {
            continue;
        };
        for article in &law.articles {
            let hooks = article
                .machine_readable
                .as_ref()
                .and_then(|m| m.hooks.as_ref())
                .is_some_and(|h| {
                    h.iter()
                        .any(|h| h.applies_to.stage.as_deref() == Some(stage))
                });
            if hooks {
                for o in outputs(article) {
                    add(o);
                }
            }
        }
    }
    out
}

/// The field paths an establishment declares.
fn field_paths(
    service: &LawExecutionService,
    wa: &LawArticle<'_>,
    v: &Establishment,
    stage: Option<&str>,
) -> Result<Vec<String>, String> {
    Ok(match &v.fields {
        None => Vec::new(),
        Some(Fields::List(l)) => l.clone(),
        Some(Fields::Typed(m)) => m.keys().cloned().collect(),
        Some(Fields::Keyword(t)) if t == "outputs" => outputs(wa.article),
        Some(Fields::Keyword(t)) if t == "stage" => match stage {
            Some(s) => stage_fields(service, s),
            None => {
                return Err(format!(
                    "{}: fields: stage, but the event has no stage",
                    wa.reference
                ))
            }
        },
        Some(Fields::Keyword(t)) => {
            return Err(format!(
                "{}: fields '{t}' is not a list and not a keyword (outputs, stage)",
                wa.reference
            ))
        }
    })
}

/// Fill in the events with `establishes` from the law, and check them. An
/// error names the stream, the event and the article.
pub fn establish(streams: &mut [Stream], service: &LawExecutionService) -> Vec<String> {
    if !streams
        .iter()
        .any(|s| s.events.iter().any(|e| !e.establishes.is_empty()))
    {
        return Vec::new();
    }
    let law = match articles(service) {
        Ok(w) => w,
        Err(f) => return f,
    };
    let mut errors = Vec::new();
    for stream in streams.iter_mut() {
        for i in 0..stream.events.len() {
            if stream.events[i].establishes.is_empty() {
                continue;
            }
            let where_ = format!("stream '{}', event '{}'", stream.id, stream.events[i].name);
            match establish_event(stream, i, &law, service) {
                Ok(()) => {}
                Err(f) => errors.extend(f.into_iter().map(|f| format!("{where_}: {f}"))),
            }
        }
        if errors.is_empty() {
            // The filled-in document must pass the schema like a stream
            // without `establishes`.
            let mut copy = stream.document.clone();
            if let Some(events) = copy.get_mut("events").and_then(Value::as_array_mut) {
                for e in events {
                    if let Some(o) = e.as_object_mut() {
                        o.remove("establishes");
                    }
                }
            }
            if let Err(f) = schema::validate(Kind::Stream, &copy) {
                errors.extend(
                    f.into_iter()
                        .map(|f| format!("stream '{}', filled in from the law: {f}", stream.id)),
                );
            }
        }
    }
    errors
}

fn establish_event(
    stream: &mut Stream,
    i: usize,
    law: &BTreeMap<String, LawArticle<'_>>,
    service: &LawExecutionService,
) -> Result<(), Vec<String>> {
    let name = stream.events[i].name.clone();
    let mut errors = Vec::new();
    // Per article in the order of the stream: the establishment or extension.
    let mut parts: Vec<(&LawArticle<'_>, &Establishment, bool)> = Vec::new();
    for r in &stream.events[i].establishes {
        let Some(wa) = law.get(r) else {
            errors.push(format!(
                "article '{r}' is not loaded or has no produces.extensions.{NAMESPACE}"
            ));
            continue;
        };
        let mut found = false;
        for v in &wa.chronolex.establishes {
            if v.event.as_deref() == Some(name.as_str()) {
                parts.push((wa, v, true));
                found = true;
            } else if v.extends.as_deref() == Some(name.as_str()) {
                parts.push((wa, v, false));
                found = true;
            }
        }
        if !found {
            errors.push(format!(
                "article '{r}' neither establishes nor extends '{name}'"
            ));
        }
    }
    let basis: Vec<&(&LawArticle<'_>, &Establishment, bool)> =
        parts.iter().filter(|d| d.2).collect();
    let basis = match basis[..] {
        [b] => b,
        [] => {
            errors.push(format!(
                "none of the articles in establishes establishes '{name}' itself (event: {name})"
            ));
            return Err(errors);
        }
        _ => {
            errors.push(format!("more than one article establishes '{name}'"));
            return Err(errors);
        }
    };
    for (wa, v, is_basis) in &parts {
        if v.event.is_some() && v.extends.is_some() {
            errors.push(format!(
                "{}: event and extends at the same time",
                wa.reference
            ));
        }
        if !is_basis
            && (v.type_.is_some()
                || v.subtype.is_some()
                || v.stage.is_some()
                || !v.refers_to.is_empty())
        {
            errors.push(format!(
                "{}: an extension sets no type, subtype, stage or reference; the article that establishes '{name}' does that",
                wa.reference
            ));
        }
        if let Some(p) = v.effective_at.as_ref().and_then(|o| o.parameter.as_ref()) {
            if !parameter_types(wa.article).contains_key(p) {
                errors.push(format!(
                    "{}: effective_at.parameter '{p}' is not a parameter of this article",
                    wa.reference
                ));
            }
        }
    }
    let (bwa, bv, _) = basis;
    let Some(type_) = bv.type_.clone() else {
        errors.push(format!(
            "{}: establishes '{name}' without a type",
            bwa.reference
        ));
        return Err(errors);
    };
    let mut legal_basis: Vec<String> = Vec::new();
    let mut moment_legal_basis: Vec<String> = Vec::new();
    let mut paths: Vec<(String, String)> = Vec::new();
    let mut alias = BTreeMap::new();
    let mut field_legal_basis: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (wa, v, is_basis) in &parts {
        let g = match &v.legal_basis {
            Some(g) => g.clone(),
            None if *is_basis => vec![wa.reference.clone()],
            None => Vec::new(),
        };
        for x in &g {
            if !legal_basis.contains(x) {
                legal_basis.push(x.clone());
            }
        }
        for x in v.effective_at.iter().flat_map(|o| o.legal_basis.iter()) {
            if !moment_legal_basis.contains(x) {
                moment_legal_basis.push(x.clone());
            }
        }
        match field_paths(service, wa, v, bv.stage.as_deref()) {
            Ok(p) => {
                for path in &p {
                    let entry = field_legal_basis.entry(path.clone()).or_default();
                    for x in &g {
                        if !entry.contains(x) {
                            entry.push(x.clone());
                        }
                    }
                }
                paths.extend(p.into_iter().map(|p| (p, wa.reference.clone())));
            }
            Err(f) => errors.push(f),
        }
        alias.extend(v.aliases.clone());
    }
    for x in legal_basis.iter().chain(&moment_legal_basis) {
        if let Err(f) = crate::regulations::valid(service, x) {
            errors.push(f);
        }
    }

    let event = &mut stream.events[i];
    if !event.refers_to.is_empty() {
        errors.push(
            "the stream names refers_to, but the event has establishes: the references come from the law"
                .into(),
        );
    }
    // The fields: every leaf of the stream falls under a path of the law, and
    // every path of the law has a leaf in the stream.
    let leaves: Vec<String> = event.leaves().into_iter().map(|b| b.path).collect();
    let under = |leaf: &str, path: &str| leaf == path || leaf.starts_with(&format!("{path}."));
    for b in &leaves {
        if !paths.iter().any(|(p, _)| under(b, p)) {
            errors.push(format!(
                "field '{b}' is in the stream, but no article in establishes declares it"
            ));
        }
    }
    for (p, r) in &paths {
        if !leaves.iter().any(|b| under(b, p)) {
            errors.push(format!(
                "{r} declares field '{p}', but the stream does not bind it"
            ));
        }
    }
    match (&mut event.effective_at, moment_legal_basis.is_empty()) {
        (Some(o), false) => o.legal_basis = moment_legal_basis.clone(),
        (Some(_), true) => errors.push(
            "the stream binds effective_at, but the law does not say why that moment counts in law (effective_at.legal_basis)"
                .into(),
        ),
        (None, false) => errors.push(
            "the law names an effective_at, but the stream does not bind it (effective_at.source)".into(),
        ),
        (None, true) => {}
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    event.type_ = type_;
    event.subtype = bv.subtype.clone();
    event.stage = bv.stage.clone();
    event.refers_to = bv.refers_to.clone();
    event.field_types = parts
        .iter()
        .filter_map(|(_, v, _)| match &v.fields {
            Some(Fields::Typed(m)) => Some(m.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    event.legal_basis = legal_basis;
    event.field_legal_basis = field_legal_basis;
    event.aliases = alias;

    // The document (`GET /api/stream`) shows the event as it applies.
    if let Some(e) = stream
        .document
        .get_mut("events")
        .and_then(|e| e.get_mut(i))
        .and_then(Value::as_object_mut)
    {
        let event = &stream.events[i];
        e.insert("type".into(), Value::String(event.type_.clone()));
        if let Some(s) = &event.subtype {
            e.insert("subtype".into(), Value::String(s.clone()));
        }
        if let Some(s) = &event.stage {
            e.insert("stage".into(), Value::String(s.clone()));
        }
        if let Some(v) = bv.refers_to_as_json() {
            e.insert("refers_to".into(), v);
        }
        e.insert("legal_basis".into(), serde_json::json!(event.legal_basis));
        if let Some(o) = e.get_mut("effective_at").and_then(Value::as_object_mut) {
            o.insert("legal_basis".into(), serde_json::json!(moment_legal_basis));
        }
    }
    Ok(())
}

/// An event of the cell: the stream and the event.
struct CellEvent<'a> {
    chronicle: &'a str,
    event: &'a crate::stream::Event,
}

fn cell_events(streams: &[Stream]) -> Vec<CellEvent<'_>> {
    streams
        .iter()
        .flat_map(|s| {
            s.events.iter().map(|e| CellEvent {
                chronicle: &s.chronicle,
                event: e,
            })
        })
        .collect()
}

/// Whether a filter key with value can match an event of the cell. Only the
/// fixed attributes count; a field or an input fixes nothing.
fn touches(e: &crate::stream::Event, filter: &Filter) -> bool {
    filter.iter().all(|(k, v)| {
        if v.starts_with('$') {
            return true;
        }
        match k.as_str() {
            "name" => e.name == *v,
            "type" => e.type_ == *v,
            "subtype" => e.subtype.as_deref() == Some(v),
            "stage" => e.stage.as_deref() == Some(v),
            _ => true,
        }
    })
}

/// Translate `from` into a filter of the reduction. `None`: the events are
/// not in this cell. `event: <name>` and `established_by: <article>` become
/// `name`; `decision: this` fixes nothing (see [`Reading::decision`]); the
/// rest (such as `stage` or a field) stays as it is.
fn filter_from(
    out: &Map<String, Value>,
    events: &[CellEvent<'_>],
    where_: &str,
) -> Result<Option<Filter>, String> {
    let mut f = Filter::new();
    for (k, v) in out {
        let text = match v {
            Value::String(s) => s.clone(),
            Value::Bool(_) | Value::Number(_) => v.to_string(),
            _ => {
                return Err(format!(
                    "{where_}: from.{k} is not a text, number or yes/no"
                ))
            }
        };
        match k.as_str() {
            "event" => {
                if !events.iter().any(|e| e.event.name == text) {
                    return Ok(None);
                }
                f.insert("name".into(), text);
            }
            "established_by" => {
                let names: Vec<&str> = events
                    .iter()
                    .filter(|e| e.event.establishes.contains(&text))
                    .map(|e| e.event.name.as_str())
                    .collect();
                match names[..] {
                    [] => return Ok(None),
                    [n] => {
                        f.insert("name".into(), n.to_string());
                    }
                    _ => {
                        return Err(format!(
                            "{where_}: {text} establishes more than one event in this cell ({}); name the event (from.event)",
                            names.join(", ")
                        ))
                    }
                }
            }
            "decision" if text == "this" => {}
            "decision" => return Err(format!("{where_}: from.decision only accepts 'this'")),
            "stage" => {
                if !events
                    .iter()
                    .any(|e| e.event.stage.as_deref() == Some(&text))
                {
                    return Ok(None);
                }
                f.insert(k.clone(), text);
            }
            _ => {
                f.insert(k.clone(), text);
            }
        }
    }
    Ok(Some(f))
}

/// The lexostatuses the law reads in this cell: one per article with `reads`
/// of which every fact read lies in a stream of the cell. An article none of
/// whose facts lie here belongs to another cell; an article only part of
/// whose facts lie here is an error. `supplements` (from `lexostatuses.yaml`)
/// add extra fields to a lexostatus from the law: what the cell passes on for
/// the synthesis, not a parameter.
pub fn lexostatuses(
    streams: &[Stream],
    service: &LawExecutionService,
    supplements: &[LawSupplement],
) -> Result<Vec<LexostatusDefinition>, Vec<String>> {
    let events = cell_events(streams);
    let has_reads = || {
        service.resolver().list_laws().into_iter().any(|id| {
            service.resolver().get_law(id).is_some_and(|l| {
                l.articles
                    .iter()
                    .any(|a| block(a).is_some_and(|b| b.get("reads").is_some()))
            })
        })
    };
    if !events.iter().any(|e| e.event.case.has_attribute()) || !has_reads() {
        return match supplements.first() {
            Some(a) => Err(vec![format!(
                "law: '{}' is not a lexostatus from the law in this cell",
                a.article
            )]),
            None => Ok(Vec::new()),
        };
    }
    let law = articles(service)?;
    // The type of a parameter that a reading supplies: from the reading
    // article, and otherwise from the article in the corpus that declares it.
    // A reading in policy (note "bron en gram-id") supplies parameters of a
    // law article that it does not declare itself.
    let mut all_types: BTreeMap<String, String> = BTreeMap::new();
    for wa in law.values() {
        for (n, t) in parameter_types(wa.article) {
            all_types.entry(n).or_insert(t);
        }
    }
    for id in service.resolver().list_laws() {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        for a in &law.articles {
            for (n, t) in parameter_types(a) {
                all_types.entry(n).or_insert(t);
            }
        }
    }
    let mut out = Vec::new();
    let mut errors = Vec::new();
    let mut delivered: BTreeMap<String, String> = BTreeMap::new();
    for wa in law.values() {
        let Some(reads) = &wa.chronolex.reads else {
            continue;
        };
        for reading in reads.readings() {
            match definition(wa, reading, &events, supplements, &all_types) {
                Ok(None) => {}
                Ok(Some(d)) => {
                    for p in d.reduction.derivations.keys() {
                        if let Some(other) = delivered.insert(p.clone(), d.name.clone()) {
                            errors.push(format!(
                                "parameter '{p}' comes from two lexostatuses from the law: {other} and {}",
                                d.name
                            ));
                        }
                    }
                    if out.iter().any(|u: &LexostatusDefinition| u.name == d.name) {
                        errors.push(format!(
                            "{}: two readings with the same name; give each reading its own paragraph",
                            d.name
                        ));
                    }
                    out.push(d);
                }
                Err(f) => errors.extend(f),
            }
        }
    }
    for a in supplements {
        if !out.iter().any(|d| d.name == a.article) {
            errors.push(format!(
                "law: '{}' is not a lexostatus from the law in this cell",
                a.article
            ));
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

fn definition(
    wa: &LawArticle<'_>,
    reading: &Reading,
    events: &[CellEvent<'_>],
    supplements: &[LawSupplement],
    all_types: &BTreeMap<String, String>,
) -> Result<Option<LexostatusDefinition>, Vec<String>> {
    // The name: the article, or the paragraph if the reading names one.
    let lexo_name = match &reading.paragraph {
        None => wa.reference.clone(),
        Some(Value::String(l)) => format!("{} lid {l}", wa.reference),
        Some(l) => format!("{} lid {l}", wa.reference),
    };
    let where_ = format!("{lexo_name} (reads)");
    let mut errors = Vec::new();
    let mut types = parameter_types(wa.article);
    for p in reading.parameters.keys() {
        if let (false, Some(t)) = (types.contains_key(p), all_types.get(p)) {
            types.insert(p.clone(), t.clone());
        }
    }
    // Per filter: where it lands in the cell. None: not this cell.
    let mut here = 0usize;
    let mut elsewhere = 0usize;
    let mut top = Filter::new();
    top.insert("root".into(), "$root".into());
    if let Some(u) = &reading.from {
        match filter_from(u, events, &where_) {
            Ok(Some(f)) => {
                here += 1;
                top.extend(f);
            }
            Ok(None) => elsewhere += 1,
            Err(f) => errors.push(f),
        }
    }
    let mut derivations = BTreeMap::new();
    for (name, v) in &reading.parameters {
        let where_ = format!("{where_}, parameter '{name}'");
        if !types.contains_key(name) {
            errors.push(format!("{where_}: not a parameter of this article"));
        }
        let Some(o) = v.as_object() else {
            errors.push(format!("{where_}: a derivation is an object"));
            continue;
        };
        let mut o = o.clone();
        let mut own: Option<Filter> = None;
        if let Some(u) = o.remove("from") {
            let Some(u) = u.as_object() else {
                errors.push(format!("{where_}: from is an object"));
                continue;
            };
            match filter_from(u, events, &where_) {
                Ok(Some(f)) => {
                    here += 1;
                    own = Some(f);
                }
                Ok(None) => {
                    elsewhere += 1;
                    continue;
                }
                Err(f) => {
                    errors.push(f);
                    continue;
                }
            }
        }
        // The name bridge: if the derivation reads a field that the events read
        // carry under another name, then that field.
        let read = own.as_ref().unwrap_or(&top);
        if let Some(Value::String(field)) = o.get("field").cloned() {
            let targets: BTreeSet<Option<&String>> = events
                .iter()
                .filter(|e| touches(e.event, read))
                .map(|e| e.event.aliases.get(&field))
                .collect();
            if let [Some(other)] = targets.into_iter().collect::<Vec<_>>()[..] {
                o.insert("field".into(), Value::String(other.clone()));
            }
        }
        if let Some(f) = own {
            let mut filter = Map::new();
            for (k, w) in f {
                filter.insert(k, Value::String(w));
            }
            o.insert("filter".into(), Value::Object(filter));
        }
        if !o.contains_key("legal_basis") {
            o.insert("legal_basis".into(), serde_json::json!([lexo_name]));
        }
        match serde_json::from_value::<Derived>(Value::Object(o)) {
            Ok(a) => {
                derivations.insert(name.clone(), a);
            }
            Err(e) => errors.push(format!("{where_}: not a derivation: {e}")),
        }
    }
    if here == 0 {
        // No fact of this article in this cell: it belongs to another.
        return if errors.is_empty() {
            Ok(None)
        } else {
            Err(errors)
        };
    }
    if elsewhere > 0 {
        errors.push(format!(
            "{where_}: part of the facts read lie in this cell, part do not; a lexostatus reduces one cell"
        ));
    }
    // The chronicle: that of the events the filters match.
    let mut chronicles: BTreeSet<&str> = BTreeSet::new();
    let filters: Vec<Filter> = std::iter::once(top.clone())
        .chain(
            derivations
                .values()
                .filter_map(|a: &Derived| a.filter().cloned()),
        )
        .collect();
    for f in &filters {
        let mut complete = top.clone();
        complete.extend(f.clone());
        for e in events.iter().filter(|e| touches(e.event, &complete)) {
            chronicles.insert(e.chronicle);
        }
    }
    let chronicle = match chronicles.into_iter().collect::<Vec<_>>()[..] {
        [k] => k.to_string(),
        [] => {
            errors.push(format!(
                "{where_}: no event in this cell matches what the article reads"
            ));
            String::new()
        }
        ref more => {
            errors.push(format!(
                "{where_}: the events read are in more than one chronicle ({})",
                more.join(", ")
            ));
            String::new()
        }
    };
    if reading.decision.as_deref().is_some_and(|b| b != "this") {
        errors.push(format!("{where_}: decision only accepts 'this'"));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let extra_fields = supplements
        .iter()
        .filter(|a| a.article == lexo_name)
        .flat_map(|a| a.extra_fields.clone())
        .collect();
    Ok(Some(LexostatusDefinition {
        name: lexo_name.clone(),
        inputs: vec![InputDefinition {
            name: "root".into(),
            kind: "string".into(),
        }],
        reduction: Reduction {
            chronicle,
            filter: top,
            group_by: None,
            without: Filter::new(),
            pick: reading.pick,
            derivations,
            extra_fields,
        },
        law: Some(LawReading {
            article: lexo_name,
            types,
            decision_this: reading.decision.as_deref() == Some("this"),
        }),
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::{test_gram, Gram};
    use crate::lexostatus_engine::{self, CellRoute, Mode};
    use crate::reduction::{self, AsOf, Derivation};
    use serde_json::json;
    use std::sync::Arc;

    /// A fictitious law: art. 1 decides (a decretogram with its output as a
    /// field, and a name bridge), art. 2 establishes the payment and reads the
    /// determined amount and the sum of the payments.
    const LAW: &str = r#"
$id: testwet_lezing
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.com/testwet_lezing
articles:
  - number: '1'
    text: De instantie besluit op de aanvraag.
    url: https://example.com/testwet_lezing/1
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          decision_type: TOEKENNING
          extensions:
            chronolex:
              establishes:
                - event: besloten
                  type: decretogram
                  stage: BESLUIT
                  effective_at: {parameter: besluitdatum, legal_basis: ['testwet_lezing#1']}
                  fields: outputs
                  aliases: {vastgesteld_bedrag: bedrag_art1}
        parameters:
          - {name: besluitdatum, type: date}
        output:
          - {name: bedrag_art1, type: number}
        actions:
          - {output: bedrag_art1, value: 100}
  - number: '2'
    text: Het bedrag wordt overeenkomstig het besluit betaald.
    url: https://example.com/testwet_lezing/2
    machine_readable:
      execution:
        produces:
          legal_character: TOETS
          decision_type: GEEN_BESLUIT
          extensions:
            chronolex:
              establishes:
                - event: betaald
                  type: executogram
                  subtype: betaling
                  refers_to: {decision: {to: {stage: BESLUIT}, required: true}}
                  fields: [bedrag]
              reads:
                decision: this
                parameters:
                  vastgesteld_bedrag: {from: {stage: BESLUIT}, pick: latest, field: vastgesteld_bedrag}
                  betaald_bedrag: {from: {established_by: 'testwet_lezing#2'}, sum: bedrag}
        parameters:
          - {name: vastgesteld_bedrag, type: number}
          - {name: betaald_bedrag, type: number}
        output:
          - {name: nog_te_betalen, type: number}
        actions:
          - output: nog_te_betalen
            value: {operation: SUBTRACT, values: [$vastgesteld_bedrag, $betaald_bedrag]}
"#;

    fn stream(fields_decision: &str) -> Stream {
        let text = format!(
            r#"
$id: test_verloop
recording_actor: test_instantie
chronicle: test_kroniek
events:
  - name: besloten
    establishes: ['testwet_lezing#1']
    intake: behandelaar
    effective_at: {{source: $external.besluitdatum}}
    fields: {{{fields_decision}}}
  - name: betaald
    establishes: ['testwet_lezing#2']
    intake: behandelaar
    fields: {{bedrag: $external.bedrag}}
"#
        );
        crate::stream::parse(&text, "test").unwrap()
    }

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(LAW).unwrap();
        s
    }

    fn gram(name: &str, stage: Option<&str>, fields: Value, moment: &str) -> Gram {
        let mut g = test_gram("Z1");
        g.name = name.into();
        g.stage = stage.map(str::to_string);
        g.chronicle = "test_kroniek".into();
        g.fields = fields.as_object().cloned().unwrap();
        g.effective_at = moment.into();
        g.recorded_at = moment.into();
        g
    }

    #[test]
    fn the_law_fills_in_the_event() {
        let s = service();
        let mut streams = vec![stream("bedrag_art1: $external.bedrag_art1")];
        assert_eq!(establish(&mut streams, &s), Vec::<String>::new());
        crate::stream::derive_roles(&mut streams);
        let e = &streams[0].events[0];
        assert_eq!(e.type_, "decretogram");
        assert_eq!(e.stage.as_deref(), Some("BESLUIT"));
        assert_eq!(e.decision, Some(crate::stream::Decision::Opens));
        assert_eq!(
            streams[0].events[1].refers_to["decision"].to,
            crate::stream::To::Stage("BESLUIT".into())
        );
        assert_eq!(
            streams[0].events[1].decision,
            Some(crate::stream::Decision::Follows)
        );
        assert_eq!(e.legal_basis, ["testwet_lezing#1"]);
        assert_eq!(
            e.effective_at.as_ref().unwrap().legal_basis,
            ["testwet_lezing#1"]
        );
        assert_eq!(e.aliases["vastgesteld_bedrag"], "bedrag_art1");
        // The document of the stream shows the event as it applies.
        assert_eq!(streams[0].document["events"][0]["type"], "decretogram");
    }

    /// Each field keeps the legal basis of the establishment or extension that
    /// declares it; the event carries the union.
    #[test]
    fn each_field_keeps_the_legal_basis_that_declares_it() {
        const FIELDS: &str = r#"
$id: testwet_velden
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.com/testwet_velden
articles:
  - number: '1'
    text: De instantie ontvangt de akte.
    url: https://example.com/testwet_velden/1
    machine_readable:
      execution:
        produces:
          extensions:
            chronolex:
              establishes:
                - event: akte_ontvangen
                  type: submission
                  subtype: akte
  - number: '2'
    text: "1. De akte vermeldt de dag.\n\n2. Bij de akte hoort een verklaring."
    url: https://example.com/testwet_velden/2
    machine_readable:
      execution:
        produces:
          extensions:
            chronolex:
              establishes:
                - extends: akte_ontvangen
                  legal_basis: ['testwet_velden#2 lid 1']
                  fields: [datum]
                - extends: akte_ontvangen
                  legal_basis: ['testwet_velden#2 lid 2']
                  fields: [verklaring]
"#;
        let mut s = LawExecutionService::new();
        s.load_law(FIELDS).unwrap();
        let mut streams = vec![crate::stream::parse(
            r#"
$id: test_akten
recording_actor: test_instantie
chronicle: test_kroniek
events:
  - name: akte_ontvangen
    establishes: ['testwet_velden#1', 'testwet_velden#2']
    intake: behandelaar
    fields: {datum: $external.datum, verklaring: $external.verklaring}
"#,
            "test",
        )
        .unwrap()];
        assert_eq!(establish(&mut streams, &s), Vec::<String>::new());
        let e = &streams[0].events[0];
        assert_eq!(
            e.legal_basis,
            [
                "testwet_velden#1",
                "testwet_velden#2 lid 1",
                "testwet_velden#2 lid 2"
            ]
        );
        assert_eq!(e.field_legal_basis["datum"], ["testwet_velden#2 lid 1"]);
        assert_eq!(
            e.field_legal_basis["verklaring"],
            ["testwet_velden#2 lid 2"]
        );
    }

    #[test]
    fn a_field_the_law_does_not_name_is_an_error() {
        let s = service();
        let mut streams = vec![stream(
            "bedrag_art1: $external.bedrag_art1, notitie: $external.notitie",
        )];
        let f = establish(&mut streams, &s);
        assert!(f.iter().any(|f| f.contains("field 'notitie'")), "{f:?}");
        // And the other way around: an output the stream does not bind.
        let mut streams = vec![stream("notitie: $external.notitie")];
        let f = establish(&mut streams, &s);
        assert!(f.iter().any(|f| f.contains("'bedrag_art1'")), "{f:?}");
    }

    #[test]
    fn the_reading_article_becomes_a_lexostatus_the_engine_reads_the_same_way() {
        let s = service();
        let mut streams = vec![stream("bedrag_art1: $external.bedrag_art1")];
        assert!(establish(&mut streams, &s).is_empty());
        crate::stream::derive_roles(&mut streams);
        let defs = lexostatuses(&streams, &s, &[]).unwrap();
        assert_eq!(defs.len(), 1);
        let d = &defs[0];
        assert_eq!(d.name, "testwet_lezing#2");
        assert_eq!(d.reduction.chronicle, "test_kroniek");
        // The name bridge of art. 1: art. 2 reads its own name, the gram
        // carries that of art. 1.
        match &d.reduction.derivations["vastgesteld_bedrag"].derivation {
            Derivation::LatestField { field, filter, .. } => {
                assert_eq!(field, "bedrag_art1");
                assert_eq!(filter["stage"], "BESLUIT");
            }
            a => panic!("{a:?}"),
        }
        match &d.reduction.derivations["betaald_bedrag"].derivation {
            Derivation::Sum { filter, .. } => assert_eq!(filter["name"], "betaald"),
            a => panic!("{a:?}"),
        }
        assert!(d.law.as_ref().unwrap().decision_this);
        // Without its own legal basis a derivation rests on the reading article.
        assert_eq!(
            d.reduction.derivations["betaald_bedrag"].legal_basis,
            ["testwet_lezing#2"]
        );

        let grams = vec![
            gram(
                "besloten",
                Some("BESLUIT"),
                json!({"bedrag_art1": 100}),
                "2025-03-01T10:00:00+01:00",
            ),
            gram(
                "betaald",
                None,
                json!({"bedrag": 30}),
                "2025-03-02T10:00:00+01:00",
            ),
            gram(
                "betaald",
                None,
                json!({"bedrag": 20}),
                "2025-03-03T10:00:00+01:00",
            ),
        ];
        let inputs = json!({"root": "Z1"}).as_object().cloned().unwrap();
        let l = reduction::reduce(d, &inputs, &grams).unwrap().unwrap();
        assert_eq!(l.parameters["vastgesteld_bedrag"], json!(100));
        assert_eq!(l.parameters["betaald_bedrag"], json!(50));

        // The engine route: the regulation from the reading, compared with the
        // DSL (a difference is an error).
        let mut lexo = LawExecutionService::new();
        let text =
            crate::engine_regulation::regulation(d, "lexostatus_test", &BTreeMap::new()).unwrap();
        let id = lexo.load_law(&text).unwrap();
        let route = CellRoute {
            service: Arc::new(lexo),
            modes: BTreeMap::from([(
                d.name.clone(),
                Mode::Engine {
                    regulation: id,
                    article: Some(d.name.clone()),
                },
            )]),
            compare: true,
        };
        for g in [&grams[..1], &grams[..], &[]] {
            let r = lexostatus_engine::reduce_lexostatus(
                &route,
                d,
                &inputs,
                g,
                &AsOf::default(),
                "2025-06-01",
                false,
            );
            let l = r.unwrap().unwrap();
            assert_eq!(
                l.reduction.unwrap().regulation.as_deref(),
                Some("testwet_lezing#2")
            );
        }
    }

    #[test]
    fn one_reading_per_paragraph_is_a_lexostatus_of_its_own() {
        let law = LAW.replace(
            "              reads:\n                decision: this\n                parameters:\n                  vastgesteld_bedrag: {from: {stage: BESLUIT}, pick: latest, field: vastgesteld_bedrag}\n                  betaald_bedrag: {from: {established_by: 'testwet_lezing#2'}, sum: bedrag}\n",
            "              reads:\n                - parameters:\n                    vastgesteld_bedrag: {from: {stage: BESLUIT}, pick: latest, field: vastgesteld_bedrag}\n                - paragraph: 2\n                  parameters:\n                    betaald_bedrag: {from: {established_by: 'testwet_lezing#2'}, sum: bedrag}\n",
        );
        assert_ne!(law, LAW, "the replacement matched nothing");
        let mut s = LawExecutionService::new();
        s.load_law(&law).unwrap();
        let mut streams = vec![stream("bedrag_art1: $external.bedrag_art1")];
        assert!(establish(&mut streams, &s).is_empty());
        crate::stream::derive_roles(&mut streams);
        let names: Vec<String> = lexostatuses(&streams, &s, &[])
            .unwrap()
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert_eq!(names, ["testwet_lezing#2", "testwet_lezing#2 lid 2"]);
    }

    #[test]
    fn a_supplement_to_an_unknown_article_is_an_error() {
        let s = service();
        let mut streams = vec![stream("bedrag_art1: $external.bedrag_art1")];
        assert!(establish(&mut streams, &s).is_empty());
        crate::stream::derive_roles(&mut streams);
        let a: LawSupplement = serde_json::from_value(json!({
            "article": "testwet_lezing#1",
            "extra_fields": {"x": {"field": "bedrag_art1"}}
        }))
        .unwrap();
        let f = lexostatuses(&streams, &s, &[a]).unwrap_err();
        assert!(f[0].contains("testwet_lezing#1"), "{f:?}");
    }
}

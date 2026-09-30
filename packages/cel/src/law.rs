//! The law as the source of the gram shape and the reduction (proposal
//! "lexostatus in de wet", 28-09-2026; note "het gram uit de wet",
//! 29-09-2026; RFC-046).
//!
//! An article can say two things in `produces.extensions.chronolex`
//! (RFC-022 §3.2: one namespace per integration):
//!
//! - `establishes`: the facts this article brings into being, as an event of a
//!   chronicle: the type, the subtype, the stage, which gram it refers to
//!   (`refers_to`, with a name from the law text: a decision `on_application`,
//!   a payment to the `decision`), the legal basis, why the `effective_at`
//!   counts in law and which fields the gram carries. Or it extends an event
//!   that another article establishes (`extends: <event>`), adding fields and
//!   legal basis. Or it contributes to the submission its hook applies to
//!   (`extends: {submission: AANVRAAG}`, RFC-046).
//! - `reads`: how the article reads its own parameters from the chronicle, in
//!   the vocabulary of the reduction ([`crate::reduction`]). That is a
//!   lexostatus "from the requested perspective" (position paper): the
//!   perspective is the article that asks. It is named after the article
//!   (`<regulation>#<article>`).
//!
//! Which articles make up an application is decided by executing the law,
//! not here. Wpp 102 says that it establishes an application
//! (`produces.submission`); Wpp 107 says that it decides on it
//! (`produces.decides_on`); Awb 4:2 hooks onto every application on which a
//! beschikking is taken. The engine fires those hooks when Wpp 102 runs, and
//! [`establish`] asks the engine which hooks those are
//! (`find_submission_hooks`), so the gram and the execution cannot disagree.
//!
//! A stream names per event the article that establishes it (`establishes:`)
//! and holds only the registration: which cell records the fact, and through
//! which intake. [`establish`] derives the rest: the fields with their type and
//! origin, and how each binds. A field is input by default (`$external`); only
//! an origin says otherwise (the decision requested is fixed, what the channel
//! or a register supplies binds to `$supplied`). [`lexostatuses`] turns every
//! reading article whose facts lie in the cell into a lexostatus definition,
//! which then goes through the same checks and the same reduction as a
//! definition from `cell.yaml`.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_engine::{Article, ArticleBasedLaw, LawExecutionService};
use regelrecht_law_model::{Declared, Origin, OriginOverride, OriginRole, OriginValue};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::reduction::{
    Derived, Filter, InputDefinition, LawSupplement, LexostatusDefinition, Pick, Reduction,
};
use crate::schema::{self, Kind};
use crate::stream::{EffectiveAtBinding, Reference, Stream, SUPPLIED_BINDING};

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

/// What an extension extends: an event by name, or the submission (such as an
/// application) that a hook of the article applies to (RFC-046). Which
/// submission that is, the engine says: the article takes part when its hook
/// fires on the article that establishes the submission.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Extends {
    Event(String),
    Submission { submission: String },
}

/// A fact that an article establishes, or the extension of a fact that
/// another article establishes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Establishment {
    /// The event this article establishes (the name in the chronicle).
    #[serde(default)]
    pub event: Option<String>,
    /// The event (by name) or the stage (every event at that stage, through
    /// a hook of this article) that this article extends.
    #[serde(default)]
    pub extends: Option<Extends>,
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
    /// an event, for an extension the legal basis of the origin of each
    /// field it declares.
    #[serde(default)]
    pub legal_basis: Option<Vec<String>>,
    #[serde(default)]
    pub effective_at: Option<EffectiveAtLaw>,
    #[serde(default)]
    pub fields: Option<Fields>,
    /// Name bridge: under which name another article reads a field of this
    /// gram (`<name at the reader>: <field of the gram>`). This is how Awb 4:52
    /// reads "het vastgestelde bedrag" without knowing the name of the output
    /// of the specific law. If both names are fields of the event, they are
    /// one field with both legal bases (one question: Wpp 102 lid 3 onder a
    /// asks the statutory name, Awb 4:2 lid 1 the name of the applicant).
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    /// Fields a register may fill in beforehand: per field the output of the
    /// policy that knows it (the policy of the competent authority that says
    /// it fetches the data itself, such as the statutory name from the
    /// commercial register). Where the register knows nothing, the applicant
    /// fills it in.
    #[serde(default)]
    pub prefill: BTreeMap<String, PrefillLaw>,
}

/// Where a field can be filled in beforehand: an output of a regulation (by
/// default the regulation of the article that says so).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefillLaw {
    #[serde(default)]
    pub regulation: Option<String>,
    pub output: String,
}

/// A field a register may fill in beforehand, as the portal runs it: the
/// output of the regulation, with the legal basis of the policy that says
/// so. The parameters of the article of that output come from what the
/// channel supplies, by name.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Prefill {
    pub regulation: String,
    pub output: String,
    pub legal_basis: Vec<String>,
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

    fn extends_event(&self) -> Option<&str> {
        match &self.extends {
            Some(Extends::Event(e)) => Some(e),
            _ => None,
        }
    }

    fn extends_submission(&self) -> Option<&str> {
        match &self.extends {
            Some(Extends::Submission { submission }) => Some(submission),
            _ => None,
        }
    }
}

/// Why the `effective_at` counts in law, and which piece of data it is. The
/// runtime derives the binding: a parameter or a field binds to itself, a
/// moment without either (the receipt, Awb 4:13) to `$intake.received_at`.
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
    /// Only when the moment is stated (the counter gives the day of the date
    /// stamp), or only when nothing states it and the recording counts (the
    /// portal). Without it: always.
    #[serde(default)]
    pub only: Option<MomentSource>,
}

/// Which moment a legal basis of `effective_at` is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MomentSource {
    /// A submitted value states the moment.
    Stated,
    /// Nothing states it: the moment of recording counts.
    Recorded,
}

/// The fields of a gram: a list of field paths, or a keyword.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Fields {
    /// `outputs`: the outputs of this article (a decision carries what the
    /// decision outputs). `stage`: what the stage of the procedure requires
    /// (`requires`, RFC-008) and the outputs of the hooks on that stage
    /// (RFC-007), such as the objection period of Awb 6:8 on publication.
    /// `parameters`: the parameters of this article that the applicant or
    /// the channel supplies (origin BELANGHEBBENDE or KANAAL), with their
    /// type and origin, such as the core of an application in Awb 4:2 lid 1.
    Keyword(String),
    /// Field paths under `fields` (`content.subsidiejaar`); a path covers
    /// everything below it.
    List(Vec<String>),
    /// Named parameters of this article, with their type and origin:
    /// `{parameters: [subsidiejaar, statutaire_naam]}`.
    Selection(Selection),
    /// Field paths with their type, as Awb 4:87 names the amount of a
    /// payment: `{bedrag: {type: amount, unit: eurocent}}`, optionally with
    /// the columns of a table and an origin.
    Typed(BTreeMap<String, FieldType>),
}

/// Named parameters of an article as fields.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub parameters: Vec<String>,
}

/// The type of a field of a gram, as the establishing article names it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldType {
    #[serde(rename = "type")]
    pub type_: regelrecht_law_model::ParameterType,
    #[serde(default)]
    pub unit: Option<String>,
    /// For a table: its columns.
    #[serde(default)]
    pub columns: Option<Vec<String>>,
    /// Who supplies the field (RFC-043); without it the field is input.
    #[serde(default)]
    pub origin: Option<Origin>,
}

/// How a field came into the gram: from the article that establishes the
/// event, from one that extends it by name, or from one that hooks onto its
/// stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Via {
    Establishes,
    Extends,
    Hook,
}

/// A field of an event as the law declares it. The document of the stream
/// (`GET /api/stream`) and the form show it per field: which article, how
/// (establishes, extends or through a hook), the type and the origin.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    /// The origin in force: from the law, or from the policy that takes part
    /// in the event (`origins`, RFC-043).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    /// The article of the policy whose `origins` gives the origin in force.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_policy: Option<String>,
    /// `required: false` in the law (RFC-036): the applicant may leave it
    /// out, and the form says so (Awb 4:4 lid 2).
    pub optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub legal_basis: Vec<String>,
    /// The article that declares the field (`<regulation>#<article>`).
    pub declared_by: String,
    pub via: Via,
}

impl FieldDef {
    /// Whether the channel supplies the field (origin KANAAL in force).
    pub fn from_channel(&self) -> bool {
        self.origin
            .as_ref()
            .is_some_and(|o| o.waarde == OriginValue::Kanaal)
    }

    /// Whether the field is the decision requested (`rol: GEVRAAGD_BESLUIT`).
    pub fn is_requested_decision(&self) -> bool {
        self.origin
            .as_ref()
            .is_some_and(|o| o.rol == Some(OriginRole::GevraagdBesluit))
    }
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
    pub regulation: &'s ArticleBasedLaw,
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

/// The version of a regulation that applies on `date` (the newest without a
/// date, or when none applies yet).
fn version<'s>(
    service: &'s LawExecutionService,
    id: &str,
    date: Option<NaiveDate>,
) -> Option<&'s ArticleBasedLaw> {
    let resolver = service.resolver();
    resolver
        .get_law_for_date(id, date)
        .or_else(|| resolver.get_law(id))
}

/// Every article in the corpus with a chronolex block, by reference, in the
/// version that applies on `date` (the newest without one). A block that
/// cannot be read is an error naming the article.
pub fn articles(
    service: &LawExecutionService,
    date: Option<NaiveDate>,
) -> Result<BTreeMap<String, LawArticle<'_>>, Vec<String>> {
    let resolver = service.resolver();
    let mut out = BTreeMap::new();
    let mut errors = Vec::new();
    for id in resolver.list_laws() {
        let Some(law) = version(service, id, date) else {
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
                            regulation: law,
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

/// A type as the law writes it (`date`, `number`, ...).
fn type_text(t: regelrecht_law_model::ParameterType) -> String {
    serde_json::to_value(t)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The parameters of an article with their type (`date`, `number`, ...).
fn parameter_types(article: &Article) -> BTreeMap<String, String> {
    article
        .get_parameters()
        .iter()
        .map(|p| (p.name.clone(), type_text(p.param_type)))
        .collect()
}

/// Whether the article declares a hook on this stage, for this legal
/// character (any, if the event does not know it or the hook names none).
fn hooks_onto(article: &Article, stage: &str, legal_character: Option<&str>) -> bool {
    article
        .machine_readable
        .as_ref()
        .and_then(|m| m.hooks.as_ref())
        .is_some_and(|h| {
            h.iter().any(|h| {
                h.applies_to.stage.as_deref() == Some(stage)
                    && match (legal_character, h.applies_to.legal_character.as_deref()) {
                        (Some(lc), Some(of_hook)) => lc == of_hook,
                        _ => true,
                    }
            })
        })
}

/// Whether the article declares a hook on a submission of this kind (RFC-046).
fn hooks_onto_submission(article: &Article, kind: &str) -> bool {
    article
        .machine_readable
        .as_ref()
        .and_then(|m| m.hooks.as_ref())
        .is_some_and(|h| {
            h.iter()
                .any(|h| h.applies_to.submission.as_deref() == Some(kind))
        })
}

/// What a stage requires: `requires` of that stage in a procedure (RFC-008)
/// and the outputs of the articles with a hook on that stage (RFC-007), for
/// the legal character if the event knows it.
fn stage_fields(
    service: &LawExecutionService,
    stage: &str,
    legal_character: Option<&str>,
    date: Option<NaiveDate>,
) -> Vec<String> {
    let resolver = service.resolver();
    let mut out: Vec<String> = Vec::new();
    let mut add = |n: String| {
        if !out.contains(&n) {
            out.push(n);
        }
    };
    for id in resolver.list_laws() {
        let Some(law) = version(service, id, date) else {
            continue;
        };
        for p in law
            .procedure
            .iter()
            .flatten()
            .filter(|p| legal_character.is_none_or(|lc| p.applies_to.legal_character == lc))
        {
            for s in p.stages.iter().filter(|s| s.name == stage) {
                for r in s.requires.iter().flatten() {
                    add(r.name.clone());
                }
            }
        }
    }
    for id in resolver.list_laws() {
        let Some(law) = version(service, id, date) else {
            continue;
        };
        for article in &law.articles {
            if hooks_onto(article, stage, legal_character) {
                for o in outputs(article) {
                    add(o);
                }
            }
        }
    }
    out
}

/// What is taken on a submission decides its stage and the hooks that apply
/// (RFC-046): the decisions that name the establishing article in
/// `produces.decides_on`, their legal character, and the first stage of the
/// procedure for it (RFC-008: AANVRAAG). `None` stage: no decision is taken
/// on it, or its procedure has no stages (a regulation of general scope, for
/// which Awb 4:2 does not apply). More than one legal character is an error
/// until there is a real case for it.
fn decided(
    service: &LawExecutionService,
    law_id: &str,
    article: &str,
) -> Result<(Vec<String>, Option<String>), String> {
    let decisions = service.resolver().decisions_on(law_id, article);
    let characters: BTreeSet<&str> = decisions
        .iter()
        .map(|d| d.legal_character.as_str())
        .collect();
    let references = decisions
        .iter()
        .map(|d| format!("{}#{}", d.law_id, d.article_number))
        .collect();
    let lc = match characters.into_iter().collect::<Vec<_>>()[..] {
        [] => return Ok((references, None)),
        [one] => one,
        ref more => {
            return Err(format!(
                "decisions of more than one legal character are taken on it ({}): the general law would apply twice",
                more.join(", ")
            ))
        }
    };
    let stage = service
        .resolver()
        .find_procedure(lc, None)
        .and_then(|p| p.stages.first())
        .map(|s| s.name.clone());
    Ok((references, stage))
}

/// Fill in the events with `establishes` from the law, in the version that
/// applies on `date` (the newest without one), and check them. An error names
/// the stream, the event and the article.
pub fn establish(
    streams: &mut [Stream],
    service: &LawExecutionService,
    date: Option<NaiveDate>,
) -> Vec<String> {
    if !streams
        .iter()
        .any(|s| s.events.iter().any(|e| !e.establishes.is_empty()))
    {
        return Vec::new();
    }
    let law = match articles(service, date) {
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
            match establish_event(stream, i, &law, service, date) {
                Ok(()) => {}
                Err(f) => errors.extend(f.into_iter().map(|f| format!("{where_}: {f}"))),
            }
        }
        if errors.is_empty() {
            // The filled-in document must pass the schema like any stream;
            // what the runtime adds for the reader is not part of it.
            let mut copy = stream.document.clone();
            if let Some(events) = copy.get_mut("events").and_then(Value::as_array_mut) {
                for e in events {
                    if let Some(o) = e.as_object_mut() {
                        o.remove("decided_by");
                        o.remove("field_sources");
                        o.remove("prefill");
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

/// One contribution of an article to an event.
struct Part<'a, 's> {
    law: &'a LawArticle<'s>,
    establishment: &'a Establishment,
    via: Via,
}

impl Part<'_, '_> {
    fn is_basis(&self) -> bool {
        self.via == Via::Establishes
    }

    /// The legal basis this part states: its own, the article for the
    /// establishing one, otherwise none.
    fn legal_basis(&self) -> Vec<String> {
        match &self.establishment.legal_basis {
            Some(g) => g.clone(),
            None if self.is_basis() => vec![self.law.reference.clone()],
            None => Vec::new(),
        }
    }
}

fn push_new(to: &mut Vec<String>, from: &[String]) {
    for x in from {
        if !to.contains(x) {
            to.push(x.clone());
        }
    }
}

/// The fields a part declares.
fn part_fields(
    service: &LawExecutionService,
    part: &Part<'_, '_>,
    stage: Option<&str>,
    legal_character: Option<&str>,
    date: Option<NaiveDate>,
) -> Result<Vec<FieldDef>, String> {
    let article = part.law.article;
    let reference = &part.law.reference;
    let stated = part.legal_basis();
    let def = |name: String| FieldDef {
        name,
        type_: None,
        unit: None,
        columns: None,
        origin: None,
        origin_policy: None,
        optional: false,
        description: None,
        legal_basis: stated.clone(),
        declared_by: reference.clone(),
        via: part.via,
    };
    let from_parameter = |p: &regelrecht_law_model::Parameter| {
        let origin = p.origin.as_ref().and_then(Declared::as_valid).cloned();
        let mut legal_basis = stated.clone();
        if let Some(o) = &origin {
            push_new(&mut legal_basis, std::slice::from_ref(&o.grondslag));
        }
        FieldDef {
            type_: Some(type_text(p.param_type)),
            unit: p.type_spec.as_ref().and_then(|t| t.unit.clone()),
            origin,
            optional: p.required == Some(false),
            description: p.description.clone(),
            legal_basis,
            ..def(p.name.clone())
        }
    };
    Ok(match &part.establishment.fields {
        None => Vec::new(),
        Some(Fields::List(l)) => l.iter().cloned().map(def).collect(),
        Some(Fields::Typed(m)) => m
            .iter()
            .map(|(name, t)| {
                let mut legal_basis = stated.clone();
                if let Some(o) = &t.origin {
                    push_new(&mut legal_basis, std::slice::from_ref(&o.grondslag));
                }
                FieldDef {
                    type_: Some(type_text(t.type_)),
                    unit: t.unit.clone(),
                    columns: t.columns.clone(),
                    origin: t.origin.clone(),
                    legal_basis,
                    ..def(name.clone())
                }
            })
            .collect(),
        Some(Fields::Selection(s)) => {
            let mut out = Vec::new();
            for name in &s.parameters {
                let p = article
                    .get_parameters()
                    .iter()
                    .find(|p| &p.name == name)
                    .ok_or_else(|| {
                        format!("{reference}: fields names parameter '{name}', which the article does not have")
                    })?;
                out.push(from_parameter(p));
            }
            out
        }
        Some(Fields::Keyword(t)) if t == "parameters" => article
            .get_parameters()
            .iter()
            .filter(|p| {
                p.origin
                    .as_ref()
                    .and_then(Declared::as_valid)
                    .is_some_and(|o| {
                        matches!(o.waarde, OriginValue::Belanghebbende | OriginValue::Kanaal)
                    })
            })
            .map(from_parameter)
            .collect(),
        Some(Fields::Keyword(t)) if t == "outputs" => {
            let spec = article.get_execution_spec();
            outputs(article)
                .into_iter()
                .map(|o| {
                    let out = spec
                        .and_then(|e| e.output.as_ref())
                        .into_iter()
                        .flatten()
                        .find(|x| x.name == o);
                    FieldDef {
                        type_: out.map(|x| type_text(x.output_type)),
                        unit: out
                            .and_then(|x| x.type_spec.as_ref())
                            .and_then(|t| t.unit.clone()),
                        ..def(o)
                    }
                })
                .collect()
        }
        Some(Fields::Keyword(t)) if t == "stage" => match stage {
            Some(s) => stage_fields(service, s, legal_character, date)
                .into_iter()
                .map(def)
                .collect(),
            None => return Err(format!("{reference}: fields: stage, but the event has no stage")),
        },
        Some(Fields::Keyword(t)) => {
            return Err(format!(
                "{reference}: fields '{t}' is not a list and not a keyword (outputs, stage, parameters)"
            ))
        }
    })
}

/// The overrides of an origin by the policy that takes part in the event,
/// per (regulation, parameter): `origins` (RFC-043) of every article of an
/// implementing policy of which an article takes part (the policy that
/// extends the event may say elsewhere who supplies a field, such as a
/// provision that only overrides origins). With the article that says so.
fn origins_of(parts: &[Part<'_, '_>]) -> BTreeMap<(String, String), (Origin, String)> {
    let mut out = BTreeMap::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for p in parts {
        let law = p.law.regulation;
        if law.regulatory_layer != regelrecht_engine::RegulatoryLayer::Uitvoeringsbeleid
            || !seen.insert(law.id.as_str())
        {
            continue;
        }
        for a in &law.articles {
            let Some(origins) = a.machine_readable.as_ref().and_then(|m| m.origins.as_ref()) else {
                continue;
            };
            for o in origins
                .iter()
                .filter_map(Declared::<OriginOverride>::as_valid)
            {
                out.insert(
                    (o.regulation.clone(), o.parameter.clone()),
                    (o.origin.clone(), format!("{}#{}", law.id, a.number)),
                );
            }
        }
    }
    out
}

/// Set a binding at a path of a YAML mapping, creating the branches.
fn set_yaml(map: &mut serde_yaml_ng::Mapping, path: &str, value: serde_yaml_ng::Value) {
    use serde_yaml_ng::Value as Y;
    match path.split_once('.') {
        None => {
            map.insert(Y::String(path.to_string()), value);
        }
        Some((head, rest)) => {
            let entry = map
                .entry(Y::String(head.to_string()))
                .or_insert_with(|| Y::Mapping(serde_yaml_ng::Mapping::new()));
            if !entry.is_mapping() {
                *entry = Y::Mapping(serde_yaml_ng::Mapping::new());
            }
            if let Y::Mapping(m) = entry {
                set_yaml(m, rest, value);
            }
        }
    }
}

/// The binding of a field, derived from its origin: the decision requested
/// is fixed, a table binds its rows, what the channel or a register supplies
/// binds to `$supplied`, and everything else is input (`$external`).
fn binding_of(
    f: &FieldDef,
    decided_by: &[String],
    prefilled: bool,
) -> Result<serde_yaml_ng::Value, String> {
    use serde_yaml_ng::Value as Y;
    if f.is_requested_decision() {
        if decided_by.is_empty() {
            return Err(format!(
                "field '{}' is the decision requested (rol GEVRAAGD_BESLUIT), but no decision is taken on the submission (produces.decides_on of a decision article)",
                f.name
            ));
        }
        return Ok(Y::String(decided_by.join(", ")));
    }
    if let Some(columns) = &f.columns {
        let mut m = serde_yaml_ng::Mapping::new();
        m.insert(
            Y::String("table".into()),
            Y::String(format!("$external.{}", f.name)),
        );
        m.insert(
            Y::String("columns".into()),
            Y::Sequence(columns.iter().cloned().map(Y::String).collect()),
        );
        return Ok(Y::Mapping(m));
    }
    Ok(Y::String(if f.from_channel() || prefilled {
        format!("{SUPPLIED_BINDING}{}", f.name)
    } else {
        format!("$external.{}", f.name)
    }))
}

fn establish_event(
    stream: &mut Stream,
    i: usize,
    law: &BTreeMap<String, LawArticle<'_>>,
    service: &LawExecutionService,
    date: Option<NaiveDate>,
) -> Result<(), Vec<String>> {
    let name = stream.events[i].name.clone();
    let listed = stream.events[i].establishes.clone();
    let mut errors = Vec::new();

    // The article that establishes the event, among those the stream names;
    // a listed article must take part.
    let mut parts: Vec<Part<'_, '_>> = Vec::new();
    for r in &listed {
        let Some(wa) = law.get(r) else {
            errors.push(format!(
                "article '{r}' is not loaded or has no produces.extensions.{NAMESPACE}"
            ));
            continue;
        };
        let mut found = false;
        for v in &wa.chronolex.establishes {
            if v.event.as_deref() == Some(name.as_str()) {
                parts.push(Part {
                    law: wa,
                    establishment: v,
                    via: Via::Establishes,
                });
                found = true;
            } else if v.extends_event() == Some(name.as_str()) {
                found = true;
            }
        }
        if !found {
            errors.push(format!(
                "article '{r}' neither establishes nor extends '{name}'"
            ));
        }
    }
    let basis = match parts.len() {
        1 => 0,
        0 => {
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
    let bv = parts[basis].establishment;
    let bwa = parts[basis].law;
    // A submission (RFC-046): the engine says which articles hook onto it,
    // by the rule it fires them when the establishing article runs, and which
    // decisions are taken on it. An article that produces a submission may
    // establish other facts too (Wpp 102 also the verdict on a late
    // application); the submission is the event it establishes without a
    // type of its own.
    let submission = bwa
        .article
        .get_produces()
        .and_then(|p| p.submission.as_ref())
        .filter(|_| bv.type_.is_none())
        .map(|s| s.kind.clone());
    let (law_id, number) = (&bwa.regulation.id, &bwa.article.number);
    let (decisions, stage) = match &submission {
        None => (Vec::new(), bv.stage.clone()),
        Some(_) => match decided(service, law_id, number) {
            Ok((d, s)) => (d, bv.stage.clone().or(s)),
            Err(f) => {
                errors.push(format!("{}: {f}", bwa.reference));
                (Vec::new(), bv.stage.clone())
            }
        },
    };

    // The articles that hook onto the submission (the general law that
    // applies itself), then the extensions by name (the specific law and the
    // policy), each in the order of the corpus. The legal basis of the gram
    // follows this order: the establishing article first.
    if let Some(kind) = &submission {
        let resolver = service.resolver();
        let mut hooked: Vec<String> = Vec::new();
        for point in [
            regelrecht_law_model::HookPoint::PreActions,
            regelrecht_law_model::HookPoint::PostActions,
        ] {
            for h in resolver.find_submission_hooks(point, kind, law_id, number) {
                push_new(&mut hooked, &[format!("{}#{}", h.law_id, h.article_number)]);
            }
        }
        hooked.sort();
        for r in &hooked {
            // A hook article without a chronolex block takes part in the
            // execution but adds nothing to the gram.
            let Some(wa) = law.get(r) else { continue };
            for v in &wa.chronolex.establishes {
                if v.extends_submission() == Some(kind.as_str()) {
                    parts.push(Part {
                        law: wa,
                        establishment: v,
                        via: Via::Hook,
                    });
                }
            }
        }
    }
    for wa in law.values() {
        for v in &wa.chronolex.establishes {
            let Some(k) = v.extends_submission() else {
                continue;
            };
            if !hooks_onto_submission(wa.article, k) {
                errors.push(format!(
                    "{}: extends {{submission: {k}}}, but the article declares no hook on a submission {k}",
                    wa.reference
                ));
            }
        }
    }

    for wa in law.values() {
        for v in &wa.chronolex.establishes {
            if v.extends_event() == Some(name.as_str()) {
                parts.push(Part {
                    law: wa,
                    establishment: v,
                    via: Via::Extends,
                });
            }
        }
    }
    for p in &parts {
        let v = p.establishment;
        let wa = p.law;
        if v.event.is_some() && v.extends.is_some() {
            errors.push(format!(
                "{}: event and extends at the same time",
                wa.reference
            ));
        }
        if !p.is_basis() && (v.stage.is_some() || !v.refers_to.is_empty()) {
            errors.push(format!(
                "{}: an extension sets no stage or reference; the article that establishes '{name}' does that",
                wa.reference
            ));
        }
        if p.via == Via::Extends && (v.type_.is_some() || v.subtype.is_some()) {
            errors.push(format!(
                "{}: an extension by name sets no type or subtype; the article that establishes '{name}' or a hook on its stage does that",
                wa.reference
            ));
        }
        if let Some(q) = v.effective_at.as_ref().and_then(|o| o.parameter.as_ref()) {
            if !parameter_types(wa.article).contains_key(q) {
                errors.push(format!(
                    "{}: effective_at.parameter '{q}' is not a parameter of this article",
                    wa.reference
                ));
            }
        }
    }

    // Type and subtype: from the establishing article, or from a hook on the
    // stage (Awb 4:1: an application is a submission of subtype aanvraag).
    let mut type_: Option<(String, &str)> = None;
    let mut subtype: Option<(String, &str)> = None;
    for p in &parts {
        for (slot, value) in [
            (&mut type_, &p.establishment.type_),
            (&mut subtype, &p.establishment.subtype),
        ] {
            let Some(value) = value else { continue };
            match slot {
                None => *slot = Some((value.clone(), &p.law.reference)),
                Some((earlier, by)) if earlier != value => errors.push(format!(
                    "{} says '{value}', {by} '{earlier}'",
                    p.law.reference
                )),
                Some(_) => {}
            }
        }
    }
    // An article that establishes a submission gives its type (RFC-046): an
    // application is a submission of subtype aanvraag.
    if let (None, Some(kind)) = (&type_, &submission) {
        type_ = Some(("submission".to_string(), &bwa.reference));
        if subtype.is_none() {
            subtype = Some((kind.to_lowercase(), &bwa.reference));
        }
    }
    let Some((type_, _)) = type_ else {
        errors.push(format!(
            "{}: establishes '{name}' without a type, and no hook on its stage gives one",
            bwa.reference
        ));
        return Err(errors);
    };

    // The fields, per part; a field declared twice keeps both legal bases.
    let mut legal_basis: Vec<String> = Vec::new();
    let mut defs: Vec<FieldDef> = Vec::new();
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    let mut prefill: BTreeMap<String, Prefill> = BTreeMap::new();
    let mut moments: Vec<(&EffectiveAtLaw, &str)> = Vec::new();
    for p in &parts {
        push_new(&mut legal_basis, &p.legal_basis());
        match part_fields(service, p, stage.as_deref(), None, date) {
            Ok(fs) => {
                for f in fs {
                    push_new(&mut legal_basis, &f.legal_basis);
                    match defs.iter_mut().find(|d| d.name == f.name) {
                        Some(d) => {
                            push_new(&mut d.legal_basis, &f.legal_basis);
                            if d.type_.is_none() {
                                d.type_ = f.type_;
                                d.unit = f.unit;
                            }
                        }
                        None => defs.push(f),
                    }
                }
            }
            Err(f) => errors.push(f),
        }
        aliases.extend(p.establishment.aliases.clone());
        for (field, w) in &p.establishment.prefill {
            let mut g = p.legal_basis();
            if g.is_empty() {
                g.push(p.law.reference.clone());
            }
            prefill.insert(
                field.clone(),
                Prefill {
                    regulation: w
                        .regulation
                        .clone()
                        .unwrap_or_else(|| p.law.regulation.id.clone()),
                    output: w.output.clone(),
                    legal_basis: g,
                },
            );
        }
        if let Some(m) = &p.establishment.effective_at {
            moments.push((m, &p.law.reference));
        }
    }
    // The origin in force: the policy that takes part in the event may
    // override the origin a field has in the law (RFC-043 `origins`).
    let overrides = origins_of(&parts);
    for d in &mut defs {
        let regulation = d
            .declared_by
            .split_once('#')
            .map_or("", |(r, _)| r)
            .to_string();
        if let Some((o, by)) = overrides.get(&(regulation, d.name.clone())) {
            push_new(&mut d.legal_basis, std::slice::from_ref(&o.grondslag));
            push_new(&mut legal_basis, std::slice::from_ref(&o.grondslag));
            d.origin = Some(o.clone());
            d.origin_policy = Some(by.clone());
        }
    }
    // One question, two legal bases: an alias between two fields of the
    // event makes them one field, and a reader of either name reads it.
    for (reader, field) in &aliases {
        let Some(k) = defs.iter().position(|d| &d.name == reader) else {
            continue;
        };
        if !defs.iter().any(|d| &d.name == field) {
            continue;
        }
        let gone = defs.remove(k);
        if let Some(d) = defs.iter_mut().find(|d| &d.name == field) {
            push_new(&mut d.legal_basis, &gone.legal_basis);
            d.optional = d.optional && gone.optional;
        }
    }
    for (field, w) in &prefill {
        if !defs.iter().any(|d| &d.name == field) {
            errors.push(format!(
                "prefill names '{field}', which is not a field of the event"
            ));
        }
        if service
            .resolver()
            .get_article_by_output(&w.regulation, &w.output, date)
            .is_none()
        {
            errors.push(format!(
                "prefill of '{field}': regulation '{}' has no output '{}'",
                w.regulation, w.output
            ));
        }
    }
    let mut moment_legal_basis: Vec<String> = Vec::new();
    let mut recorded_legal_basis: Vec<String> = Vec::new();
    let mut moment_of: Option<(String, &str)> = None;
    for (m, by) in &moments {
        match m.only {
            None => {
                push_new(&mut moment_legal_basis, &m.legal_basis);
                push_new(&mut recorded_legal_basis, &m.legal_basis);
            }
            Some(MomentSource::Stated) => push_new(&mut moment_legal_basis, &m.legal_basis),
            Some(MomentSource::Recorded) => push_new(&mut recorded_legal_basis, &m.legal_basis),
        }
        let data = m.parameter.clone().or_else(|| m.field.clone());
        if let Some(d) = data {
            match &moment_of {
                Some((earlier, other)) if *earlier != d => errors.push(format!(
                    "{by} names '{d}' as the moment, {other} '{earlier}'"
                )),
                Some(_) => {}
                None => moment_of = Some((d, by)),
            }
        }
    }
    // Only a recorded-moment basis that adds something to the stated one
    // says anything of its own.
    if recorded_legal_basis == moment_legal_basis
        || !moments
            .iter()
            .any(|(m, _)| m.only == Some(MomentSource::Recorded))
    {
        recorded_legal_basis.clear();
    }
    for x in legal_basis
        .iter()
        .chain(&moment_legal_basis)
        .chain(&recorded_legal_basis)
    {
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
    let derived = event.fields.is_empty();
    if derived {
        // The bindings follow from the law: the stream holds only the
        // registration.
        let mut fields = serde_yaml_ng::Mapping::new();
        for d in &defs {
            match binding_of(d, &decisions, prefill.contains_key(&d.name)) {
                Ok(b) => set_yaml(&mut fields, &d.name, b),
                Err(f) => errors.push(f),
            }
        }
        event.fields = fields;
    } else {
        // A stream that still names its fields: every leaf falls under a
        // field of the law, and every field of the law has a leaf.
        let leaves: Vec<String> = event.leaves().into_iter().map(|b| b.path).collect();
        let under = |leaf: &str, path: &str| leaf == path || leaf.starts_with(&format!("{path}."));
        for b in &leaves {
            if !defs.iter().any(|d| under(b, &d.name)) {
                errors.push(format!(
                    "field '{b}' is in the stream, but no article in establishes declares it"
                ));
            }
        }
        for d in &defs {
            if !leaves.iter().any(|b| under(b, &d.name)) {
                errors.push(format!(
                    "{} declares field '{}', but the stream does not bind it",
                    d.declared_by, d.name
                ));
            }
        }
    }
    let has_moment = !moment_legal_basis.is_empty() || !recorded_legal_basis.is_empty();
    match (&mut event.effective_at, has_moment) {
        (Some(o), true) => {
            o.legal_basis = moment_legal_basis.clone();
            o.legal_basis_recorded = recorded_legal_basis.clone();
        }
        (Some(_), false) => errors.push(
            "the stream binds effective_at, but the law does not say why that moment counts in law (effective_at.legal_basis)"
                .into(),
        ),
        (None, true) => {
            // The moment binds to its data item as that is bound (a
            // parameter or a field of the event), otherwise to what the
            // receiving channel states: the counter gives the day of receipt,
            // the portal gives none and the recording counts.
            let source = match &moment_of {
                Some((d, _)) => match event.leaves().into_iter().find(|b| &b.path == d) {
                    Some(b) => match b.binding {
                        crate::stream::Binding::Supplied(s) => format!("{SUPPLIED_BINDING}{s}"),
                        crate::stream::Binding::Intake(s) => format!("$intake.{s}"),
                        crate::stream::Binding::External(s) => format!("$external.{s}"),
                        _ => format!("$external.{d}"),
                    },
                    None => format!("$external.{d}"),
                },
                None => format!("$intake.{RECEIVED_AT}"),
            };
            event.effective_at = Some(EffectiveAtBinding {
                source,
                legal_basis: moment_legal_basis.clone(),
                legal_basis_recorded: recorded_legal_basis.clone(),
            });
        }
        (None, false) => {}
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut establishes: Vec<String> = vec![bwa.reference.clone()];
    for p in &parts {
        push_new(&mut establishes, std::slice::from_ref(&p.law.reference));
    }
    event.type_ = type_;
    event.subtype = subtype.map(|(s, _)| s);
    event.stage = stage;
    event.refers_to = bv.refers_to.clone();
    event.decided_by = decisions.clone();
    event.establishes = establishes;
    event.field_types = defs
        .iter()
        .filter_map(|d| {
            let t = serde_json::from_value(Value::String(d.type_.clone()?)).ok()?;
            Some((
                d.name.clone(),
                FieldType {
                    type_: t,
                    unit: d.unit.clone(),
                    columns: d.columns.clone(),
                    origin: d.origin.clone(),
                },
            ))
        })
        .collect();
    event.legal_basis = legal_basis;
    event.field_legal_basis = defs
        .iter()
        .map(|d| (d.name.clone(), d.legal_basis.clone()))
        .collect();
    event.aliases = aliases;
    event.prefill = prefill;
    event.field_defs = defs;

    // The document (`GET /api/stream`) shows the event as it applies, with
    // per field where it comes from in the law.
    if let Some(e) = stream
        .document
        .get_mut("events")
        .and_then(|e| e.get_mut(i))
        .and_then(Value::as_object_mut)
    {
        let event = &stream.events[i];
        e.insert("establishes".into(), serde_json::json!(event.establishes));
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
        if !event.decided_by.is_empty() {
            e.insert("decided_by".into(), serde_json::json!(event.decided_by));
        }
        if derived && !event.fields.is_empty() {
            e.insert(
                "fields".into(),
                serde_json::to_value(&event.fields).unwrap_or(Value::Null),
            );
        }
        if let Some(b) = &event.effective_at {
            let mut o = Map::new();
            o.insert("source".into(), Value::String(b.source.clone()));
            if !b.legal_basis.is_empty() {
                o.insert("legal_basis".into(), serde_json::json!(b.legal_basis));
            }
            e.insert("effective_at".into(), Value::Object(o));
        }
        if !event.field_defs.is_empty() {
            e.insert(
                "field_sources".into(),
                serde_json::to_value(&event.field_defs).unwrap_or(Value::Null),
            );
        }
        if !event.prefill.is_empty() {
            e.insert(
                "prefill".into(),
                serde_json::to_value(&event.prefill).unwrap_or(Value::Null),
            );
        }
    }
    Ok(())
}

/// The path under `$intake` where the receiving channel states the moment of
/// receipt, if it does (the counter: the day of the date stamp).
pub const RECEIVED_AT: &str = "received_at";

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
    let law = articles(service, None)?;
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
        assert_eq!(establish(&mut streams, &s, None), Vec::<String>::new());
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
        assert_eq!(establish(&mut streams, &s, None), Vec::<String>::new());
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
        let f = establish(&mut streams, &s, None);
        assert!(f.iter().any(|f| f.contains("field 'notitie'")), "{f:?}");
        // And the other way around: an output the stream does not bind.
        let mut streams = vec![stream("notitie: $external.notitie")];
        let f = establish(&mut streams, &s, None);
        assert!(f.iter().any(|f| f.contains("'bedrag_art1'")), "{f:?}");
    }

    #[test]
    fn the_reading_article_becomes_a_lexostatus_the_engine_reads_the_same_way() {
        let s = service();
        let mut streams = vec![stream("bedrag_art1: $external.bedrag_art1")];
        assert!(establish(&mut streams, &s, None).is_empty());
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
        assert!(establish(&mut streams, &s, None).is_empty());
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
        assert!(establish(&mut streams, &s, None).is_empty());
        crate::stream::derive_roles(&mut streams);
        let a: LawSupplement = serde_json::from_value(json!({
            "article": "testwet_lezing#1",
            "extra_fields": {"x": {"field": "bedrag_art1"}}
        }))
        .unwrap();
        let f = lexostatuses(&streams, &s, &[a]).unwrap_err();
        assert!(f[0].contains("testwet_lezing#1"), "{f:?}");
    }

    /// A fictitious general law that applies itself to every application for
    /// a decision (a hook on the stage AANVRAAG), a specific law that
    /// establishes two applications (one for a decision, one for a
    /// regulation of general scope) and a policy that makes the channel
    /// supply the signature.
    const GENERAL: &str = r#"
$id: testwet_algemeen
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.com/testwet_algemeen
procedure:
  - id: beschikking
    default: true
    applies_to: {legal_character: BESCHIKKING}
    stages: [{name: AANVRAAG}, {name: BESLUIT}]
articles:
  - number: '1'
    text: "1. De aanvraag wordt ondertekend en bevat de naam van de aanvrager en de gevraagde beschikking.\n\n2. De aanvraag telt vanaf de ontvangst."
    url: https://example.com/testwet_algemeen/1
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {submission: AANVRAAG, decided_by: BESCHIKKING}
      execution:
        produces:
          legal_character: TOETS
          decision_type: GEEN_BESLUIT
          extensions:
            chronolex:
              establishes:
                - extends: {submission: AANVRAAG}
                  fields: parameters
                  effective_at: {legal_basis: ['testwet_algemeen#1 lid 2']}
        parameters:
          - {name: naam_aanvrager, type: string, required: false, origin: {waarde: BELANGHEBBENDE, grondslag: 'testwet_algemeen#1 lid 1'}}
          - {name: gevraagde_beschikking, type: string, required: false, origin: {waarde: BELANGHEBBENDE, grondslag: 'testwet_algemeen#1 lid 1', rol: GEVRAAGD_BESLUIT}}
          - {name: ondertekening, type: string, required: false, origin: {waarde: BELANGHEBBENDE, grondslag: 'testwet_algemeen#1 lid 1'}}
          - {name: interne_notitie, type: string, required: false}
"#;

    const SPECIFIC: &str = r#"
$id: testwet_bijzonder
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.com/testwet_bijzonder
articles:
  - number: '1'
    text: De vereniging kan een bijdrage aanvragen; de aanvraag bevat haar statutaire naam.
    url: https://example.com/testwet_bijzonder/1
    machine_readable:
      execution:
        produces:
          legal_character: TOETS
          decision_type: GEEN_BESLUIT
          submission: {kind: AANVRAAG}
          extensions:
            chronolex:
              establishes:
                - event: bijdrage_aangevraagd
                  fields: {parameters: [statutaire_naam]}
                  aliases: {naam_aanvrager: statutaire_naam}
        parameters:
          - {name: statutaire_naam, type: string, required: false, origin: {waarde: BELANGHEBBENDE, grondslag: 'testwet_bijzonder#1'}}
  - number: '2'
    text: De instantie besluit op de aanvraag.
    url: https://example.com/testwet_bijzonder/2
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING, decides_on: ['testwet_bijzonder#1']}
        output: [{name: bijdrage, type: number}]
        actions: [{output: bijdrage, value: 100}]
  - number: '3'
    text: Ieder kan de instantie verzoeken een regeling vast te stellen.
    url: https://example.com/testwet_bijzonder/3
    machine_readable:
      execution:
        produces:
          legal_character: TOETS
          decision_type: GEEN_BESLUIT
          submission: {kind: AANVRAAG}
          extensions:
            chronolex:
              establishes:
                - event: regeling_verzocht
                  subtype: verzoek
                  fields: [onderwerp]
  - number: '4'
    text: De instantie stelt de regeling vast.
    url: https://example.com/testwet_bijzonder/4
    machine_readable:
      execution:
        produces: {legal_character: BESLUIT_VAN_ALGEMENE_STREKKING, decision_type: ALGEMEEN_VERBINDEND_VOORSCHRIFT, decides_on: ['testwet_bijzonder#3']}
        output: [{name: vastgesteld, type: boolean}]
        actions: [{output: vastgesteld, value: true}]
"#;

    const POLICY: &str = r#"
$id: testbeleid_haak
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.com/testbeleid_haak
competent_authority: {name: De instantie}
articles:
  - number: '1'
    text: Via het portaal ondertekent wie inlogt, en legt het portaal het nummer van de vereniging vast.
    url: https://example.com/testbeleid_haak/1
    machine_readable:
      origins:
        - regulation: testwet_algemeen
          parameter: ondertekening
          origin: {waarde: KANAAL, grondslag: 'testbeleid_haak#1'}
      execution:
        produces:
          legal_character: TOETS
          decision_type: GEEN_BESLUIT
          extensions:
            chronolex:
              establishes:
                - extends: bijdrage_aangevraagd
                  fields:
                    nummer: {type: string, origin: {waarde: KANAAL, grondslag: 'testbeleid_haak#1'}}
"#;

    fn composed(general: &str) -> (Vec<Stream>, Vec<String>) {
        let mut s = LawExecutionService::new();
        for t in [general, SPECIFIC, POLICY] {
            s.load_law(t).unwrap();
        }
        let mut streams = vec![crate::stream::parse(
            "$id: test_bijdragen\nrecording_actor: test_instantie\nchronicle: test_kroniek\nevents:\n  - {name: bijdrage_aangevraagd, establishes: 'testwet_bijzonder#1', intake: portaal}\n  - {name: regeling_verzocht, establishes: 'testwet_bijzonder#3', intake: portaal}\n",
            "test",
        )
        .unwrap()];
        let errors = establish(&mut streams, &s, None);
        (streams, errors)
    }

    fn binding(e: &crate::stream::Event, field: &str) -> Option<crate::stream::Binding> {
        e.leaves()
            .into_iter()
            .find(|b| b.path == field)
            .map(|b| b.binding)
    }

    /// The gram of an application is the sum of what the articles say about
    /// it: the specific law establishes it and names the decision it asks
    /// for; the general law hooks onto the stage of that decision and gives
    /// the type, its fields and the moment; the policy that takes part
    /// makes the channel supply the signature. The stream names none of it.
    #[test]
    fn the_general_law_applies_itself_to_an_application() {
        use crate::stream::Binding;
        let (streams, errors) = composed(GENERAL);
        assert_eq!(errors, Vec::<String>::new());
        let e = &streams[0].events[0];
        assert_eq!(e.type_, "submission");
        assert_eq!(e.subtype.as_deref(), Some("aanvraag"));
        assert_eq!(e.stage.as_deref(), Some("AANVRAAG"));
        assert_eq!(
            e.establishes,
            [
                "testwet_bijzonder#1",
                "testwet_algemeen#1",
                "testbeleid_haak#1"
            ]
        );
        // A field is input by default ...
        assert_eq!(
            binding(e, "statutaire_naam"),
            Some(Binding::External("statutaire_naam".into()))
        );
        // ... the decision requested is fixed by what the law requests ...
        assert_eq!(
            binding(e, "gevraagde_beschikking"),
            Some(Binding::Constant(json!("testwet_bijzonder#2")))
        );
        // ... and what the channel supplies, by the origin in force (the
        // policy overrides the law) or in the law itself, binds to $supplied.
        assert_eq!(
            binding(e, "ondertekening"),
            Some(Binding::Supplied("ondertekening".into()))
        );
        assert_eq!(
            binding(e, "nummer"),
            Some(Binding::Supplied("nummer".into()))
        );
        // A parameter without origin is no field; one question, two legal
        // bases: the name of the applicant is the statutory name.
        assert_eq!(binding(e, "interne_notitie"), None);
        assert_eq!(binding(e, "naam_aanvrager"), None);
        assert_eq!(
            e.field_legal_basis["statutaire_naam"],
            ["testwet_bijzonder#1", "testwet_algemeen#1 lid 1"]
        );
        assert_eq!(e.aliases["naam_aanvrager"], "statutaire_naam");
        // The moment is the receipt: the counter states it, otherwise the
        // recording counts.
        let m = e.effective_at.as_ref().unwrap();
        assert_eq!(m.source, "$intake.received_at");
        assert_eq!(m.legal_basis, ["testwet_algemeen#1 lid 2"]);
        // The document of the stream says per field where it comes from.
        let sources = streams[0].document["events"][0]["field_sources"]
            .as_array()
            .unwrap();
        let via: Vec<(&str, &str)> = sources
            .iter()
            .map(|f| (f["name"].as_str().unwrap(), f["via"].as_str().unwrap()))
            .collect();
        assert!(via.contains(&("ondertekening", "hook")), "{via:?}");
        assert!(via.contains(&("nummer", "extends")), "{via:?}");
        assert!(via.contains(&("statutaire_naam", "establishes")), "{via:?}");
    }

    /// Awb 4:2 applies to an application for a decision (beschikking), not
    /// to a request for a regulation of general scope: that decision has no
    /// procedure with a stage AANVRAAG, so no hook applies, and the request
    /// gets only what its own article gives it.
    #[test]
    fn a_request_for_a_regulation_of_general_scope_gets_no_awb_4_2() {
        let (streams, errors) = composed(GENERAL);
        assert_eq!(errors, Vec::<String>::new());
        let e = &streams[0].events[1];
        assert_eq!(e.stage, None);
        assert_eq!(e.subtype.as_deref(), Some("verzoek"));
        assert_eq!(e.establishes, ["testwet_bijzonder#3"]);
        let fields: Vec<String> = e.leaves().into_iter().map(|b| b.path).collect();
        assert_eq!(fields, ["onderwerp"]);
    }

    /// A field added to the general law reaches every application without a
    /// change to the stream (the version that applies on the date).
    #[test]
    fn a_new_field_in_the_general_law_reaches_the_application() {
        let general = GENERAL.replace(
            "          - {name: interne_notitie",
            "          - {name: telefoon_aanvrager, type: string, required: false, origin: {waarde: BELANGHEBBENDE, grondslag: 'testwet_algemeen#1 lid 1'}}\n          - {name: interne_notitie",
        );
        assert_ne!(general, GENERAL);
        let (streams, errors) = composed(&general);
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(
            binding(&streams[0].events[0], "telefoon_aanvrager"),
            Some(crate::stream::Binding::External(
                "telefoon_aanvrager".into()
            ))
        );
        assert_eq!(binding(&streams[0].events[1], "telefoon_aanvrager"), None);
    }

    /// An article that extends a stage says so with a hook on that stage; an
    /// extension by name sets no type.
    #[test]
    fn an_extension_of_a_stage_needs_a_hook() {
        let without_hook = GENERAL.replace(
            "      hooks:\n        - hook_point: pre_actions\n          applies_to: {submission: AANVRAAG, decided_by: BESCHIKKING}\n",
            "",
        );
        assert_ne!(without_hook, GENERAL);
        let (_, errors) = composed(&without_hook);
        assert!(
            errors
                .iter()
                .any(|f| f.contains("declares no hook on a submission AANVRAAG")),
            "{errors:?}"
        );
    }
}

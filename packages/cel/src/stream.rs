//! The stream definition: which facts the cell records, and building a gram
//! from what comes in.
//!
//! A field of an event binds to `$intake.<path>` (who and by which route: the
//! receiving channel), to `$external.<path>` (the content as submitted), to
//! `$supplied.<field>` (what the receiving channel supplies, such as the login
//! or a register the portal queried, and otherwise the content as submitted),
//! or is a constant of the stream. Fields may be nested, and an `$external`
//! value may feed more than one field. A table field
//! (`{table: $external.<path>, columns: [...]}`) declares its columns.
//!
//! An event that names its establishing article (`establishes`) holds only
//! the registration; its fields and their bindings come from the law
//! ([`crate::law`], note "het gram uit de wet", 29-09-2026): a field is input
//! by default, and only an origin makes it otherwise.
//!
//! What a submission passes under `external` must fit the shape the stream
//! declares ([`Shape`]): an unknown field or an unknown column is refused,
//! with the field path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::date;
use crate::gram::{at_path, set_path, Gram, StreamReference};
use crate::load;
use crate::schema::Kind;

/// The gram type of a decision (RFC-022).
pub const DECRETOGRAM: &str = "decretogram";
/// The gram type of a delivery or settlement that executes a decision.
pub const EXECUTOGRAM: &str = "executogram";

/// A loaded stream definition.
#[derive(Debug, Clone)]
pub struct Stream {
    pub id: String,
    pub recording_actor: String,
    pub chronicle: String,
    pub events: Vec<Event>,
    /// SHA-256 of the file as read; goes along in every gram.
    pub sha256: String,
    /// The document itself, for `GET /api/stream`.
    pub document: Value,
    /// The file it was loaded from; `None` for a stream parsed from text.
    pub file: Option<PathBuf>,
}

/// An event from a stream.
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub name: String,
    pub intake: String,
    /// The articles that establish this event (`produces.extensions.chronolex`
    /// in the law, see [`crate::law`]). Then `type`, `subtype`, `stage`,
    /// `refers_to`, `legal_basis` and the legal basis of `effective_at` come
    /// from the law; the runtime fills them in when loading the cell, before
    /// anything else reads the event. In the stream one article (the one that
    /// establishes the event) or a list; after loading, every article that
    /// takes part: the one that establishes it, the ones that extend it and
    /// the ones that hook onto its stage.
    #[serde(default, deserialize_with = "one_or_many")]
    pub establishes: Vec<String>,
    /// For a submission (RFC-046): the decisions taken on it, the articles
    /// that name its establishing article in `produces.decides_on` (Wpp 107
    /// "besluit op de aanvraag"). Filled in from the law; not in the YAML.
    #[serde(skip)]
    pub decided_by: Vec<String>,
    #[serde(default)]
    pub legal_basis: Vec<String>,
    #[serde(rename = "type", default)]
    pub type_: String,
    #[serde(default)]
    pub subtype: Option<String>,
    /// For a stage decretogram: the stage of the decision (RFC-008).
    #[serde(default)]
    pub stage: Option<String>,
    /// Which grams a gram of this event refers to, per name from the law
    /// text, and what each may point to (see [`Reference`]).
    #[serde(default)]
    pub refers_to: BTreeMap<String, Reference>,
    /// Derived when loading the cell (see [`derive_roles`]): whether a gram of
    /// this event opens a group (a root that others refer to), belongs to a
    /// group, or stands alone. Not in the YAML.
    #[serde(skip)]
    pub case: Case,
    /// Derived when loading the cell (see [`derive_roles`]): whether a gram of
    /// this event is a decision, follows a decision or amends a decision. Not
    /// in the YAML.
    #[serde(skip)]
    pub decision: Option<Decision>,
    /// What the `effective_at` of the gram binds to, if it is not the moment
    /// of recording.
    #[serde(default)]
    pub effective_at: Option<EffectiveAtBinding>,
    /// The field tree, in document order (a YAML mapping preserves it). With
    /// `establishes` the runtime derives it from the law; the stream then
    /// leaves it out.
    #[serde(default)]
    pub fields: serde_yaml_ng::Mapping,
    #[serde(default)]
    pub not_reduced: Vec<NotReduced>,
    /// The name bridge from the law (see [`crate::law::Establishment::aliases`]):
    /// `<name at the reader>: <field of this event>`.
    #[serde(skip)]
    pub aliases: BTreeMap<String, String>,
    /// The type of a field as the law names it (`fields: {bedrag: {type:
    /// amount, unit: eurocent}}` in `establishes`): the form of a fact adopts
    /// it when no reading reads the field.
    #[serde(skip)]
    pub field_types: BTreeMap<String, crate::law::FieldType>,
    /// The legal basis per field path, from the establishment or extension
    /// that declares the path (BW 2:318 lid 1 for the date of a fusion deed,
    /// lid 2 for the notarial statement). `legal_basis` is the union over the
    /// whole event; the form of a fact shows this one per field.
    #[serde(skip)]
    pub field_legal_basis: BTreeMap<String, Vec<String>>,
    /// Per field, how the law declares it (see [`crate::law::FieldDef`]): the
    /// article, whether by establishing, extending or a hook, the type and
    /// the origin in force. Empty for an event without `establishes`.
    #[serde(skip)]
    pub field_defs: Vec<crate::law::FieldDef>,
    /// Why the event looks the way it does ([`crate::law::Explanation`]);
    /// filled in by [`crate::law::establish`], empty without `establishes`.
    #[serde(skip)]
    pub explanation: crate::law::Explanation,
    /// The fields a register may fill in beforehand, per field the policy
    /// output that knows it (see [`crate::law::Prefill`]).
    #[serde(skip)]
    pub prefill: BTreeMap<String, crate::law::Prefill>,
}

/// One text or a list of texts, such as `establishes` or a legal basis.
pub(crate) fn one_or_many<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        One(String),
        Many(Vec<String>),
    }
    Ok(match Raw::deserialize(d)? {
        Raw::One(s) => vec![s],
        Raw::Many(v) => v,
    })
}

/// A reference of an event: what the gram that a gram of this event refers
/// to must be (`to`), and whether the process must pass it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub to: To,
    #[serde(default)]
    pub required: bool,
}

/// What a gram must be to be the target of a reference.
#[derive(Debug, Clone, PartialEq)]
pub enum To {
    /// Established by this article (`<regulation>#<article>`): the event of
    /// the target names it in `establishes`, or the gram carries it as its
    /// legal basis.
    Article(String),
    /// A gram of this event.
    Event(String),
    /// A gram with this stage.
    Stage(String),
}

impl<'de> Deserialize<'de> for To {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            Stage { stage: String },
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Text(t) if t.contains('#') => To::Article(t),
            Raw::Text(t) => To::Event(t),
            Raw::Stage { stage } => To::Stage(stage),
        })
    }
}

impl std::fmt::Display for To {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            To::Article(a) => write!(f, "established by {a}"),
            To::Event(e) => write!(f, "a gram '{e}'"),
            To::Stage(s) => write!(f, "a gram with stage {s}"),
        }
    }
}

impl To {
    /// Whether a gram of `event` (in the cell with these streams) can be the
    /// target. For an article: the event names it in `establishes`, or its
    /// legal basis starts with it.
    pub fn fits_event(&self, event: &Event) -> bool {
        match self {
            To::Article(a) => {
                event.establishes.contains(a)
                    || event
                        .legal_basis
                        .iter()
                        .any(|g| g == a || g.starts_with(&format!("{a} ")))
            }
            To::Event(e) => event.name == *e,
            To::Stage(s) => event.stage.as_deref() == Some(s),
        }
    }

    /// Whether `gram` can be the target; `event` is the event of the gram, if
    /// the cell knows it.
    pub fn fits(&self, gram: &Gram, event: Option<&Event>) -> bool {
        match self {
            To::Article(a) => {
                event.is_some_and(|e| e.establishes.contains(a))
                    || gram
                        .legal_basis
                        .iter()
                        .any(|g| g == a || g.starts_with(&format!("{a} ")))
            }
            To::Event(e) => gram.name == *e,
            To::Stage(s) => gram.stage.as_deref() == Some(s),
        }
    }
}

/// The role of an event with respect to a group, derived from the references
/// (see [`derive_roles`]). There is no case in the gram: this only says
/// whether a gram of the event is a root that other grams refer to (`Opens`,
/// like the application), refers to another gram (`Follows`), or stands
/// alone (`Standalone`, like a register fact that nobody follows).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Case {
    /// A gram of this event is a root that other events refer to.
    Opens,
    /// A gram of this event refers to another gram.
    Follows,
    /// Neither.
    #[default]
    Standalone,
}

impl Case {
    /// Whether a gram of this event belongs to a group.
    pub fn has_attribute(self) -> bool {
        self != Case::Standalone
    }

    pub fn as_text(self) -> &'static str {
        match self {
            Case::Opens => "opens",
            Case::Follows => "follows",
            Case::Standalone => "standalone",
        }
    }
}

/// The role of an event with respect to a decision, derived from stage and
/// references (see [`derive_roles`]). A decision is the state container of
/// RFC-008; the stage grams of a decision refer to it (RFC-022 par. 1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// The gram is a decision (stage BESLUIT, without `amends`). The cell
    /// refuses a second decision of the same event that refers to the same
    /// gram: another decision on this requires its own legal basis.
    Opens,
    /// The gram refers to a decision, such as the publication or a payment
    /// that executes it (RFC-022 par. 3.3 `references_decision`).
    Follows,
    /// The gram is a decision that refers to another decision with `amends`
    /// (RFC-022 par. 3.1, Awb 4:48 and 4:49).
    Amends,
}

impl Decision {
    pub fn as_text(self) -> &'static str {
        match self {
            Decision::Opens => "opens",
            Decision::Follows => "follows",
            Decision::Amends => "amends",
        }
    }

    /// Whether a gram of this event is itself a decision.
    pub fn is_decision(self) -> bool {
        self != Decision::Follows
    }
}

/// The stage of a decision (RFC-008).
pub const DECISION: &str = "BESLUIT";
/// The name of that stage is in the code, not in the procedure of the Awb.
pub const DECISION_CODE: crate::code_ref::CodeRef = crate::code_ref!(
    "De fase van een besluit heet hier BESLUIT; die naam staat in de code, niet in de procedure van de Awb-YAML."
);

/// The name of the reference with which a decision amends another decision.
pub const AMENDS: &str = "amends";

/// Derive the roles per event of a cell (see [`Case`] and [`Decision`]),
/// from the stages and the references of all events of the cell. An event
/// with stage BESLUIT is a decision, with an `amends` reference an
/// amendment; another event follows a decision if one of its references can
/// only point to a decision. An event refers to a group if it has
/// references, and opens one if another event can refer to a gram of this
/// event and it refers to nothing itself.
pub fn derive_roles(streams: &mut [Stream]) {
    let events: Vec<Event> = streams.iter().flat_map(|s| s.events.clone()).collect();
    let is_decision = |e: &Event| e.stage.as_deref() == Some(DECISION);
    for s in streams.iter_mut() {
        for e in &mut s.events {
            e.decision = if is_decision(e) {
                Some(if e.refers_to.contains_key(AMENDS) {
                    Decision::Amends
                } else {
                    Decision::Opens
                })
            } else if e.refers_to.values().any(|v| {
                let targets: Vec<&Event> = events.iter().filter(|d| v.to.fits_event(d)).collect();
                !targets.is_empty() && targets.iter().all(|d| is_decision(d))
            }) {
                Some(Decision::Follows)
            } else {
                None
            };
            let becomes_followed = events
                .iter()
                .any(|a| a.refers_to.values().any(|v| v.to.fits_event(e)));
            e.case = if !e.refers_to.is_empty() {
                Case::Follows
            } else if becomes_followed {
                Case::Opens
            } else {
                Case::Standalone
            };
        }
    }
}

/// Check the references of the events of a cell: every reference can point
/// to an event of the cell.
pub fn check_references(streams: &[Stream]) -> Vec<String> {
    let events: Vec<&Event> = streams.iter().flat_map(|s| s.events.iter()).collect();
    let mut errors = Vec::new();
    for s in streams {
        for e in &s.events {
            for (name, v) in &e.refers_to {
                if !events.iter().any(|d| v.to.fits_event(d)) {
                    errors.push(format!(
                        "stream '{}', event '{}': reference '{name}' points to {}, but no event of the cell fits",
                        s.id, e.name, v.to
                    ));
                }
            }
        }
    }
    errors
}

/// What a key of the gram itself is for an event (see [`Event::attribute`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventAttribute<'a> {
    /// Fixed in the stream; `None` if a gram of this event does not have it.
    Fixed(Option<&'a str>),
    /// Depends on the gram, such as the id, the root or a reference.
    Free,
    /// A gram of this event never has it.
    Never,
}

/// The `effective_at` of an event bound to a submitted value, with the legal
/// basis for why that moment counts in law. Example: the day a paper
/// application came in (Awb 4:13: the deadline runs from receipt), recorded
/// on a later day.
#[derive(Debug, Clone, Deserialize)]
pub struct EffectiveAtBinding {
    /// `$intake.<path>` or `$external.<path>`.
    pub source: String,
    /// From the law if the event has `establishes`: why the stated moment
    /// counts.
    #[serde(default)]
    pub legal_basis: Vec<String>,
    /// From the law: why the moment of recording counts when nothing states
    /// another (a portal application is received when the portal records it).
    /// Empty: the gram then carries no legal basis for its moment.
    #[serde(skip)]
    pub legal_basis_recorded: Vec<String>,
}

impl EffectiveAtBinding {
    /// The source as a [`Binding`]: the schema allows only `$intake`,
    /// `$supplied` and `$external`, and the law derives only those. Any other
    /// source binds nothing ([`Binding::Constant`] null): the moment is then
    /// the recording.
    pub fn binding(&self) -> Binding {
        if let Some(r) = self.source.strip_prefix("$intake.") {
            return Binding::Intake(r.to_string());
        }
        if let Some(r) = self.source.strip_prefix(SUPPLIED_BINDING) {
            return Binding::Supplied(r.to_string());
        }
        match self.source.strip_prefix("$external.") {
            Some(r) => Binding::External(r.to_string()),
            None => Binding::Constant(Value::Null),
        }
    }
}

/// A field that deliberately no derivation reads, with the reason.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotReduced {
    pub field: String,
    pub reason: String,
}

/// Where the value of a field comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    /// `$intake.<path>`: the receiving channel.
    Intake(String),
    /// `$external.<path>`: the content as submitted.
    External(String),
    /// `$supplied.<field>`: what the receiving channel supplies under
    /// `$intake.supplied.<field>` (the login, a register the portal queried),
    /// and otherwise the content as submitted under `<field>`. The gram
    /// records where the value came from.
    Supplied(String),
    /// `{table: $external.<path>, columns: [...]}`: a list of rows with the
    /// declared columns.
    Table {
        source: String,
        columns: Vec<String>,
    },
    /// `{table: $external.<path>}` without columns: a list of rows whose
    /// columns the law does not declare (an `array` parameter; the law
    /// format has no `items` yet). Every row is an object of single values.
    Rows { source: String },
    /// A fixed value of the stream.
    Constant(Value),
}

/// The shape a submission under `external` may have, derived from the
/// `$external` bindings of an event.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// A single value (text, number, yes/no or null).
    Value,
    /// A list of rows with only these columns.
    Table(Vec<String>),
    /// A list of rows with any columns of single values.
    Rows,
    /// An object with only these keys.
    Branch(BTreeMap<String, Shape>),
}

/// A leaf of the field tree: the path in the gram and what it binds to.
#[derive(Debug, Clone, PartialEq)]
pub struct Leaf {
    /// Path in the gram under `fields`, for example `content.organen`.
    pub path: String,
    pub binding: Binding,
}

/// Parse a stream definition from text. `source` names the file in messages.
pub fn parse(text: &str, source: &str) -> Result<Stream, Vec<String>> {
    // The YAML tree preserves the order of the fields; see `Event::fields`.
    let (yaml, document) = load::yaml_document(text, source, Kind::Stream)?;

    #[derive(Deserialize)]
    struct Raw {
        #[serde(rename = "$id")]
        id: String,
        recording_actor: String,
        chronicle: String,
        events: Vec<Event>,
    }
    let raw: Raw = serde_yaml_ng::from_value(yaml).map_err(|e| vec![format!("{source}: {e}")])?;
    let errors: Vec<String> = raw
        .events
        .iter()
        .filter_map(|e| e.external_shape().err())
        .flatten()
        .map(|f| format!("{source}: {f}"))
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Stream {
        id: raw.id,
        recording_actor: raw.recording_actor,
        chronicle: raw.chronicle,
        events: raw.events,
        sha256: hex::encode(Sha256::digest(text.as_bytes())),
        document,
        file: None,
    })
}

/// Load the stream definitions from a file or from all `.yaml` files in a
/// directory.
pub fn load(path: &Path) -> Result<Vec<Stream>, Vec<String>> {
    let files: Vec<PathBuf> = if path.is_dir() {
        load::yaml_files(path).map_err(|e| vec![e])?
    } else {
        vec![path.to_path_buf()]
    };
    let mut streams = Vec::new();
    let mut errors = Vec::new();
    for file in &files {
        match load::load(file, parse) {
            Ok(mut s) => {
                s.file = Some(file.clone());
                streams.push(s)
            }
            Err(f) => errors.extend(f),
        }
    }
    if streams.is_empty() && errors.is_empty() {
        errors.push(format!("{}: no stream definition found", path.display()));
    }
    if errors.is_empty() {
        Ok(streams)
    } else {
        Err(errors)
    }
}

impl Stream {
    /// The event with this name.
    pub fn event(&self, name: &str) -> Option<&Event> {
        self.events.iter().find(|e| e.name == name)
    }
}

/// The type of a gram an interested party submits (RFC-022 par. 1).
pub const SUBMISSION: &str = "submission";

impl Event {
    /// Whether a gram of this event is a submission: `type: submission`. An
    /// event the law establishes as an application gets that type from the
    /// law (RFC-046), and its stage from the procedure in the Awb; the name
    /// of that stage is not the runtime's to know.
    pub fn is_submission(&self) -> bool {
        self.type_ == SUBMISSION
    }

    /// Why a source may supply a field (`$intake.supplied.<name>`): for the
    /// channel the origin the field has in force (the rule that makes the
    /// channel supply it), otherwise what the receiving channel says (the
    /// policy that names the register).
    pub fn supply_legal_basis(&self, name: &str, source: &str, supplied: &Value) -> Vec<String> {
        let of_origin = self
            .field_defs
            .iter()
            .find(|d| d.name == name && source == "channel")
            .and_then(|d| d.origin.as_ref())
            .map(|o| vec![o.legal_basis.clone()]);
        of_origin.unwrap_or_else(|| {
            supplied
                .get("legal_basis")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
    }

    /// The leaves of the field tree, in document order.
    pub fn leaves(&self) -> Vec<Leaf> {
        use serde_yaml_ng::Value as Y;
        fn loop_(prefix: &str, fields: &serde_yaml_ng::Mapping, out: &mut Vec<Leaf>) {
            for (name, value) in fields {
                let Some(name) = name.as_str() else { continue };
                let path = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{prefix}.{name}")
                };
                let binding = match value {
                    // The schema allows `columns` as a list only in a table
                    // field; a group of fields has no lists.
                    Y::Mapping(kind) if kind.get("columns").is_some_and(Y::is_sequence) => {
                        let source = kind
                            .get("table")
                            .and_then(Y::as_str)
                            .and_then(|t| t.strip_prefix("$external."))
                            .unwrap_or_default()
                            .to_string();
                        let columns = kind
                            .get("columns")
                            .and_then(Y::as_sequence)
                            .into_iter()
                            .flatten()
                            .filter_map(Y::as_str)
                            .map(str::to_string)
                            .collect();
                        Binding::Table { source, columns }
                    }
                    Y::Mapping(kind) if kind.get("table").is_some() => {
                        let source = kind
                            .get("table")
                            .and_then(Y::as_str)
                            .and_then(|t| t.strip_prefix("$external."))
                            .unwrap_or_default()
                            .to_string();
                        Binding::Rows { source }
                    }
                    Y::Mapping(kind) => {
                        loop_(&path, kind, out);
                        continue;
                    }
                    Y::String(text) => {
                        if let Some(r) = text.strip_prefix("$intake.") {
                            Binding::Intake(r.to_string())
                        } else if let Some(r) = text.strip_prefix(SUPPLIED_BINDING) {
                            Binding::Supplied(r.to_string())
                        } else if let Some(r) = text.strip_prefix("$external.") {
                            Binding::External(r.to_string())
                        } else {
                            Binding::Constant(Value::String(text.clone()))
                        }
                    }
                    other => Binding::Constant(serde_json::to_value(other).unwrap_or(Value::Null)),
                };
                out.push(Leaf { path, binding });
            }
        }
        let mut out = Vec::new();
        loop_("", &self.fields, &mut out);
        out
    }

    /// The fields of a gram of this event as YAML, in the order of the stream
    /// (a JSON object from the chronicle is alphabetical).
    pub fn ordered(&self, fields: &Map<String, Value>) -> serde_yaml_ng::Mapping {
        fn loop_(
            template: &serde_yaml_ng::Mapping,
            values: &Map<String, Value>,
        ) -> serde_yaml_ng::Mapping {
            let mut out = serde_yaml_ng::Mapping::new();
            for (name, sub) in template {
                let Some(name) = name.as_str() else { continue };
                let Some(value) = values.get(name) else {
                    continue;
                };
                let ordered = match (sub, value) {
                    (serde_yaml_ng::Value::Mapping(s), Value::Object(w)) => {
                        serde_yaml_ng::Value::Mapping(loop_(s, w))
                    }
                    _ => serde_yaml_ng::to_value(value).unwrap_or(serde_yaml_ng::Value::Null),
                };
                out.insert(serde_yaml_ng::Value::String(name.to_string()), ordered);
            }
            out
        }
        loop_(&self.fields, fields)
    }

    /// What a filter key of the gram itself (see
    /// [`crate::reduction::GRAM_KEYS`]) can be for a gram of this event, in
    /// stream `stream`. `None` if `key` is a field path. This way the startup
    /// check and the reduction ([`crate::gram::Gram::attribute`]) read the
    /// same keys.
    pub fn attribute<'a>(&'a self, stream: &'a Stream, key: &str) -> Option<EventAttribute<'a>> {
        let free_as = |can: bool| {
            if can {
                EventAttribute::Free
            } else {
                EventAttribute::Never
            }
        };
        if let Some(name) = key.strip_prefix(crate::gram::REFERS_TO) {
            return Some(free_as(self.refers_to.contains_key(name)));
        }
        Some(match key {
            "id" | "root" => EventAttribute::Free,
            "name" => EventAttribute::Fixed(Some(self.name.as_str())),
            "type" => EventAttribute::Fixed(Some(self.type_.as_str())),
            "subtype" => EventAttribute::Fixed(self.subtype.as_deref()),
            "stage" => EventAttribute::Fixed(self.stage.as_deref()),
            "recording_actor" => EventAttribute::Fixed(Some(stream.recording_actor.as_str())),
            "chronicle" => EventAttribute::Fixed(Some(stream.chronicle.as_str())),
            // The fields of a decision that a process passes.
            "legal_character" | "decision_type" | "regulation" | "competent_authority" => {
                free_as(self.type_ == DECRETOGRAM)
            }
            _ => return None,
        })
    }

    /// Whether a path is a leaf or a branch of the field tree.
    pub fn has_path(&self, path: &str) -> bool {
        self.leaves()
            .iter()
            .any(|b| b.path == path || b.path.starts_with(&format!("{path}.")))
    }

    /// Whether a path is a leaf (a field with its own value, such as a table).
    pub fn has_leaf(&self, path: &str) -> bool {
        self.leaves().iter().any(|b| b.path == path)
    }

    /// The columns of a table field, or `None` if the path is not a table field.
    pub fn columns(&self, path: &str) -> Option<Vec<String>> {
        self.leaves().into_iter().find_map(|b| match b.binding {
            Binding::Table { columns, .. } if b.path == path => Some(columns),
            _ => None,
        })
    }

    /// The `$external` bindings: the source path and the shape of the value,
    /// including that of `effective_at`.
    fn external_sources(&self) -> Vec<(String, Shape)> {
        self.leaves()
            .into_iter()
            .map(|b| b.binding)
            .chain(self.effective_at.as_ref().map(EffectiveAtBinding::binding))
            .filter_map(|b| match b {
                Binding::External(source) | Binding::Supplied(source) => {
                    Some((source, Shape::Value))
                }
                Binding::Table { source, columns } => Some((source, Shape::Table(columns))),
                Binding::Rows { source } => Some((source, Shape::Rows)),
                _ => None,
            })
            .collect()
    }

    /// The names a submission may pass under `external`: the first part of
    /// every `$external` path, in the order of the stream.
    pub fn external_keys(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for (source, _) in self.external_sources() {
            let head = source.split('.').next().unwrap_or_default().to_string();
            if !v.contains(&head) {
                v.push(head);
            }
        }
        v
    }

    /// The shape `external` may have. An error if two bindings give the same
    /// source path a different shape, such as a single value and a table, or
    /// a value and an object with fields below it.
    pub fn external_shape(&self) -> Result<BTreeMap<String, Shape>, Vec<String>> {
        let mut root = BTreeMap::new();
        let mut errors = Vec::new();
        for (source, shape) in self.external_sources() {
            let parts: Vec<&str> = source.split('.').collect();
            if !add_shape_to(&mut root, &parts, shape) {
                errors.push(format!(
                    "event '{}': '$external.{source}' gets more than one shape (value, table or fields below it)",
                    self.name
                ));
            }
        }
        if errors.is_empty() {
            Ok(root)
        } else {
            Err(errors)
        }
    }
}

/// Put a shape on a source path. False if another shape is already there.
fn add_shape_to(branch: &mut BTreeMap<String, Shape>, parts: &[&str], shape: Shape) -> bool {
    match parts {
        [] => true,
        [latest] => {
            let existing = branch.entry((*latest).to_string()).or_insert(shape.clone());
            *existing == shape
        }
        [head, rest @ ..] => match branch
            .entry((*head).to_string())
            .or_insert_with(|| Shape::Branch(BTreeMap::new()))
        {
            Shape::Branch(sub) => add_shape_to(sub, rest, shape),
            _ => false,
        },
    }
}

/// A single value: not a list and not an object.
fn single(w: &Value) -> bool {
    !matches!(w, Value::Array(_) | Value::Object(_))
}

/// Check `external` against the shape of the stream. Yields the field paths
/// the stream does not know, and the messages about values of the wrong
/// shape.
fn assessment_shape(
    fields: &Map<String, Value>,
    shape: &BTreeMap<String, Shape>,
    prefix: &str,
    unknown: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    for (name, value) in fields {
        let path = format!("{prefix}{name}");
        match shape.get(name) {
            None => unknown.push(path),
            Some(Shape::Value) => {
                if !single(value) {
                    errors.push(format!("field '{path}' expects a single value"));
                }
            }
            Some(Shape::Branch(sub)) => match value {
                Value::Null => {}
                Value::Object(m) => assessment_shape(m, sub, &format!("{path}."), unknown, errors),
                _ => errors.push(format!("field '{path}' expects fields below it")),
            },
            Some(Shape::Table(columns)) => match value {
                Value::Null => {}
                Value::Array(rows) => {
                    for (i, row) in rows.iter().enumerate() {
                        let Value::Object(row) = row else {
                            errors.push(format!("row '{path}[{i}]' is not an object with columns"));
                            continue;
                        };
                        for (column, w) in row {
                            let column_path = format!("{path}[{i}].{column}");
                            if !columns.contains(column) {
                                unknown.push(column_path);
                            } else if !single(w) {
                                errors
                                    .push(format!("column '{column_path}' expects a single value"));
                            }
                        }
                    }
                }
                _ => errors.push(format!(
                    "field '{path}' is a table and expects a list of rows"
                )),
            },
            Some(Shape::Rows) => match value {
                Value::Null => {}
                Value::Array(rows) => {
                    for (i, row) in rows.iter().enumerate() {
                        let Value::Object(row) = row else {
                            errors.push(format!("row '{path}[{i}]' is not an object with columns"));
                            continue;
                        };
                        for (column, w) in row {
                            if !single(w) {
                                errors.push(format!(
                                    "column '{path}[{i}].{column}' expects a single value"
                                ));
                            }
                        }
                    }
                }
                _ => errors.push(format!(
                    "field '{path}' is a table and expects a list of rows"
                )),
            },
        }
    }
}

/// A table as the gram records it: every row with all declared columns in
/// the order of the stream, a missing column as null.
fn as_table(value: Option<&Value>, columns: &[String]) -> Value {
    let Some(Value::Array(rows)) = value else {
        return Value::Null;
    };
    Value::Array(
        rows.iter()
            .map(|r| {
                Value::Object(
                    columns
                        .iter()
                        .map(|k| (k.clone(), r.get(k).cloned().unwrap_or(Value::Null)))
                        .collect(),
                )
            })
            .collect(),
    )
}

/// What is needed to build a gram, besides the stream itself.
pub struct Submission<'a> {
    /// The receiving channel: `channel` and what the login provides.
    pub intake: &'a Value,
    /// The content as submitted.
    pub external: &'a Map<String, Value>,
    /// The moment of recording: the clock of the cell.
    pub recorded_at: DateTime<FixedOffset>,
    /// Per reference of the event, the id of the gram the gram refers to. The
    /// cell checks under its lock that it exists and fits.
    pub refers_to: &'a BTreeMap<String, String>,
}

/// Build a gram from a submission. The gram keeps the shape of the stream: a
/// field that is not filled in is in it as null, because an incomplete
/// submission is recorded too. A field or table column that the stream does
/// not know is refused, with the field path: what has no legal basis is not
/// recorded.
pub fn build_gram(
    stream: &Stream,
    event: &Event,
    submission: &Submission<'_>,
) -> Result<Gram, String> {
    assessment_references(event, submission.refers_to)?;
    let shape = event.external_shape().map_err(|f| f.join("; "))?;
    let mut unknown = Vec::new();
    let mut errors = Vec::new();
    assessment_shape(submission.external, &shape, "", &mut unknown, &mut errors);
    if !unknown.is_empty() {
        unknown.sort_unstable();
        return Err(format!(
            "unknown field {}: the event '{}' does not record it",
            unknown
                .iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(", "),
            event.name
        ));
    }
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }

    let mut fields = Map::new();
    let mut field_provenance = BTreeMap::new();
    for leaf in event.leaves() {
        let value = match &leaf.binding {
            Binding::Intake(source_path) => intake_value(submission, source_path)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "the receiving channel does not supply '$intake.{source_path}' (field '{}')",
                        leaf.path
                    )
                })?,
            Binding::External(source_path) => at_path(submission.external, source_path)
                .cloned()
                .unwrap_or(Value::Null),
            Binding::Table { source, columns } => {
                as_table(at_path(submission.external, source), columns)
            }
            Binding::Rows { source } => at_path(submission.external, source)
                .cloned()
                .unwrap_or(Value::Null),
            Binding::Constant(w) => w.clone(),
            Binding::Supplied(name) => {
                let (value, provenance) = supplied_value(event, submission, name, &leaf.path)?;
                field_provenance.insert(leaf.path.clone(), provenance);
                value
            }
        };
        set_path(&mut fields, &leaf.path, value);
    }
    let (effective_at, effective_at_legal_basis, effective_at_stated) =
        effective_at_of(event, submission)?;

    Ok(Gram {
        kind: "chronolexogram".to_string(),
        id: crate::gram::new_id(submission.recorded_at),
        type_: event.type_.clone(),
        subtype: event.subtype.clone(),
        stage: event.stage.clone(),
        name: event.name.clone(),
        chronicle: stream.chronicle.clone(),
        recording_actor: stream.recording_actor.clone(),
        legal_basis: event.legal_basis.clone(),
        legal_character: None,
        decision_type: None,
        regulation: None,
        regulation_valid_from: None,
        competent_authority: None,
        acting_actor: None,
        effective_at: date::as_effective_at(&effective_at),
        effective_at_legal_basis,
        effective_at_stated,
        recorded_at: date::as_effective_at(&submission.recorded_at),
        refers_to: submission.refers_to.clone(),
        stream: StreamReference {
            id: stream.id.clone(),
            sha256: stream.sha256.clone(),
        },
        provenance: None,
        fields,
        field_provenance,
        inputs: BTreeMap::new(),
        receipt: None,
        times: Default::default(),
        root: None,
    })
}

/// The prefix of a binding to what the receiving channel supplies.
pub const SUPPLIED_BINDING: &str = "$supplied.";

/// The key under `$intake` where the receiving channel puts what it supplies,
/// per field: `{value, source, legal_basis}`.
pub const SUPPLIED: &str = "supplied";

/// The sources a receiving channel can name under `$intake.supplied`.
const SUPPLIED_SOURCES: [&str; 2] = ["channel", "register"];

/// The value of a `$supplied` field and where it came from: what the
/// receiving channel supplies under `$intake.supplied.<name>`, and otherwise
/// what was submitted under `<name>`. A submitted value that differs from a
/// supplied one is refused: the login and the register the policy names are
/// not for the submitter to change.
fn supplied_value(
    event: &Event,
    submission: &Submission<'_>,
    name: &str,
    path: &str,
) -> Result<(Value, crate::gram::FieldProvenance), String> {
    let submitted = at_path(submission.external, name)
        .cloned()
        .unwrap_or(Value::Null);
    let supplied = submission
        .intake
        .get(SUPPLIED)
        .and_then(|s| s.get(name))
        .filter(|s| s.get("value").is_some_and(|w| !w.is_null()));
    match supplied {
        Some(s) => {
            let value = s.get("value").cloned().unwrap_or(Value::Null);
            let source = s
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("channel")
                .to_string();
            // What a receiving channel supplies comes from the login or the
            // route, or from a register; `applicant` and `handler` are for
            // what was typed in (gram.json `field_provenance.source`).
            if !SUPPLIED_SOURCES.contains(&source.as_str()) {
                return Err(format!(
                    "field '{path}': $intake.supplied gives source '{source}'; a receiving channel supplies from: {}",
                    SUPPLIED_SOURCES.join(", ")
                ));
            }
            if !submitted.is_null() && submitted != value {
                return Err(format!(
                    "field '{path}' is supplied by the {source} ({value}); the submission may not change it ({submitted})"
                ));
            }
            let legal_basis = event.supply_legal_basis(name, &source, s);
            Ok((
                value,
                crate::gram::FieldProvenance {
                    source,
                    legal_basis,
                },
            ))
        }
        None => Ok((
            submitted,
            crate::gram::FieldProvenance {
                source: if event.is_submission() {
                    "applicant"
                } else {
                    "handler"
                }
                .into(),
                legal_basis: Vec::new(),
            },
        )),
    }
}

/// Whether the references the process passes fit the event: every name is a
/// reference of the event, and every required reference is present. The
/// cell checks under its lock whether the referenced gram exists and fits.
pub fn assessment_references(
    event: &Event,
    refers_to: &BTreeMap<String, String>,
) -> Result<(), String> {
    let name = &event.name;
    for n in refers_to.keys() {
        if !event.refers_to.contains_key(n) {
            let can: Vec<&str> = event.refers_to.keys().map(String::as_str).collect();
            return Err(format!(
                "event '{name}' does not refer with '{n}' (it does with: {})",
                if can.is_empty() {
                    "no reference".to_string()
                } else {
                    can.join(", ")
                }
            ));
        }
    }
    for (n, v) in &event.refers_to {
        if v.required && !refers_to.contains_key(n) {
            return Err(format!(
                "event '{name}' must refer with '{n}' to {}: pass the id",
                v.to
            ));
        }
    }
    Ok(())
}

fn intake_value<'i>(submission: &'i Submission<'_>, path: &str) -> Option<&'i Value> {
    submission.intake.as_object().and_then(|i| at_path(i, path))
}

/// The moment of the submitted value that the `effective_at` of an event
/// binds to (`$intake.<path>` or `$external.<path>`), parsed, with that
/// binding. `None` if the event binds nothing or the value is absent (or
/// null). A value that is not a date or moment is an error; a date is the
/// start of that day in the time zone `offset`. This is how the process
/// reads the reference date of an action from its form, and the cell the
/// `effective_at` of a gram.
pub fn bound_moment<'e>(
    event: &'e Event,
    intake: Option<&Map<String, Value>>,
    external: &Map<String, Value>,
    offset: FixedOffset,
) -> Result<Option<(DateTime<FixedOffset>, &'e EffectiveAtBinding)>, String> {
    let Some(b) = &event.effective_at else {
        return Ok(None);
    };
    let value = match b.binding() {
        Binding::Intake(path) => intake.and_then(|i| at_path(i, &path)),
        Binding::External(path) => at_path(external, &path),
        Binding::Supplied(path) => intake
            .and_then(|i| i.get(SUPPLIED))
            .and_then(|s| s.get(&path))
            .and_then(|s| s.get("value"))
            .filter(|w| !w.is_null())
            .or_else(|| at_path(external, &path)),
        _ => None,
    };
    let Some(text) = value.filter(|w| !w.is_null()) else {
        return Ok(None);
    };
    let text = text.as_str().ok_or_else(|| {
        format!(
            "'{}' (effective_at of event '{}') is not a date or moment",
            b.source, event.name
        )
    })?;
    let moment = date::TimePoint::read(&format!("effective_at from '{}'", b.source), text)?
        .as_moment(offset);
    Ok(Some((moment, b)))
}

/// The `effective_at` of a gram, its legal basis, and whether it was stated.
type EffectiveAt = (DateTime<FixedOffset>, Option<Vec<String>>, bool);

/// The `effective_at` of a gram, its legal basis, and whether it was stated:
/// bound to a submitted value that was present. Without a value (or without
/// a binding) it is the moment of recording, with the legal basis the law
/// gives for that, if any. A bound moment after the recording is refused:
/// what has yet to happen is not a fact. So is a stated moment for which the
/// law gives no legal basis.
fn effective_at_of(event: &Event, submission: &Submission<'_>) -> Result<EffectiveAt, String> {
    let now = submission.recorded_at;
    let intake = submission.intake.as_object();
    let Some((moment, b)) = bound_moment(event, intake, submission.external, *now.offset())? else {
        // Nothing states another moment: the recording counts, with the
        // legal basis the law gives for that, if it gives one.
        let recorded = event
            .effective_at
            .as_ref()
            .map(|b| b.legal_basis_recorded.clone())
            .filter(|g| !g.is_empty());
        return Ok((now, recorded, false));
    };
    if b.legal_basis.is_empty() {
        return Err(format!(
            "effective_at is stated ('{}'), but the law only says why the moment of recording counts: leave it out",
            b.source
        ));
    }
    if moment > now {
        return Err(format!(
            "effective_at {} from '{}' lies after the recording ({}): what has yet to happen is not recorded",
            date::as_effective_at(&moment),
            b.source,
            date::as_effective_at(&now)
        ));
    }
    Ok((moment, Some(b.legal_basis.clone()), true))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    const STREAM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");

    const CASE_HISTORY: &str =
        include_str!("../tests/fixtures/chronicles/test_afnemer_zaakverloop.yaml");

    /// What a key of the gram itself is for an event: fixed in the stream,
    /// dependent on the gram, or never; another key is a field path.
    #[test]
    fn the_attribute_of_an_event() {
        let s = parse(CASE_HISTORY, "fixture").unwrap();
        let (decision, known) = (
            s.events
                .iter()
                .find(|e| e.name == "besluit_genomen")
                .unwrap(),
            s.events
                .iter()
                .find(|e| e.name == "besluit_bekendgemaakt")
                .unwrap(),
        );
        assert_eq!(
            known.attribute(&s, "refers_to.decision"),
            Some(EventAttribute::Free)
        );
        assert_eq!(
            known.attribute(&s, "refers_to.amends"),
            Some(EventAttribute::Never)
        );
        assert_eq!(known.attribute(&s, "root"), Some(EventAttribute::Free));
        assert_eq!(known.attribute(&s, "zaak"), None);
        assert_eq!(
            decision.attribute(&s, "legal_character"),
            Some(EventAttribute::Free)
        );
        assert_eq!(known.attribute(&s, "content.naam"), None);
    }

    const CASE: &str = "00000000-0000-4000-8000-000000000001";

    fn moment() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap()
    }

    fn intake() -> Value {
        json!({"channel": "portaal", "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"}, "burger": {"nummer": null}})
    }

    #[test]
    fn fixture_loads_and_has_a_hash() {
        let s = parse(STREAM, "fixture").unwrap();
        assert_eq!(s.id, "test_aanvragen");
        assert_eq!(s.sha256.len(), 64);
        assert_eq!(s.events[0].legal_basis, vec!["testregeling_aanvraag#1"]);
    }

    /// Since chronolex v0.3.0 the schema no longer knows the core of an
    /// application (Awb 4:2 lid 1): the law gives it (note "het gram uit de
    /// wet"). A stream that names its own fields may leave it out.
    #[test]
    fn the_schema_no_longer_fixes_the_core_of_an_application() {
        let text = STREAM.replace("        dagtekening: $external.dagtekening\n", "");
        parse(&text, "t").unwrap();
    }

    /// An event with `establishes` names no fields: the law gives them. An
    /// event without names them.
    #[test]
    fn fields_only_without_establishes() {
        let only_registration = "$id: s\nrecording_actor: a\nchronicle: k\nevents:\n  - {name: x, establishes: 'r#1', intake: portal}\n";
        let s = parse(only_registration, "t").unwrap();
        assert_eq!(s.events[0].establishes, ["r#1"]);
        assert!(s.events[0].fields.is_empty());
        let error = parse(
            "$id: s\nrecording_actor: a\nchronicle: k\nevents:\n  - {name: x, intake: portal, type: act}\n",
            "t",
        )
        .unwrap_err();
        assert!(error.iter().any(|f| f.contains("fields")), "{error:?}");
    }

    #[test]
    fn invalid_stream_fails_on_the_schema() {
        let error = parse(
            "$id: x\nrecording_actor: y\nchronicle: z\nevents: []\n",
            "t",
        )
        .unwrap_err();
        assert!(error[0].contains("/events"), "{error:?}");
    }

    #[test]
    fn leaves_in_document_order() {
        let s = parse(STREAM, "fixture").unwrap();
        let paths: Vec<String> = s.events[0].leaves().into_iter().map(|b| b.path).collect();
        assert_eq!(paths[0], "core.aanvrager.naam");
        assert!(paths.contains(&"content.organen".to_string()));
        assert!(s.events[0].has_path("core.signed_via"));
        assert!(!s.events[0].has_leaf("core.signed_via"));
        assert!(!s.events[0].has_path("content.bestaat_niet"));
        assert_eq!(
            s.events[0].external_keys(),
            vec![
                "naam",
                "adres",
                "dagtekening",
                "aanvraagjaar",
                "aanduiding",
                "registratie",
                "organen",
                "rekeningnummer"
            ]
        );
    }

    #[test]
    fn build_gram_from_intake_and_external() {
        let s = parse(STREAM, "fixture").unwrap();
        let external = json!({"naam": "Vereniging Voorbeeld", "aanvraagjaar": 2025, "organen": [{"orgaan": "raad", "zetels": 2}]});
        let gram = build_gram(
            &s,
            &s.events[0],
            &Submission {
                intake: &intake(),
                external: external.as_object().unwrap(),
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
        .unwrap();
        assert_eq!(gram.type_, "submission");
        assert_eq!(gram.subtype.as_deref(), Some("aanvraag"));
        assert_eq!(gram.effective_at, "2025-03-12T10:14:03+01:00");
        assert_eq!(
            gram.field("core.signed_via.kvk_nummer"),
            Some(&json!("12345678"))
        );
        // One $external value feeds two fields.
        assert_eq!(
            gram.field("content.naam"),
            Some(&json!("Vereniging Voorbeeld"))
        );
        assert_eq!(
            gram.field("core.aanvrager.naam"),
            Some(&json!("Vereniging Voorbeeld"))
        );
        // A constant of the stream.
        assert_eq!(
            gram.field("core.gevraagde_beschikking"),
            Some(&json!("testbeschikking, testregeling artikel 1"))
        );
        // Not filled in: recorded as null, the shape stays.
        assert_eq!(gram.field("content.aanduiding"), Some(&Value::Null));
        gram.validate().unwrap();
    }

    fn with_intake(s: &Stream, intake: Value) -> Result<Gram, String> {
        build_gram(
            s,
            &s.events[0],
            &Submission {
                intake: &intake,
                external: &Map::new(),
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
    }

    /// What a receiving channel supplies names a source from the vocabulary
    /// of gram.json; another source is refused on building (a 400), not at
    /// the schema check under the lock.
    #[test]
    fn a_supplied_value_from_an_unknown_source_is_refused() {
        let text = STREAM.replace(
            "        rekeningnummer: $external.rekeningnummer\n",
            "        rekeningnummer: $supplied.rekeningnummer\n",
        );
        assert_ne!(text, STREAM);
        let s = parse(&text, "t").unwrap();
        let mut i = intake();
        i["supplied"] = json!({"rekeningnummer": {"value": "NL01", "source": "register", "legal_basis": ["testregeling_aanvraag#1"]}});
        let g = with_intake(&s, i.clone()).unwrap();
        assert_eq!(
            g.field_provenance["content.rekeningnummer"].source,
            "register"
        );
        i["supplied"]["rekeningnummer"]["source"] = json!("elders");
        let f = with_intake(&s, i).unwrap_err();
        assert!(f.contains("source 'elders'"), "{f}");
    }

    /// Two times: without a stated receipt, effective_at is the recording;
    /// with a date stamp from the counter it is that day, with the legal basis
    /// from the stream, and recorded_at stays the clock.
    #[test]
    fn effective_at_from_a_stated_receipt() {
        let s = parse(STREAM, "fixture").unwrap();
        let g = with_intake(&s, intake()).unwrap();
        assert_eq!(g.effective_at, "2025-03-12T10:14:03+01:00");
        assert_eq!(g.recorded_at, "2025-03-12T10:14:03+01:00");
        assert_eq!(g.effective_at_legal_basis, None);

        let mut counter = intake();
        counter["received_at"] = json!("2025-03-05");
        let g = with_intake(&s, counter.clone()).unwrap();
        assert_eq!(g.effective_at, "2025-03-05T00:00:00+01:00");
        assert_eq!(g.recorded_at, "2025-03-12T10:14:03+01:00");
        assert_eq!(
            g.effective_at_legal_basis,
            Some(vec!["testregeling_aanvraag#1".to_string()])
        );
        g.validate().unwrap();

        // A moment with time zone is allowed too; null means: not stated.
        counter["received_at"] = json!("2025-03-05T16:45:00+01:00");
        assert_eq!(
            with_intake(&s, counter.clone()).unwrap().effective_at,
            "2025-03-05T16:45:00+01:00"
        );
        counter["received_at"] = Value::Null;
        assert_eq!(
            with_intake(&s, counter.clone())
                .unwrap()
                .effective_at_legal_basis,
            None
        );

        // After the recording, or not a date: refused.
        counter["received_at"] = json!("2025-03-13");
        let f = with_intake(&s, counter.clone()).unwrap_err();
        assert!(f.contains("lies after the recording"), "{f}");
        counter["received_at"] = json!("vorige week");
        let f = with_intake(&s, counter.clone()).unwrap_err();
        assert!(
            f.contains("invalid effective_at from '$intake.received_at'"),
            "{f}"
        );
        counter["received_at"] = json!(20250305);
        let f = with_intake(&s, counter).unwrap_err();
        assert!(f.contains("is not a date or moment"), "{f}");
    }

    /// What may not choose its own moment of receipt binds to `$intake`: a
    /// `received_at` under `external` is an unknown field.
    #[test]
    fn the_submitter_does_not_choose_the_receipt() {
        let s = parse(STREAM, "fixture").unwrap();
        let f = build(&s, json!({"received_at": "2025-03-01"})).unwrap_err();
        assert!(f.contains("unknown field 'received_at'"), "{f}");
    }

    #[test]
    fn unknown_field_is_refused() {
        let s = parse(STREAM, "fixture").unwrap();
        let external = json!({"schoenmaat": 44});
        let error = build_gram(
            &s,
            &s.events[0],
            &Submission {
                intake: &intake(),
                external: external.as_object().unwrap(),
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("'schoenmaat'"), "{error}");
    }

    fn build(s: &Stream, external: Value) -> Result<Gram, String> {
        build_gram(
            s,
            &s.events[0],
            &Submission {
                intake: &intake(),
                external: external.as_object().unwrap(),
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
    }

    #[test]
    fn table_field_declares_its_columns() {
        let s = parse(STREAM, "fixture").unwrap();
        let e = &s.events[0];
        assert_eq!(
            e.columns("content.organen").unwrap(),
            vec!["orgaan", "zetels", "samengevoegd", "aantal_aanduidingen"]
        );
        assert_eq!(e.columns("content.naam"), None);
        assert!(e.has_leaf("content.organen"));
    }

    #[test]
    fn unknown_column_is_refused_with_field_path() {
        let s = parse(STREAM, "fixture").unwrap();
        let error = build(
            &s,
            json!({"organen": [{"orgaan": "raad"}, {"orgaan": "raad", "kleur": "rood"}]}),
        )
        .unwrap_err();
        assert!(
            error.contains("unknown field 'organen[1].kleur'"),
            "{error}"
        );
    }

    #[test]
    fn table_gets_every_column_in_the_gram() {
        let s = parse(STREAM, "fixture").unwrap();
        let gram = build(&s, json!({"organen": [{"zetels": 3, "orgaan": "raad"}]})).unwrap();
        assert_eq!(
            gram.field("content.organen"),
            Some(
                &json!([{"orgaan": "raad", "zetels": 3, "samengevoegd": null, "aantal_aanduidingen": null}])
            )
        );
        gram.validate().unwrap();
        // Not filled in: null, just like an ordinary field.
        let gram = build(&s, json!({})).unwrap();
        assert_eq!(gram.field("content.organen"), Some(&Value::Null));
        gram.validate().unwrap();
    }

    #[test]
    fn value_of_the_wrong_shape_is_refused() {
        let s = parse(STREAM, "fixture").unwrap();
        for (external, expected) in [
            (json!({"organen": "raad"}), "field 'organen' is a table"),
            (
                json!({"organen": ["raad"]}),
                "row 'organen[0]' is not an object",
            ),
            (
                json!({"organen": [{"zetels": {"aantal": 3}}]}),
                "column 'organen[0].zetels' expects a single value",
            ),
            (
                json!({"naam": {"voornaam": "A"}}),
                "field 'naam' expects a single value",
            ),
        ] {
            let error = build(&s, external).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn unknown_field_in_a_nested_external_object() {
        let text = STREAM.replace("adres: $external.adres", "adres: $external.adres.straat");
        let s = parse(&text, "t").unwrap();
        let gram = build(&s, json!({"adres": {"straat": "Voorbeeldstraat 1"}})).unwrap();
        assert_eq!(
            gram.field("core.aanvrager.adres"),
            Some(&json!("Voorbeeldstraat 1"))
        );
        let error = build(&s, json!({"adres": {"straat": "x", "huisdier": "kat"}})).unwrap_err();
        assert!(error.contains("unknown field 'adres.huisdier'"), "{error}");
    }

    #[test]
    fn a_source_path_with_two_shapes_fails_on_load() {
        let text = STREAM.replace(
            "rekeningnummer: $external.rekeningnummer",
            "rekeningnummer: $external.organen",
        );
        let error = parse(&text, "t").unwrap_err();
        assert!(
            error
                .iter()
                .any(|f| f.contains("'$external.organen' gets more than one shape")),
            "{error:?}"
        );
    }

    #[test]
    fn table_without_columns_fails_on_the_schema() {
        let text = STREAM.replace(
            "          columns: [orgaan, zetels, samengevoegd, aantal_aanduidingen]\n",
            "          columns: []\n",
        );
        let error = parse(&text, "t").unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("/events/0/fields/content")),
            "{error:?}"
        );
    }

    /// The references the process passes fit the event: an unknown name and
    /// a missing required reference are refused. The roles (decision,
    /// follows, root) follow from stage and references.
    #[test]
    fn references_and_roles() {
        let mut streams = vec![
            parse(
                include_str!("../tests/fixtures/chronicles/test_afnemer_aanvragen.yaml"),
                "a",
            )
            .unwrap(),
            parse(CASE_HISTORY, "v").unwrap(),
        ];
        assert!(check_references(&streams).is_empty());
        derive_roles(&mut streams);
        let e = |n: &str| {
            streams
                .iter()
                .flat_map(|s| s.events.iter())
                .find(|e| e.name == n)
                .unwrap()
                .clone()
        };
        assert_eq!(e("aanvraag_ontvangen").case, Case::Opens);
        assert_eq!(e("besluit_genomen").decision, Some(Decision::Opens));
        assert_eq!(e("betaling_verricht").decision, Some(Decision::Follows));
        assert_eq!(e("aanvulling_gevraagd").decision, None);
        assert_eq!(e("aanvulling_gevraagd").case, Case::Follows);
        let payment = e("betaling_verricht");
        let empty = BTreeMap::new();
        assert!(assessment_references(&payment, &empty)
            .unwrap_err()
            .contains("must refer with 'decision'"));
        let mut v = BTreeMap::new();
        v.insert("application".to_string(), CASE.to_string());
        assert!(assessment_references(&payment, &v)
            .unwrap_err()
            .contains("does not refer with 'application'"));
        v.clear();
        v.insert("decision".to_string(), CASE.to_string());
        assessment_references(&payment, &v).unwrap();
        // A reference that no event of the cell fits.
        let loose = parse(
            &CASE_HISTORY.replace("to: aanvraag_ontvangen", "to: bestaat_niet"),
            "v",
        )
        .unwrap();
        assert!(!check_references(&[loose]).is_empty());
    }

    #[test]
    fn missing_intake_is_an_error() {
        let s = parse(STREAM, "fixture").unwrap();
        let external = Map::new();
        let error = build_gram(
            &s,
            &s.events[0],
            &Submission {
                intake: &json!({"channel": "portaal"}),
                external: &external,
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("$intake.eherkenning.kvk"), "{error}");
    }
}

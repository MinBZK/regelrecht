//! The gram: a fact as a cell records it
//! (`schema/chronolex/v0.3.0/gram.json`), with what a decision carries along.
//!
//! A gram has its own id (a uuid v7, which the cell assigns when recording)
//! and refers, with a name from the law text, to the gram it belongs to
//! (`refers_to`: a decision `op_aanvraag`, a payment to the `besluit`). There
//! is no case or decision identifier: a group follows from the references.
//! The root of a gram (the gram without a reference that it leads to through
//! its references, such as the application) is kept by the chronicle in an
//! index; it is not in the gram.
//! How a gram arises from a stream and a submission is described in
//! [`crate::stream`].
//!
//! A gram has two times (paper P:46: "op 3 april heeft de ambtenaar
//! vastgesteld dat ... per 2 april"):
//!
//! - `effective_at`: when the fact legally holds or took place. By default
//!   that is the moment of recording; an event can bind it to a submitted
//!   value, with a legal basis (`effective_at_legal_basis`), such as the day of
//!   receipt of an application that came in by another route (Awb 4:1,
//!   4:13). In an initial state it is set by hand (the date of a decision, or
//!   of a determination).
//! - `recorded_at`: when the cell recorded it, always its own clock; for an
//!   initial state, the load time.
//!
//! A chronicle from before chronolex v0.2.0 (without id, with case and
//! decision identifiers) is not converted: the cell refuses to load it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::date;
use crate::schema::{self, Kind};

/// The recorded gram (`schema/chronolex/v0.3.0/gram.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gram {
    pub kind: String,
    /// Its own id: a uuid v7, which the cell assigns when recording (under
    /// the same lock as `recorded_at`).
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub name: String,
    pub chronicle: String,
    pub recording_actor: String,
    pub legal_basis: Vec<String>,
    /// Only for a decision that a process took: the legal character and the
    /// decision type from `produces` of the article (RFC-008).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_character: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_type: Option<String>,
    /// The regulation the decision rests on, with its version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_valid_from: Option<String>,
    /// The competent authority according to the law. If the regulation lacks
    /// it, it is not there: the cell does not invent an authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competent_authority: Option<String>,
    /// Only for a decision that a process took: who acted (see
    /// [`ActingActor`]). Next to `recording_actor` (who records) and
    /// `competent_authority` (whom the law makes competent), the third axis of
    /// RFC-022 §2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acting_actor: Option<ActingActor>,
    /// When the fact legally holds or took place.
    pub effective_at: String,
    /// Only if the event bound `effective_at` to a submitted value and that
    /// value was present: its legal basis, from the stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_at_legal_basis: Option<Vec<String>>,
    /// When the cell recorded the gram: its own clock.
    pub recorded_at: String,
    /// Which grams this gram refers to, per name from the law text
    /// (`op_aanvraag`, `besluit`, `amends`, ...): the id of that gram.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refers_to: BTreeMap<String, String>,
    pub stream: StreamReference,
    /// Only if the cell did not establish the gram itself: `initial_state` was
    /// placed into an empty chronicle at startup (see [`crate::initial_state`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    pub fields: Map<String, Value>,
    /// Per field that the receiving channel could supply (`$supplied` in the
    /// stream), where its value came from this time: the login, a register
    /// the policy names, or whoever submitted it (note "het gram uit de wet":
    /// the law puts the field in the gram, the provenance may differ per time).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_provenance: BTreeMap<String, FieldProvenance>,
    /// For every action the engine computed (a decision, a follow-up or a
    /// fact with outputs): every parameter that took part, with its value and
    /// its provenance (RFC-013 `accepted_values`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Input>,
    /// For every action the engine computed: what took part, with the hash
    /// over it (RFC-013, RFC-022 par. 1.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
    /// `effective_at` and `recorded_at`, parsed: once per gram, not on every
    /// reduction or as-of point. Not part of the gram.
    #[serde(skip)]
    pub times: Times,
    /// The root of the gram, from the index of the chronicle: the id of the
    /// gram without a reference that it leads to through its references (a
    /// gram without a reference is its own root). Not part of the gram: the
    /// chronicle fills it in when loading and recording.
    #[serde(skip)]
    pub root: Option<String>,
}

/// The parsed times of a gram, each with the text it was parsed from. If the
/// text changes (the stamp sets `recorded_at`), the gram parses it again; the
/// cache does not count in a comparison.
#[derive(Debug, Clone, Default)]
pub struct Times {
    moment: OnceLock<(String, DateTime<FixedOffset>)>,
    recorded: OnceLock<(String, DateTime<FixedOffset>)>,
}

impl PartialEq for Times {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// A time from `cache` if it was parsed from `text`, otherwise `read`, and
/// that stored if nothing was there yet.
fn read(
    cache: &OnceLock<(String, DateTime<FixedOffset>)>,
    text: &str,
    read: impl FnOnce() -> Result<DateTime<FixedOffset>, String>,
) -> Result<DateTime<FixedOffset>, String> {
    if let Some((t, m)) = cache.get() {
        if t == text {
            return Ok(*m);
        }
    }
    let m = read()?;
    // If a time from an earlier text was already there, it stays and this
    // text is parsed again every time: correct, just not cached.
    let _ = cache.set((text.to_string(), m));
    Ok(m)
}

/// Who took a decision: the role and the channel through which the user
/// logged in, with the values of the identification fields, and on behalf of
/// which authority. If the process acts under mandate (Awb 10:1), `mandate`
/// names the legal basis. The channel is simulated: the identity is what the
/// user entered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActingActor {
    pub role: String,
    pub channel: String,
    pub identity: BTreeMap<String, String>,
    /// The legal basis of the role, if the configuration names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<String>,
    /// The authority on whose behalf the action was taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<String>,
    /// The legal basis of the mandate, if the authority is not the process's
    /// own authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandate: Option<String>,
}

/// Where the value of a `$supplied` field came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldProvenance {
    /// `channel`, `register`, `applicant` or `handler`.
    pub source: String,
    /// Why that source may supply the field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legal_basis: Vec<String>,
}

/// An accepted input of a decision: a value with its provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub value: Value,
    pub provenance: crate::synthesis::Provenance,
}

/// What took part in a decision, so it can be repeated: the loaded
/// regulations and the stream definitions, with a hash over both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub regulations: Vec<LoadedRegulation>,
    pub streams: Vec<StreamReference>,
    /// SHA-256 over the two lists above, as canonical JSON.
    pub sha256: String,
}

/// A regulation as the runtime loaded it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LoadedRegulation {
    pub id: String,
    pub valid_from: String,
    pub sha256: String,
}

impl Receipt {
    /// Build the receipt and compute the hash.
    pub fn new(regulations: Vec<LoadedRegulation>, streams: Vec<StreamReference>) -> Self {
        let canonical =
            serde_json::json!({"regulations": regulations, "streams": streams}).to_string();
        Self {
            regulations,
            streams,
            sha256: hex::encode(Sha256::digest(canonical.as_bytes())),
        }
    }
}

/// Which stream definition built a gram, and which version of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamReference {
    pub id: String,
    pub sha256: String,
}

/// The prefix of a filter key on a reference: `refers_to.<name>` is the id
/// the gram refers to under that name.
pub const REFERS_TO: &str = "refers_to.";

/// A new id for a gram: a uuid v7 at the moment `now` (the clock of the
/// cell), so ids increase over time.
pub fn new_id(now: DateTime<FixedOffset>) -> String {
    let ts = uuid::Timestamp::from_unix(
        uuid::NoContext,
        u64::try_from(now.timestamp()).unwrap_or(0),
        now.timestamp_subsec_nanos(),
    );
    uuid::Uuid::new_v7(ts).to_string()
}

/// A fixed id: a uuid v5 over `key`, so every read gives the same id (for a
/// line of an initial state without its own id).
pub fn fixed_id(key: &str) -> String {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, key.as_bytes()).to_string()
}

impl Gram {
    /// The gram as a JSON value.
    pub fn as_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Validate the gram against `gram.json`, and its `effective_at` and
    /// `recorded_at` as a moment with a time zone (the schema only checks the
    /// shape).
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let json = serde_json::to_value(self).map_err(|e| vec![e.to_string()])?;
        let mut errors = schema::validate(Kind::Gram, &json)
            .err()
            .unwrap_or_default();
        if let Err(f) = date::moment(&self.effective_at) {
            errors.push(f);
        }
        if let Err(f) = date::moment_of("recorded_at", &self.recorded_at) {
            errors.push(f);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// A field of the gram itself that a filter selects on (a key from
    /// [`crate::reduction::GRAM_KEYS`]), as text. `None` if `key` is not such
    /// a field: then it is a field path under `fields`. `Some(None)` if the
    /// gram does not have the field.
    pub fn attribute(&self, key: &str) -> Option<Option<&str>> {
        if let Some(name) = key.strip_prefix(REFERS_TO) {
            return Some(self.refers_to.get(name).map(String::as_str));
        }
        Some(match key {
            "id" => Some(self.id.as_str()),
            "root" => self.root.as_deref(),
            "name" => Some(self.name.as_str()),
            "type" => Some(self.type_.as_str()),
            "subtype" => self.subtype.as_deref(),
            "stage" => self.stage.as_deref(),
            "recording_actor" => Some(self.recording_actor.as_str()),
            "chronicle" => Some(self.chronicle.as_str()),
            "legal_character" => self.legal_character.as_deref(),
            "decision_type" => self.decision_type.as_deref(),
            "regulation" => self.regulation.as_deref(),
            "competent_authority" => self.competent_authority.as_deref(),
            _ => return None,
        })
    }

    /// The value at a path under `fields`.
    pub fn field(&self, path: &str) -> Option<&Value> {
        at_path(&self.fields, path)
    }

    /// The `effective_at`, parsed; an invalid moment is an error.
    pub fn moment(&self) -> Result<DateTime<FixedOffset>, String> {
        read(&self.times.moment, &self.effective_at, || {
            date::moment(&self.effective_at).map_err(|e| format!("gram '{}': {e}", self.name))
        })
    }

    /// The `recorded_at`, parsed; an invalid moment is an error.
    pub fn recorded(&self) -> Result<DateTime<FixedOffset>, String> {
        read(&self.times.recorded, &self.recorded_at, || {
            date::moment_of("recorded_at", &self.recorded_at)
                .map_err(|e| format!("gram '{}': {e}", self.name))
        })
    }

    /// Set the moment of recording: the cell does this under its write lock,
    /// so the order of the lines in the chronicle is that of `recorded_at`.
    /// `not_for` is the `recorded_at` of the last line of the chronicle: if the
    /// clock runs backwards, the gram gets that moment, not an earlier one. An
    /// `effective_at` that the event did not bind to a value (without
    /// `effective_at_legal_basis`) is the moment of recording and moves along;
    /// a bound `effective_at` may not lie after it.
    pub fn stamp(
        &mut self,
        now: DateTime<FixedOffset>,
        not_for: Option<DateTime<FixedOffset>>,
    ) -> Result<(), String> {
        let moment = match not_for {
            Some(v) if v > now => v,
            _ => now,
        };
        let text = date::as_effective_at(&moment);
        if self.effective_at_legal_basis.is_none() && self.provenance.is_none() {
            self.effective_at = text.clone();
        } else if self.moment()? > moment {
            return Err(format!(
                "effective_at {} lies after the recording ({text}): what has yet to happen is not recorded",
                self.effective_at
            ));
        }
        self.recorded_at = text;
        Ok(())
    }

    /// The order of two grams in time: first by `effective_at`, on an equal
    /// moment by `recorded_at`. `Equal` lets the order in the chronicle
    /// decide.
    pub fn time_order(&self, other: &Gram) -> Result<std::cmp::Ordering, String> {
        Ok(self
            .moment()?
            .cmp(&other.moment()?)
            .then(self.recorded()?.cmp(&other.recorded()?)))
    }
}

/// The value at a dotted field path (`inhoud.organen`) in an object; `None`
/// if a part of the path is not there or is not an object.
pub fn at_path<'v>(fields: &'v Map<String, Value>, path: &str) -> Option<&'v Value> {
    let mut parts = path.split('.');
    let mut current = fields.get(parts.next()?)?;
    for part in parts {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}
/// Set a value at a dotted field path, and create the intermediate objects;
/// whatever on the way is not an object becomes one.
pub fn set_path(target: &mut Map<String, Value>, path: &str, value: Value) {
    let mut parts = path.split('.').peekable();
    let mut here = target;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            here.insert(part.to_string(), value);
            return;
        }
        let next = here
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !next.is_object() {
            *next = Value::Object(Map::new());
        }
        let Value::Object(m) = next else {
            return;
        };
        here = m;
    }
}

/// A gram for tests: a notification in `test_kroniek` with this id, without
/// a reference (so its own root).
#[cfg(test)]
pub(crate) fn test_gram(id: &str) -> Gram {
    Gram {
        kind: "chronolexogram".into(),
        id: id.into(),
        type_: "submission".into(),
        subtype: Some("melding".into()),
        stage: None,
        name: "melding_ontvangen".into(),
        chronicle: "test_kroniek".into(),
        recording_actor: "test_instantie".into(),
        legal_basis: vec!["testregeling_aanvraag#1".into()],
        legal_character: None,
        decision_type: None,
        regulation: None,
        regulation_valid_from: None,
        competent_authority: None,
        acting_actor: None,
        effective_at: "2025-03-12T10:14:03+01:00".into(),
        effective_at_legal_basis: None,
        recorded_at: "2025-03-12T10:14:05+01:00".into(),
        refers_to: BTreeMap::new(),
        stream: StreamReference {
            id: "test".into(),
            sha256: "a".repeat(64),
        },
        provenance: None,
        fields: serde_json::json!({"x": 1})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        field_provenance: BTreeMap::new(),
        inputs: BTreeMap::new(),
        receipt: None,
        times: Times::default(),
        root: Some(id.into()),
    }
}

/// A gram for tests that refers with `name` to `target`, with a new id; the
/// root is that of the target.
#[cfg(test)]
pub(crate) fn test_follower(name: &str, target: &Gram) -> Gram {
    let mut g = test_gram(&uuid::Uuid::now_v7().to_string());
    g.refers_to.insert(name.into(), target.id.clone());
    g.root = target.root.clone();
    g
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn set_path_creates_the_objects() {
        let mut m = Map::new();
        set_path(&mut m, "a.b.c", json!(1));
        set_path(&mut m, "a.d", json!(2));
        assert_eq!(at_path(&m, "a.b.c"), Some(&json!(1)));
        assert_eq!(Value::Object(m), json!({"a": {"b": {"c": 1}, "d": 2}}));
    }

    /// The times of a gram are parsed once, and again when the text changes
    /// (as with the stamp).
    #[test]
    fn the_parsed_time_follows_the_text() {
        let mut g = test_gram("z");
        let first = g.recorded().unwrap();
        assert_eq!(g.recorded().unwrap(), first);
        let later = DateTime::parse_from_rfc3339("2025-03-13T09:00:00+01:00").unwrap();
        g.stamp(later, None).unwrap();
        assert_eq!(g.recorded().unwrap(), later);
        assert_eq!(
            g.moment().unwrap(),
            later,
            "an unbound effective_at moves along"
        );
        g.effective_at = "not a moment".into();
        assert!(g.moment().is_err());
    }
}

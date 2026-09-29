//! The initial state: grams that a cell puts into an empty chronicle at
//! startup.
//!
//! A line of the JSONL file names the stream, the event (`name`),
//! `effective_at`, `provenance: initial_state` and `fields`, and if needed `id`
//! and `refers_to` (to an earlier gram of the initial state, with the names of
//! the event). Without `id` the gram gets a fixed id derived from the line
//! itself, so every load gives the same id. The rest (type, subtype, legal
//! basis, chronicle, actor and the hash of the stream) follows from the
//! stream, so an initial state does not go stale when the stream changes. The
//! fields must be exactly those of the event.
//!
//! An initial-state gram is placed, not computed: there is no engine trace
//! with it. That is why it carries `provenance: initial_state`.
//!
//! The `effective_at` of a line is set by hand: when the fact legally holds or
//! took place, such as the day of a decision or of a determination.
//! `recorded_at` is not in it: that is the load time, the moment at which the
//! runtime puts the initial state into the empty chronicle ([`placed`]). This
//! way an initial state never says that the cell knew something before it had
//! it.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::date;
use crate::gram::{at_path, Gram, StreamReference};
use crate::load;
use crate::stream::{Event, Stream};

/// The only provenance a line of the initial state may have.
pub const PROVENANCE: &str = "initial_state";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    stream: String,
    name: String,
    effective_at: String,
    provenance: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    refers_to: BTreeMap<String, String>,
    fields: Map<String, Value>,
}

/// Read the initial state and build the grams. Every error names the line.
pub fn load(path: &Path, streams: &[Stream]) -> Result<Vec<Gram>, Vec<String>> {
    load::load(path, |text, source| parse(text, source, streams))
}

/// Build the grams from the text of an initial state.
pub fn parse(text: &str, source: &str, streams: &[Stream]) -> Result<Vec<Gram>, Vec<String>> {
    let mut grams = Vec::new();
    let mut errors = Vec::new();
    for (i, row) in text.lines().enumerate() {
        if row.trim().is_empty() {
            continue;
        }
        match build(row, streams) {
            Ok(g) => grams.push(g),
            Err(f) => errors.push(format!("{source} line {}: {f}", i + 1)),
        }
    }
    if errors.is_empty() {
        Ok(grams)
    } else {
        Err(errors)
    }
}

fn build(text: &str, streams: &[Stream]) -> Result<Gram, String> {
    let row: Row = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if row.provenance != PROVENANCE {
        return Err(format!(
            "provenance is '{}', an initial state has provenance '{PROVENANCE}'",
            row.provenance
        ));
    }
    let stream = streams
        .iter()
        .find(|s| s.id == row.stream)
        .ok_or_else(|| format!("stream '{}' is not a stream of this cell", row.stream))?;
    let event = stream
        .event(&row.name)
        .ok_or_else(|| format!("stream '{}' has no event '{}'", stream.id, row.name))?;
    fields_fit(event, &row.fields, "")?;
    crate::stream::assessment_references(event, &row.refers_to)?;
    let id = row.id.clone().unwrap_or_else(|| {
        crate::gram::fixed_id(&format!(
            "urn:regelrecht:cel:startstand:{}:{}",
            stream.id,
            text.trim()
        ))
    });
    let gram = Gram {
        kind: "chronolexogram".into(),
        id,
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
        effective_at: row.effective_at,
        effective_at_legal_basis: None,
        // The load time comes with placing, see `placed`.
        recorded_at: String::new(),
        refers_to: row.refers_to,
        stream: StreamReference {
            id: stream.id.clone(),
            sha256: stream.sha256.clone(),
        },
        provenance: Some(PROVENANCE.into()),
        fields: row.fields,
        field_provenance: BTreeMap::new(),
        inputs: BTreeMap::new(),
        receipt: None,
        times: Default::default(),
        root: None,
    };
    // Validation is already possible: with the effective_at in place of the load time.
    Gram {
        recorded_at: gram.effective_at.clone(),
        ..gram.clone()
    }
    .validate()
    .map_err(|f| format!("gram does not validate: {}", f.join("; ")))?;
    Ok(gram)
}

/// The initial state as the runtime puts it into an empty chronicle: every
/// gram with the load time as `recorded_at`. A line with an `effective_at`
/// after the load time is refused, as with any other gram: what has yet to
/// happen is not a fact. A decision that takes effect on a later day (a
/// removal effective next year) is recorded on the day it was taken; the
/// effective day is then a field of the gram.
pub fn placed(grams: &[Gram], load_time: &DateTime<FixedOffset>) -> Result<Vec<Gram>, String> {
    let op = date::as_effective_at(load_time);
    let mut out = Vec::with_capacity(grams.len());
    for g in grams {
        if g.moment()? > *load_time {
            return Err(format!(
                "initial_state: '{}' has effective_at {}, after the load time ({op}); what has yet to happen is not recorded",
                g.name, g.effective_at
            ));
        }
        out.push(Gram {
            recorded_at: op.clone(),
            ..g.clone()
        });
    }
    Ok(out)
}

/// The fields are exactly those of the event: no unknown field, and every
/// leaf of the event is present (as null if need be).
fn fields_fit(event: &Event, fields: &Map<String, Value>, prefix: &str) -> Result<(), String> {
    for (name, value) in fields {
        let path = format!("{prefix}{name}");
        if !event.has_path(&path) {
            return Err(format!("event '{}' has no field '{path}'", event.name));
        }
        if !event.has_leaf(&path) {
            let Value::Object(sub) = value else {
                return Err(format!("field '{path}' expects fields below it"));
            };
            fields_fit(event, sub, &format!("{path}."))?;
        }
    }
    if prefix.is_empty() {
        for leaf in event.leaves() {
            if at_path(fields, &leaf.path).is_none() {
                return Err(format!(
                    "field '{}' of event '{}' is absent",
                    leaf.path, event.name
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const STREAM: &str = include_str!("../tests/fixtures/chronicles/test_registers.yaml");
    const INITIAL_STATE: &str =
        include_str!("../tests/fixtures/cells/register/initial_state.jsonl");

    fn streams() -> Vec<Stream> {
        vec![crate::stream::parse(STREAM, "stream").unwrap()]
    }

    fn error(row: &str) -> String {
        parse(row, "s", &streams()).unwrap_err().join("; ")
    }

    #[test]
    fn fixture_initial_state() {
        let grams = parse(INITIAL_STATE, "s", &streams()).unwrap();
        assert_eq!(grams.len(), 4);
        let load_time = date::moment("2025-03-12T10:14:03+01:00").unwrap();
        for (g, placed) in grams.iter().zip(placed(&grams, &load_time).unwrap()) {
            assert_eq!(g.provenance.as_deref(), Some("initial_state"));
            assert_eq!(g.type_, "decretogram");
            assert_eq!(g.chronicle, "test_register");
            // Two times: the effective_at of the line, the load time as
            // recorded_at.
            assert_eq!(placed.effective_at, g.effective_at);
            assert_eq!(placed.recorded_at, "2025-03-12T10:14:03+01:00");
            placed.validate().unwrap();
        }
        assert_eq!(grams[0].legal_basis, ["testregeling_register#1"]);
    }

    /// A line with an effective_at after the load time is not placed.
    #[test]
    fn an_initial_state_from_the_future_is_refused() {
        let grams = parse(INITIAL_STATE, "s", &streams()).unwrap();
        let earlier = date::moment("2024-02-01T00:00:00+01:00").unwrap();
        let f = placed(&grams, &earlier).unwrap_err();
        assert!(f.contains("after the load time"), "{f}");
    }

    #[test]
    fn an_initial_state_sets_no_recorded_at() {
        // That is the load time: an initial state does not say when the cell
        // knew it.
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "recorded_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(error(r).contains("recorded_at"), "{}", error(r));
    }

    #[test]
    fn line_without_initial_state_provenance() {
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "engine", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(
            error(r).contains("provenance 'initial_state'"),
            "{}",
            error(r)
        );
    }

    #[test]
    fn unknown_and_absent_field() {
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "X", "orgaan": "raad", "kleur": 1}}"#;
        assert!(error(r).contains("no field 'kleur'"), "{}", error(r));
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "X"}}"#;
        assert!(error(r).contains("'orgaan' of event"), "{}", error(r));
    }

    #[test]
    fn unknown_event_and_invalid_moment() {
        let r = r#"{"stream": "test_registers", "name": "bestaat_niet", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "fields": {}}"#;
        assert!(error(r).contains("no event 'bestaat_niet'"));
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "gisteren", "provenance": "initial_state", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(error(r).contains("effective_at"), "{}", error(r));
        assert!(error(r).starts_with("s line 1: "));
    }

    /// A line without id gets a fixed id derived from the line itself; a line
    /// may name an id and references, with the names of the event.
    #[test]
    fn id_and_references() {
        let a = parse(INITIAL_STATE, "s", &streams()).unwrap();
        let b = parse(INITIAL_STATE, "s", &streams()).unwrap();
        assert_eq!(a[0].id, b[0].id, "every load gives the same id");
        assert_ne!(a[0].id, a[1].id);
        assert!(a[0].refers_to.is_empty());
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "refers_to": {"zaak": "00000000-0000-4000-8000-000000000001"}, "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(error(r).contains("'zaak'"), "{}", error(r));
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "id": "00000000-0000-4000-8000-00000000000a", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        let g = parse(r, "s", &streams()).unwrap().remove(0);
        assert_eq!(g.id, "00000000-0000-4000-8000-00000000000a");
        let load_time = date::moment("2025-03-12T10:14:03+01:00").unwrap();
        placed(&[g], &load_time).unwrap()[0].validate().unwrap();
    }
}

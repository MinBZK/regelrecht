//! The JSON schemas from `schema/chronolex/v0.3.0/`, embedded at build
//! time. The files are the source; this module compiles them once.

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;

const STREAM: &str = include_str!("../../../schema/chronolex/v0.3.0/stream.json");
const LEXOSTATUS: &str = include_str!("../../../schema/chronolex/v0.3.0/lexostatus.json");
const GRAM: &str = include_str!("../../../schema/chronolex/v0.3.0/gram.json");
const CELL: &str = include_str!("../../../schema/chronolex/v0.3.0/cell.json");
const PROCESS: &str = include_str!("../../../schema/chronolex/v0.3.0/process.json");

/// Which of the schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A stream definition (`stream.json`).
    Stream,
    /// The lexostatus definitions of a cell (`lexostatus.json`).
    Lexostatus,
    /// A cell definition, `cell.yaml` (`cell.json`).
    Cell,
    /// A process definition, `process.yaml` (`process.json`).
    Process,
    /// A recorded gram (`gram.json`).
    Gram,
}

fn compile(source: &str, name: &str) -> Result<Validator, String> {
    let schema: Value = serde_json::from_str(source)
        .map_err(|e| format!("embedded schema {name} is not valid JSON: {e}"))?;
    Validator::new(&schema).map_err(|e| format!("embedded schema {name} does not compile: {e}"))
}

static STREAM_V: LazyLock<Result<Validator, String>> =
    LazyLock::new(|| compile(STREAM, "stream.json"));
static LEXOSTATUS_V: LazyLock<Result<Validator, String>> =
    LazyLock::new(|| compile(LEXOSTATUS, "lexostatus.json"));
static GRAM_V: LazyLock<Result<Validator, String>> = LazyLock::new(|| compile(GRAM, "gram.json"));
static CELL_V: LazyLock<Result<Validator, String>> = LazyLock::new(|| compile(CELL, "cell.json"));
static PROCESS_V: LazyLock<Result<Validator, String>> =
    LazyLock::new(|| compile(PROCESS, "process.json"));

/// Validate a document against one of the schemas. On failure: every
/// violation as `<path>: <message>`.
pub fn validate(kind: Kind, doc: &Value) -> Result<(), Vec<String>> {
    let validator = match kind {
        Kind::Stream => &*STREAM_V,
        Kind::Lexostatus => &*LEXOSTATUS_V,
        Kind::Gram => &*GRAM_V,
        Kind::Cell => &*CELL_V,
        Kind::Process => &*PROCESS_V,
    }
    .as_ref()
    .map_err(|e| vec![e.clone()])?;
    let errors: Vec<String> = validator
        .iter_errors(doc)
        .map(|e| {
            let path = e.instance_path().to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{path}: {e}")
            }
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn all_schemas_compile() {
        for (name, v) in [
            ("stream", &*STREAM_V),
            ("lexostatus", &*LEXOSTATUS_V),
            ("gram", &*GRAM_V),
            ("cell", &*CELL_V),
            ("process", &*PROCESS_V),
        ] {
            assert!(v.is_ok(), "{name}: {:?}", v.as_ref().err());
        }
    }

    /// A cell whose lexostatuses all come from the law (RFC-043) and the
    /// runtime (the worklist, RFC-047) defines none of its own.
    #[test]
    fn a_cell_may_define_no_lexostatus_of_its_own() {
        let doc = serde_json::json!({
            "cell": "c",
            "law": [{"article": "r#1", "extra_fields": {"k": {"field": "k"}}}],
            "lexostatus_definitions": []
        });
        validate(Kind::Lexostatus, &doc).unwrap();
    }

    #[test]
    fn a_violation_names_the_path() {
        let doc = serde_json::json!({"$id": "x", "recording_actor": "y", "chronicle": "z", "events": [{"name": "Fout"}]});
        let errors = validate(Kind::Stream, &doc).unwrap_err();
        assert!(
            errors.iter().any(|f| f.starts_with("/events/0")),
            "{errors:?}"
        );
    }

    fn gram() -> Value {
        serde_json::json!({
            "kind": "chronolexogram", "id": "01900000-0000-7000-8000-000000000001",
            "type": "decretogram", "name": "x",
            "chronicle": "k", "recording_actor": "a", "legal_basis": ["r#1"],
            "effective_at": "2025-03-12T10:14:03+01:00",
            "recorded_at": "2025-03-12T10:14:03+01:00",
            "stream": {"id": "s", "sha256": "0".repeat(64)}, "fields": {}
        })
    }

    /// A gram has an id (a uuid) and refers by name to another id; v0.2.0
    /// no longer knows a case or decision reference number.
    #[test]
    fn gram_id_and_references() {
        let mut g = gram();
        validate(Kind::Gram, &g).unwrap();
        g["refers_to"] =
            serde_json::json!({"on_application": "00000000-0000-4000-8000-000000000001"});
        validate(Kind::Gram, &g).unwrap();
        g["refers_to"] = serde_json::json!({"on_application": "g-001"});
        assert!(validate(Kind::Gram, &g).is_err());
        let mut z = gram();
        z["zaakkenmerk"] = "00000000-0000-4000-8000-000000000001".into();
        assert!(validate(Kind::Gram, &z).is_err());
        let mut z = gram();
        z.as_object_mut().unwrap().remove("id");
        assert!(validate(Kind::Gram, &z).is_err());
    }

    #[test]
    fn stage_on_a_decretogram_submission_or_act() {
        let mut g = gram();
        g["type"] = "submission".into();
        g["subtype"] = "melding".into();
        g["stage"] = "AANVRAAG".into();
        validate(Kind::Gram, &g).unwrap();
        g["type"] = "decretogram".into();
        g["stage"] = "BESLUIT".into();
        validate(Kind::Gram, &g).unwrap();
        // An executogram has no stage.
        g["type"] = "executogram".into();
        assert!(validate(Kind::Gram, &g).is_err());
    }

    #[test]
    fn stream_refers_by_name_and_to() {
        let stream = |refers_to: Value| {
            serde_json::json!({"$id": "s", "recording_actor": "a", "chronicle": "k", "events": [{
                "name": "x", "intake": "besluit", "legal_basis": ["r#1"],
                "type": "decretogram", "refers_to": refers_to, "fields": {"a": "$external.a"}
            }]})
        };
        for to in [
            serde_json::json!("algemene_wet_bestuursrecht#4:1"),
            serde_json::json!("aanvraag_ontvangen"),
            serde_json::json!({"stage": "BESLUIT"}),
        ] {
            validate(
                Kind::Stream,
                &stream(serde_json::json!({"on_application": {"to": to, "required": true}})),
            )
            .unwrap();
        }
        assert!(validate(
            Kind::Stream,
            &stream(serde_json::json!({"on_application": {}}))
        )
        .is_err());
        let mut z = stream(serde_json::json!({"decision": {"to": {"stage": "BESLUIT"}}}));
        z["events"][0]["case"] = "volgt".into();
        let errors = validate(Kind::Stream, &z).unwrap_err();
        assert!(errors.iter().any(|f| f.contains("case")), "{errors:?}");
    }
}

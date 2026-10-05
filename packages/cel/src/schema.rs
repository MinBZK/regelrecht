//! The JSON schemas from `schema/chronolex/v0.3.0/`, embedded at build
//! time. The files are the source; this module compiles them once.

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;

/// The shared definitions (`common.json`): not a document of its own; every
/// schema can refer to it as `common.json#/definitions/<name>`.
const COMMON: &str = include_str!("../../../schema/chronolex/v0.3.0/common.json");

/// Which of the schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A stream definition (`stream.json`).
    Stream,
    /// The lexostatus definitions of a cell (`lexostatus.json`).
    Lexostatus,
    /// A cell definition, `cell.yaml` (`cell.json`).
    Cell,
    /// A recorded gram (`gram.json`).
    Gram,
    /// The synthesis of the deployment, `synthesis.yaml` (`synthesis.json`).
    Synthesis,
    /// The channels of the deployment, `channels.yaml` (`channels.json`).
    Channels,
    /// The registers of the deployment, `registers.yaml` (`registers.json`).
    Registers,
    /// The examples of the deployment, `examples.yaml` (`examples.json`).
    Examples,
    /// A line of the initial state of a cell (`initial_state.json`).
    InitialState,
}

impl Kind {
    /// Every kind, in the order of the files.
    pub const ALL: [Kind; 9] = [
        Kind::Stream,
        Kind::Lexostatus,
        Kind::Cell,
        Kind::Gram,
        Kind::Synthesis,
        Kind::Channels,
        Kind::Registers,
        Kind::Examples,
        Kind::InitialState,
    ];

    /// The file name of the schema and its embedded text.
    fn file(self) -> (&'static str, &'static str) {
        macro_rules! embed {
            ($name:literal) => {
                (
                    $name,
                    include_str!(concat!("../../../schema/chronolex/v0.3.0/", $name)),
                )
            };
        }
        match self {
            Kind::Stream => embed!("stream.json"),
            Kind::Lexostatus => embed!("lexostatus.json"),
            Kind::Cell => embed!("cell.json"),
            Kind::Gram => embed!("gram.json"),
            Kind::Synthesis => embed!("synthesis.json"),
            Kind::Channels => embed!("channels.json"),
            Kind::Registers => embed!("registers.json"),
            Kind::Examples => embed!("examples.json"),
            Kind::InitialState => embed!("initial_state.json"),
        }
    }
}

fn parse(source: &str, name: &str) -> Result<Value, String> {
    serde_json::from_str(source)
        .map_err(|e| format!("embedded schema {name} is not valid JSON: {e}"))
}

/// Compile the schema of `kind`, with `common.json` registered under its
/// `$id` so references to its definitions resolve without fetching anything.
fn compile(kind: Kind) -> Result<Validator, String> {
    let (name, source) = kind.file();
    let common = parse(COMMON, "common.json")?;
    let schema = parse(source, name)?;
    let id = common["$id"]
        .as_str()
        .ok_or("embedded schema common.json has no $id")?
        .to_string();
    let registry = jsonschema::Registry::new()
        .add(id, common)
        .and_then(|r| r.prepare())
        .map_err(|e| format!("embedded schema common.json does not register: {e}"))?;
    jsonschema::options()
        .with_registry(&registry)
        .build(&schema)
        .map_err(|e| format!("embedded schema {name} does not compile: {e}"))
}

/// Every schema, compiled once, in the order of [`Kind::ALL`].
static VALIDATORS: LazyLock<Vec<Result<Validator, String>>> =
    LazyLock::new(|| Kind::ALL.iter().map(|k| compile(*k)).collect());

fn validator(kind: Kind) -> Result<&'static Validator, String> {
    let i = Kind::ALL
        .iter()
        .position(|k| *k == kind)
        .ok_or_else(|| format!("no schema for {kind:?}"))?;
    match &VALIDATORS[i] {
        Ok(v) => Ok(v),
        Err(e) => Err(e.clone()),
    }
}

/// Validate a document against one of the schemas. On failure: every
/// violation as `<path>: <message>`.
pub fn validate(kind: Kind, doc: &Value) -> Result<(), Vec<String>> {
    let validator = validator(kind).map_err(|e| vec![e])?;
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
        for kind in Kind::ALL {
            let v = validator(kind);
            assert!(v.is_ok(), "{kind:?}: {:?}", v.err());
        }
    }

    /// A cell whose lexostatuses all come from the law (RFC-048) and the
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

    /// A gram has an id (a uuid) and refers by name to another id; since
    /// v0.2.0 the schema no longer knows a case or decision reference number.
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

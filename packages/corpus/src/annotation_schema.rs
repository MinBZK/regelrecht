//! Note (annotation) YAML schema validation.
//!
//! The editor write path (`PUT /api/corpus/laws/{id}/annotations`) must
//! reject a malformed note file *before* a commit and PR are created —
//! once a branch exists, an invalid file is a dead PR someone has to clean
//! up. This is the technical half of the two-layer review: schema
//! correctness is enforced here, resolve correctness (orphaned/ambiguous
//! selectors) stays a warning in `validate-annotations` per RFC-018 §8.
//!
//! The same schema is embedded by the `validate-annotations` engine binary.
//! Keeping a second compiled copy here avoids editor-api depending on the
//! engine crate; the schema file itself is the single source of truth.

use std::sync::LazyLock;

use jsonschema::Validator;

/// The note schema, embedded at build time. Path is relative to this
/// source file: `packages/corpus/src/` → repo `schema/v0.5.2/`.
const ANNOTATION_SCHEMA: &str = include_str!("../../../schema/v0.5.2/annotation-schema.json");

/// Compiled once on first use. The embedded schema not compiling is a
/// build/release fault, not a per-request one — but the workspace bans
/// `expect()`, so init is total: the error is carried in the `Result` and
/// surfaced from [`validate_annotation_yaml`]. `schema_compiles` pins the
/// build-time guarantee that this `Err` arm is unreachable in practice.
static VALIDATOR: LazyLock<Result<Validator, String>> = LazyLock::new(|| {
    let schema: serde_json::Value = serde_json::from_str(ANNOTATION_SCHEMA)
        .map_err(|e| format!("embedded annotation schema is not valid JSON: {e}"))?;
    Validator::new(&schema).map_err(|e| format!("embedded annotation schema does not compile: {e}"))
});

/// Parse a note file's YAML and validate it against
/// `schema/v0.5.2/annotation-schema.json`, returning the parsed document on
/// success.
///
/// On failure returns a list of error strings (a YAML parse error, or the
/// schema violations as instance-path + message). Callers that surface
/// these to a client should treat them as **diagnostic detail to log**, not
/// as a safe response body: instance paths can echo attacker-controlled map
/// keys (the schema is `additionalProperties: false`, so an unknown key's
/// name lands in the path) and that flows into UI dialogs — the same
/// self-XSS vector `save_law` avoids by not echoing the request `$id`.
pub fn parse_and_validate_annotation_yaml(yaml: &str) -> Result<serde_json::Value, Vec<String>> {
    let validator = VALIDATOR.as_ref().map_err(|e| vec![e.clone()])?;

    let doc: serde_json::Value =
        serde_yaml_ng::from_str(yaml).map_err(|e| vec![format!("YAML parse error: {e}")])?;

    let errors: Vec<String> = validator
        .iter_errors(&doc)
        .map(|err| format!("{}: {}", err.instance_path(), err))
        .collect();

    if errors.is_empty() {
        Ok(doc)
    } else {
        Err(errors)
    }
}

/// Validate without keeping the parsed document. Thin wrapper over
/// [`parse_and_validate_annotation_yaml`] for callers that only need the
/// pass/fail (e.g. the `validate-annotations` style check).
pub fn validate_annotation_yaml(yaml: &str) -> Result<(), Vec<String>> {
    parse_and_validate_annotation_yaml(yaml).map(|_| ())
}

/// The law id a note's `target.source` URI refers to.
///
/// `target.source` is a `regelrecht://<law_id>[/...]` URI (RFC-005); the
/// law id is the first path segment. Mirrors `regelrecht_engine`'s
/// `annotation::law_id_from_source` so the editor write path and the engine
/// resolver agree on what a note is "about".
pub fn law_id_from_source(source: &str) -> Option<&str> {
    let rest = source.strip_prefix("regelrecht://")?;
    Some(rest.split('/').next().unwrap_or(rest))
}

/// Every distinct law id referenced by the notes in a (schema-valid)
/// document's `target.source`. Used to reject a note file whose contents
/// are about a different law than the path it is being written to —
/// the note-side analogue of `save_law`'s `$id`/path-mismatch guard.
pub fn note_target_law_ids(doc: &serde_json::Value) -> Vec<String> {
    let Some(notes) = doc.get("annotations").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = notes
        .iter()
        .filter_map(|n| {
            n.get("target")
                .and_then(|t| t.get("source"))
                .and_then(|s| s.as_str())
                .and_then(law_id_from_source)
                .map(str::to_string)
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA_URL: &str = "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.2/annotation-schema.json";

    #[test]
    fn schema_compiles() {
        // The embedded schema must be valid JSON and a compilable JSON
        // Schema. This is the build-time guarantee that the VALIDATOR
        // `Err` arm is unreachable in production.
        assert!(VALIDATOR.as_ref().is_ok(), "{:?}", VALIDATOR.as_ref().err());
    }

    #[test]
    fn valid_minimal_note_passes() {
        let yaml = format!(
            r#"
$schema: "{SCHEMA_URL}"
annotations:
  - type: Annotation
    motivation: commenting
    creator: tester
    target:
      source: "regelrecht://zorgtoeslagwet"
      selector:
        type: TextQuoteSelector
        exact: zorgtoeslag
    body:
      type: TextualBody
      value: een toelichting
      purpose: commenting
"#
        );
        assert!(validate_annotation_yaml(&yaml).is_ok());
    }

    #[test]
    fn missing_required_field_fails_with_message() {
        // No `target` — schema requires it on an Annotation.
        let yaml = format!(
            r#"
$schema: "{SCHEMA_URL}"
annotations:
  - type: Annotation
    motivation: commenting
"#
        );
        let errs = validate_annotation_yaml(&yaml).unwrap_err();
        assert!(!errs.is_empty());
    }

    #[test]
    fn malformed_yaml_reports_parse_error() {
        let errs = validate_annotation_yaml("annotations: [unterminated").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("YAML parse error"));
    }

    #[test]
    fn law_id_from_source_takes_the_host_segment() {
        assert_eq!(
            law_id_from_source("regelrecht://zorgtoeslagwet"),
            Some("zorgtoeslagwet")
        );
        assert_eq!(
            law_id_from_source("regelrecht://zorgtoeslagwet/hoogte#out"),
            Some("zorgtoeslagwet")
        );
        assert_eq!(law_id_from_source("https://example.com/x"), None);
    }

    #[test]
    fn note_target_law_ids_are_distinct_and_sorted() {
        let yaml = format!(
            r#"
$schema: "{SCHEMA_URL}"
annotations:
  - type: Annotation
    motivation: commenting
    target:
      source: "regelrecht://zorgtoeslagwet"
      selector: {{ type: TextQuoteSelector, exact: a }}
    body: {{ type: TextualBody, value: x, purpose: commenting }}
  - type: Annotation
    motivation: linking
    target:
      source: "regelrecht://andere_wet/hoogte"
      selector: {{ type: TextQuoteSelector, exact: b }}
    body: {{ type: SpecificResource, source: "regelrecht://andere_wet/x#y", purpose: linking }}
  - type: Annotation
    motivation: commenting
    target:
      source: "regelrecht://zorgtoeslagwet"
      selector: {{ type: TextQuoteSelector, exact: c }}
    body: {{ type: TextualBody, value: z, purpose: commenting }}
"#
        );
        let doc = parse_and_validate_annotation_yaml(&yaml).expect("schema-valid fixture");
        assert_eq!(
            note_target_law_ids(&doc),
            vec!["andere_wet".to_string(), "zorgtoeslagwet".to_string()]
        );
    }
}

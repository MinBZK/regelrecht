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
    let doc: serde_json::Value =
        serde_yaml_ng::from_str(yaml).map_err(|e| vec![format!("YAML parse error: {e}")])?;
    validate_annotation_doc(&doc)?;
    Ok(doc)
}

/// Validate an already-parsed sidecar document against the note schema.
///
/// Used by the editor write path, which builds the merged document
/// in-memory (base notes from the branch + new notes) and must validate
/// the *result* before it is written. Same error-handling caveat as
/// [`parse_and_validate_annotation_yaml`]: log the errors, do not echo
/// them to the client.
pub fn validate_annotation_doc(doc: &serde_json::Value) -> Result<(), Vec<String>> {
    let validator = VALIDATOR.as_ref().map_err(|e| vec![e.clone()])?;

    let errors: Vec<String> = validator
        .iter_errors(doc)
        .map(|err| format!("{}: {}", err.instance_path(), err))
        .collect();

    if errors.is_empty() {
        Ok(())
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

/// Serialise a sidecar document back to YAML for writing.
///
/// The editor write path builds the merged document as JSON in-memory;
/// the on-disk corpus format is YAML. Centralised here so the dump options
/// (block style, no anchors) stay consistent with the frontend export and
/// the committed files, keeping git diffs readable.
pub fn serialize_annotation_doc(doc: &serde_json::Value) -> Result<String, String> {
    serde_yaml_ng::to_string(doc).map_err(|e| format!("failed to serialise notes: {e}"))
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

/// Why a note's `target.source` is not an acceptable reference to the law
/// the sidecar is being written for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteTargetError {
    /// `target.source` absent or not a string (schema permits any string).
    Unparseable,
    /// `target.source` is a string but not a `regelrecht://<law_id>` URI.
    NotRegelrechtUri,
    /// Parsed to a law id, but a *different* one than expected.
    WrongLaw(String),
}

/// Reject the sidecar unless **every** note's `target.source` resolves to
/// exactly `law_id`.
///
/// This is an *allowlist*, deliberately: an earlier version collected the
/// parseable law ids and looked for a mismatch, which silently let a note
/// whose source was `https://evil/...` or absent slip through (the schema
/// only requires `target.source` to be *a string*, no `regelrecht://`
/// pattern). RFC-018 §1 keys the sidecar by law id — one file is one law's
/// notes — so anything that does not provably refer to `law_id` is
/// rejected, the note-side analogue of `save_law`'s `$id`/path guard.
/// Returns the first offending note (by array index) and why.
pub fn first_note_not_targeting_law(
    doc: &serde_json::Value,
    law_id: &str,
) -> Option<(usize, NoteTargetError)> {
    let notes = doc.get("annotations").and_then(|v| v.as_array())?;
    for (i, note) in notes.iter().enumerate() {
        let source = note
            .get("target")
            .and_then(|t| t.get("source"))
            .and_then(|s| s.as_str());
        let err = match source {
            None => Some(NoteTargetError::Unparseable),
            Some(s) => match law_id_from_source(s) {
                None => Some(NoteTargetError::NotRegelrechtUri),
                Some(id) if id == law_id => None,
                Some(id) => Some(NoteTargetError::WrongLaw(id.to_string())),
            },
        };
        if let Some(e) = err {
            return Some((i, e));
        }
    }
    None
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
    fn first_note_not_targeting_law_is_an_allowlist() {
        // All on-law → None.
        let ok = serde_json::json!({"annotations": [
            {"target": {"source": "regelrecht://zorgtoeslagwet"}},
            {"target": {"source": "regelrecht://zorgtoeslagwet/hoogte#x"}},
        ]});
        assert_eq!(first_note_not_targeting_law(&ok, "zorgtoeslagwet"), None);

        // A different law → WrongLaw at its index.
        let wrong = serde_json::json!({"annotations": [
            {"target": {"source": "regelrecht://zorgtoeslagwet"}},
            {"target": {"source": "regelrecht://andere_wet"}},
        ]});
        assert_eq!(
            first_note_not_targeting_law(&wrong, "zorgtoeslagwet"),
            Some((1, NoteTargetError::WrongLaw("andere_wet".to_string())))
        );

        // Non-regelrecht:// source must be REJECTED, not silently skipped
        // (the bypass the hostile review found).
        let evil = serde_json::json!({"annotations": [
            {"target": {"source": "https://evil/zorgtoeslagwet"}},
        ]});
        assert_eq!(
            first_note_not_targeting_law(&evil, "zorgtoeslagwet"),
            Some((0, NoteTargetError::NotRegelrechtUri))
        );

        // Absent target.source → Unparseable, also rejected.
        let bare = serde_json::json!({"annotations": [{"target": {}}]});
        assert_eq!(
            first_note_not_targeting_law(&bare, "zorgtoeslagwet"),
            Some((0, NoteTargetError::Unparseable))
        );

        // Empty / absent array is vacuously fine — the destructive-shrink
        // guard, not this function, handles an empty body.
        assert_eq!(
            first_note_not_targeting_law(&serde_json::json!({"annotations": []}), "x"),
            None
        );
        assert_eq!(
            first_note_not_targeting_law(&serde_json::json!({}), "x"),
            None
        );
    }
}

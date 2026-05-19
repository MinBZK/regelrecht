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

/// The notes already present in a sidecar's text, as JSON values.
///
/// Used to dedupe an append against what is already committed. Returns an
/// empty vec when the file has no `annotations:` sequence (or is absent).
pub fn notes_in_sidecar(base_text: &str) -> Vec<serde_json::Value> {
    serde_yaml_ng::from_str::<serde_json::Value>(base_text)
        .ok()
        .and_then(|doc| doc.get("annotations").and_then(|a| a.as_array()).cloned())
        .unwrap_or_default()
}

/// Outcome of preparing an append-only write of `new_notes` onto a sidecar.
pub enum AppendOutcome {
    /// Nothing to write: every new note was already present (dedup left
    /// zero). The caller must skip the write/commit/PR entirely so a
    /// no-op save produces no branch noise (review finding NEW-2).
    NoChange,
    /// The full text to write. Either the verbatim base with new note
    /// items appended (history/comments preserved), or a freshly built
    /// document when there was no base sequence to preserve.
    Write(String),
}

/// Append `new_notes` to a sidecar **without rewriting the existing file**.
///
/// RFC-018 Decision 1 promises `git blame` shows who added each note and
/// when; RFC-005's whole premise is that a stand-off file must not be
/// rewritten under the content it annotates. Parsing the base and
/// re-serialising it (even "losslessly") reorders keys, drops the curated
/// *motivering* comments (RFC-018 §Why / AWB 3:46), and reassigns every
/// line's blame to the last writer. So the base bytes are kept **verbatim**
/// and the new notes are appended as text.
///
/// - Base has an `annotations:` block: serialise only the *new, deduped*
///   notes as a YAML sequence, re-indent to the sidecar's 2-space list
///   convention, and append after the existing content. Existing bytes are
///   untouched, so the git diff is exactly the added lines.
/// - No base, or a base without an `annotations:` key (nothing to
///   preserve): build a fresh full document. There is no history or
///   comment to protect in that case.
/// - All new notes already present: [`AppendOutcome::NoChange`].
///
/// `new_notes` are assumed schema-checked by the caller; the caller must
/// still validate the *resulting* document (a malformed base would
/// otherwise pass through).
pub fn append_notes_to_sidecar(
    base_text: Option<&str>,
    new_notes: &[serde_json::Value],
    schema_url: &str,
) -> Result<AppendOutcome, String> {
    let base_notes: Vec<serde_json::Value> = base_text.map(notes_in_sidecar).unwrap_or_default();

    let seen: std::collections::HashSet<String> = base_notes
        .iter()
        .filter_map(|n| serde_json::to_string(n).ok())
        .collect();

    // Preserve order; dedupe new notes against the base and against each
    // other (a double-submit of the same draft is idempotent).
    let mut to_add: Vec<&serde_json::Value> = Vec::new();
    let mut added_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    for note in new_notes {
        let Ok(key) = serde_json::to_string(note) else {
            continue;
        };
        if !seen.contains(&key) && added_keys.insert(key) {
            to_add.push(note);
        }
    }

    if to_add.is_empty() {
        return Ok(AppendOutcome::NoChange);
    }

    // Decide whether there is a base sequence to preserve. We only treat
    // the base as appendable when it actually parses and carries an
    // `annotations` array; anything else is rebuilt from scratch (no
    // history/comments at stake).
    let base_has_sequence = base_text
        .and_then(|t| serde_yaml_ng::from_str::<serde_json::Value>(t).ok())
        .and_then(|d| d.get("annotations").map(|a| a.is_array()))
        .unwrap_or(false);

    if !base_has_sequence {
        // Fresh document: base + new notes, full serialise is fine here
        // because there is nothing to protect.
        let mut all = base_notes;
        all.extend(to_add.iter().map(|n| (*n).clone()));
        let doc = serde_json::json!({ "$schema": schema_url, "annotations": all });
        return Ok(AppendOutcome::Write(serialize_annotation_doc(&doc)?));
    }

    // Serialise ONLY the new notes as a sequence, then re-indent every
    // line by two spaces so the items sit under the existing
    // `annotations:` key (the sidecar's list convention). serde_yaml_ng
    // emits a top-level sequence as `- ...` at column 0 with 2-space field
    // indentation; prefixing two spaces yields `  - ...`.
    let owned: Vec<serde_json::Value> = to_add.iter().map(|n| (*n).clone()).collect();
    let seq = serde_yaml_ng::to_string(&owned)
        .map_err(|e| format!("failed to serialise new notes: {e}"))?;
    let indented: String = seq
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::from("\n")
            } else {
                format!("  {line}\n")
            }
        })
        .collect();

    let base = base_text.unwrap_or_default();
    // Exactly one newline between the existing content and the appended
    // items, regardless of whether the base ended with 0, 1 or more.
    let mut out = base.trim_end_matches('\n').to_string();
    out.push('\n');
    out.push_str(&indented);
    Ok(AppendOutcome::Write(out))
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

    // A base sidecar with the curated shape the real corpus file uses:
    // `---`, comments, an explanatory header, semantic key order, a
    // multi-line block scalar. Appending must not touch any of this.
    const CURATED_BASE: &str = "\
---
# Stand-off notes. RFC-005 / RFC-018.
$schema: https://example/schema.json
annotations:
  # Linking note: curated explanation that must survive.
  - type: Annotation
    motivation: linking
    creator: Dienst Toeslagen
    target:
      source: regelrecht://zorgtoeslagwet
      selector:
        type: TextQuoteSelector
        exact: zorgtoeslag
    body:
      type: SpecificResource
      source: regelrecht://zorgtoeslagwet/x#y
      purpose: linking
";

    fn note(exact: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "Annotation",
            "motivation": "commenting",
            "creator": "tester",
            "target": {
                "source": "regelrecht://zorgtoeslagwet",
                "selector": { "type": "TextQuoteSelector", "exact": exact }
            },
            "body": { "type": "TextualBody", "value": "x", "purpose": "commenting" }
        })
    }

    #[test]
    fn append_keeps_the_base_verbatim_and_only_adds_lines() {
        let new = vec![note("normpremie")];
        let out = match append_notes_to_sidecar(Some(CURATED_BASE), &new, "https://s") {
            Ok(AppendOutcome::Write(t)) => t,
            other => panic!(
                "expected Write, got {:?}",
                matches!(other, Ok(AppendOutcome::NoChange))
            ),
        };
        // Every original byte/line is still there, in order: the comments,
        // the $schema, the curated note. The base is a strict prefix
        // (modulo the single trailing-newline normalisation).
        assert!(out.starts_with(CURATED_BASE.trim_end_matches('\n')));
        assert!(out.contains("# Linking note: curated explanation that must survive."));
        assert!(out.contains("creator: Dienst Toeslagen"));
        // The appended note sits under `annotations:` at the 2-space list
        // convention and carries its content.
        assert!(out.contains("\n  - "));
        assert!(out.contains("normpremie"));
        // Result parses and the sequence grew by exactly one.
        let doc: serde_json::Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn append_dedups_against_the_base_and_is_idempotent() {
        // The base already contains the linking note; re-submitting an
        // identical-content note must not duplicate it. Reconstruct that
        // note exactly as it parses out of the base.
        let base_doc: serde_json::Value = serde_yaml_ng::from_str(CURATED_BASE).unwrap();
        let existing = base_doc["annotations"][0].clone();

        // Only the existing note resubmitted → nothing to write.
        assert!(matches!(
            append_notes_to_sidecar(Some(CURATED_BASE), &[existing.clone()], "https://s"),
            Ok(AppendOutcome::NoChange)
        ));

        // Existing + one genuinely new → only the new one is appended.
        let out = match append_notes_to_sidecar(
            Some(CURATED_BASE),
            &[existing, note("uniek")],
            "https://s",
        ) {
            Ok(AppendOutcome::Write(t)) => t,
            _ => panic!("expected Write"),
        };
        let doc: serde_json::Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn append_with_no_base_builds_a_fresh_document() {
        let out = match append_notes_to_sidecar(None, &[note("a")], "https://s") {
            Ok(AppendOutcome::Write(t)) => t,
            _ => panic!("expected Write"),
        };
        let doc: serde_json::Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc["$schema"], "https://s");
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn append_to_base_without_annotations_key_rebuilds() {
        // A file that parses but has no annotations sequence: nothing to
        // preserve, so a fresh full doc is fine.
        let bare = "$schema: https://old\n";
        let out = match append_notes_to_sidecar(Some(bare), &[note("a")], "https://s") {
            Ok(AppendOutcome::Write(t)) => t,
            _ => panic!("expected Write"),
        };
        let doc: serde_json::Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn append_empty_input_is_nochange() {
        assert!(matches!(
            append_notes_to_sidecar(Some(CURATED_BASE), &[], "https://s"),
            Ok(AppendOutcome::NoChange)
        ));
        assert!(matches!(
            append_notes_to_sidecar(None, &[], "https://s"),
            Ok(AppendOutcome::NoChange)
        ));
    }
}

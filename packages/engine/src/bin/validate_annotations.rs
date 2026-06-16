//! Validate note sidecar files (RFC-005, RFC-018).
//!
//! For each note file:
//! 1. JSON Schema validation against the embedded annotation schema.
//! 2. Resolve every note's selector against its target law (loaded from the
//!    corpus by `$id`). Orphaned or ambiguous notes are reported as
//!    **warnings**, not errors (RFC-018 Decision 8): law text legitimately
//!    drifts away from notes over time.
//! 3. Tagging-body values are checked against the controlled vocabulary
//!    (`corpus/annotations/_vocabulary/ambiguity.yaml`); unknown values are
//!    **warnings** (RFC-018 Decision 9).
//!
//! Exit code is non-zero only on schema validation failures.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process;

use jsonschema::Validator;
use regelrecht_engine::annotation::{law_id_from_source, resolve, TextQuoteSelector};
use regelrecht_engine::article::{ArticleBasedLaw, LawLoad};

const ANNOTATION_SCHEMA: &str = include_str!("../../../../schema/v0.5.3/annotation-schema.json");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let files: Vec<PathBuf> = if args.is_empty() {
        discover_note_files()
    } else {
        args.iter().map(PathBuf::from).collect()
    };

    if files.is_empty() {
        eprintln!("No note files found.");
        return;
    }

    let schema: serde_json::Value = match serde_json::from_str(ANNOTATION_SCHEMA) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("FATAL: embedded annotation schema is not valid JSON: {e}");
            process::exit(2);
        }
    };
    let validator = match Validator::new(&schema) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("FATAL: annotation schema does not compile: {e}");
            process::exit(2);
        }
    };

    let vocabulary = load_vocabulary();
    let mut failed = false;
    let mut warnings = 0usize;

    for path in &files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("FAIL: {}: read: {e}", path.display());
                failed = true;
                continue;
            }
        };
        let doc: serde_json::Value = match serde_yaml_ng::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("FAIL: {}: yaml parse: {e}", path.display());
                failed = true;
                continue;
            }
        };

        let errors: Vec<_> = validator.iter_errors(&doc).collect();
        if !errors.is_empty() {
            eprintln!("FAIL: {}: schema", path.display());
            for err in &errors {
                eprintln!("  - {}: {}", err.instance_path(), err);
            }
            failed = true;
            continue;
        }
        eprintln!("OK: {} (annotation schema v0.5.3)", path.display());

        warnings += check_notes(path, &doc, &vocabulary);
    }

    if warnings > 0 {
        eprintln!("\n{warnings} warning(s). Orphaned/ambiguous notes and unknown tags do not fail the build (RFC-018).");
    }
    if failed {
        process::exit(1);
    }
}

/// Resolve each note and check tag values; return the warning count.
fn check_notes(path: &Path, doc: &serde_json::Value, vocabulary: &HashSet<String>) -> usize {
    let Some(notes) = doc.get("annotations").and_then(|v| v.as_array()) else {
        return 0;
    };

    let mut warnings = 0;
    for (i, note) in notes.iter().enumerate() {
        // Resolve the selector against the target law, if we can find it.
        if let Some(law) = note
            .get("target")
            .and_then(|t| t.get("source"))
            .and_then(|s| s.as_str())
            .and_then(law_id_from_source)
            .and_then(load_law_by_id)
        {
            if let Some(selector) = note
                .get("target")
                .and_then(|t| t.get("selector"))
                .and_then(|s| serde_json::from_value::<TextQuoteSelector>(s.clone()).ok())
            {
                let result = resolve(&selector, &law.articles);
                if result.is_orphaned() {
                    eprintln!(
                        "  WARN: {} note[{i}]: orphaned (selector {:?} not found in law)",
                        path.display(),
                        selector.exact
                    );
                    warnings += 1;
                } else if result.is_ambiguous() {
                    eprintln!(
                        "  WARN: {} note[{i}]: ambiguous ({} matches for {:?}; add prefix/suffix)",
                        path.display(),
                        result.matches.len(),
                        selector.exact
                    );
                    warnings += 1;
                }
            }
        }

        // Check tagging-body values against the controlled vocabulary.
        for tag in tagging_values(note) {
            if !vocabulary.contains(&tag) {
                eprintln!(
                    "  WARN: {} note[{i}]: tag {tag:?} not in _vocabulary/ambiguity.yaml",
                    path.display()
                );
                warnings += 1;
            }
        }
    }
    warnings
}

/// Collect every `TextualBody` value whose `purpose` is `tagging`.
fn tagging_values(note: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let bodies = match note.get("body") {
        Some(serde_json::Value::Array(a)) => a.clone(),
        Some(other) => vec![other.clone()],
        None => return out,
    };
    for body in bodies {
        let is_tag = body.get("purpose").and_then(|p| p.as_str()) == Some("tagging");
        if is_tag {
            if let Some(v) = body.get("value").and_then(|v| v.as_str()) {
                out.push(v.to_string());
            }
        }
    }
    out
}

/// Repo root: two levels up from this crate (`packages/engine`).
///
/// `CARGO_MANIFEST_DIR` is `<repo>/packages/engine`, so two `..` segments
/// always yield the repo root; this never fails at runtime.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// All `corpus/annotations/**/annotations.yaml` files (skips `_vocabulary`).
fn discover_note_files() -> Vec<PathBuf> {
    let dir = repo_root().join("corpus/annotations");
    let mut out = Vec::new();
    collect_yaml(&dir, &mut out);
    out.sort();
    out
}

fn collect_yaml(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().and_then(|n| n.to_str()) == Some("_vocabulary") {
                continue;
            }
            collect_yaml(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("yaml") {
            out.push(p);
        }
    }
}

/// Load the latest version of a law identified by its `$id` from the corpus.
///
/// Scans `corpus/regulation/` for a YAML whose `$id` matches, preferring the
/// lexicographically last filename (latest `valid_from`).
fn load_law_by_id(law_id: &str) -> Option<ArticleBasedLaw> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    collect_law_yaml(
        &repo_root().join("corpus/regulation"),
        law_id,
        &mut candidates,
    );
    candidates.sort();
    let path = candidates.last()?;
    ArticleBasedLaw::from_yaml_file(path).ok()
}

fn collect_law_yaml(dir: &Path, law_id: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_law_yaml(&p, law_id, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("yaml") {
            // Parse the top-level `$id` rather than substring-matching the
            // file: a comment or a nested string containing "$id: x" would
            // otherwise produce a false positive.
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(doc) = serde_yaml_ng::from_str::<serde_json::Value>(&content) {
                    if doc.get("$id").and_then(|v| v.as_str()) == Some(law_id) {
                        out.push(p);
                    }
                }
            }
        }
    }
}

/// Load the ambiguity vocabulary `id`s. Missing file means an empty set
/// (every tag will warn), which surfaces the misconfiguration.
fn load_vocabulary() -> HashSet<String> {
    let path = repo_root().join("corpus/annotations/_vocabulary/ambiguity.yaml");
    let Ok(content) = std::fs::read_to_string(&path) else {
        eprintln!("WARN: vocabulary not found at {}", path.display());
        return HashSet::new();
    };
    let doc: serde_json::Value = match serde_yaml_ng::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("WARN: vocabulary parse error: {e}");
            return HashSet::new();
        }
    };
    doc.get("ambiguity")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|it| it.get("id").and_then(|v| v.as_str()))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

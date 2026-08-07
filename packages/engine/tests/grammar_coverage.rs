//! Grammar coverage: every canonical BDD step must be exercised.
//!
//! `bdd/README.md` claims bucket B proves an engine speaks the *whole* canonical
//! language. Nothing enforced that: a step could be declared in
//! `bdd/grammar.yaml`, code-generated into a binding, and never appear in a
//! single feature file — so gutting its dispatch arm left the suite green.
//!
//! This test closes that hole. It reads the grammar, translates each step's
//! canonical `text` with the very same `to_regex` the codegen uses (included
//! from `build_codegen/`, so the two can never drift), and requires a match in
//! at least one feature file across both buckets.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use regex::Regex;
use walkdir::WalkDir;

// The codegen's grammar model + `to_regex`. Wrapped in a module so the items
// this test does not call (`to_cucumber_expr`, `needs_regex`) stay quiet.
#[allow(dead_code, clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod grammar_model {
    include!("../build_codegen/grammar_model.rs");
}

use grammar_model::{load_grammar, to_regex};

/// Steps that are declared in the grammar but not yet exercised by any
/// scenario. Each entry needs a synthetic law that demonstrably *should* produce
/// the shape being asserted (a string value, a null, a substring), and picking
/// that law is a modelling judgement rather than a mechanical fix — so the gap
/// is recorded here instead of silently tolerated.
///
/// A new undocumented gap fails this test; so does a stale entry that is now
/// covered. See issue 1181.
const UNCOVERED: &[&str] = &["assert_equals_string", "assert_null", "assert_contains"];

/// Repo root, derived from this crate's manifest dir (`packages/engine`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalize repo root")
}

/// Every `.feature` file in both buckets: engine conformance (bucket B) and the
/// law-validation scenarios next to the corpus (bucket A).
fn feature_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for base in [root.join("bdd/conformance"), root.join("corpus/regulation")] {
        for entry in WalkDir::new(&base).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("feature") {
                files.push(path.to_path_buf());
            }
        }
    }
    files
}

/// Gherkin step keywords, longest first so `Given`/`When` are stripped before
/// the bare `*` form is considered.
const KEYWORDS: &[&str] = &["Given ", "When ", "Then ", "And ", "But ", "* "];

/// The step text of a Gherkin line, or `None` when the line is not a step.
fn step_text(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    KEYWORDS
        .iter()
        .find_map(|kw| trimmed.strip_prefix(kw))
        .map(str::trim)
}

#[test]
fn every_grammar_step_is_exercised_by_a_scenario() {
    let root = repo_root();
    let grammar = load_grammar(&root.join("bdd/grammar.yaml"));

    let files = feature_files(&root);
    assert!(
        !files.is_empty(),
        "no feature files found under {} — paths wrong?",
        root.display()
    );

    let mut lines: Vec<String> = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for line in content.lines() {
            if let Some(text) = step_text(line) {
                lines.push(text.to_string());
            }
        }
    }
    assert!(!lines.is_empty(), "feature files contain no steps");

    let mut missing: Vec<&str> = Vec::new();
    let mut stale: Vec<&str> = Vec::new();

    for step in &grammar.steps {
        let pattern = to_regex(step);
        let re = Regex::new(&pattern).unwrap_or_else(|e| {
            panic!(
                "step '{}' yields an invalid regex {pattern:?}: {e}",
                step.id
            )
        });
        let covered = lines.iter().any(|line| re.is_match(line));
        let excused = UNCOVERED.contains(&step.id.as_str());

        match (covered, excused) {
            (false, false) => missing.push(&step.id),
            (true, true) => stale.push(&step.id),
            _ => {}
        }
    }

    assert!(
        missing.is_empty(),
        "grammar steps that no scenario exercises: {missing:?}. \
         Write a scenario for them, or add them to UNCOVERED with a reason."
    );
    assert!(
        stale.is_empty(),
        "grammar steps listed in UNCOVERED that are now exercised: {stale:?}. \
         Remove them from the list."
    );
}

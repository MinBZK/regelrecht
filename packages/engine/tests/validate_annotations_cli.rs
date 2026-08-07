//! End-to-end behaviour of the `validate-annotations` binary.
//!
//! The contract this suite pins down is the one RFC-018 gives the tool and the
//! one `just validate-annotations` relies on in `just check`: a schema
//! violation fails the build, and no note finding does. Orphaned notes,
//! ambiguous selectors and tag values outside the controlled vocabulary are
//! reported as warnings and leave the exit code at zero (RFC-018 Decisions 8
//! and 9). A run that cannot see the corpus at all is a misconfiguration and
//! exits 2 — it must never pass for a clean corpus.
//!
//! Only built with the `validate` feature, which is what gates the binary.

// Test code: unwrap/expect/panic is how a failure is reported here.
// Clippy's `allow-*-in-tests` covers `#[cfg(test)]` modules, not an
// integration test crate, so the allowance is spelled out per file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "validate")]

use std::path::PathBuf;
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/annotations")
        .join(name)
}

fn run_on(fixture_name: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_validate-annotations"))
        .arg(fixture(fixture_name))
        .output()
        .expect("validate-annotations runs")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The binary used to bake the build machine's repo path into itself; run
/// anywhere else it scanned nothing, printed "No note files found." and exited
/// 0 — indistinguishable from a clean corpus. Point it at a corpus-less root
/// and it must refuse, not succeed.
#[test]
fn without_a_findable_corpus_the_run_fails_instead_of_looking_clean() {
    let empty = std::env::temp_dir().join(format!(
        "validate-annotations-cli-no-corpus-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&empty).expect("temp dir");

    let output = Command::new(env!("CARGO_BIN_EXE_validate-annotations"))
        .current_dir(&empty)
        .env("REGELRECHT_REPO_ROOT", &empty)
        .output()
        .expect("validate-annotations runs");
    let stderr = stderr_of(&output);
    let _ = std::fs::remove_dir_all(&empty);

    assert_eq!(
        output.status.code(),
        Some(2),
        "a run without a corpus must exit 2, stderr:\n{stderr}"
    );
    assert!(stderr.contains("FATAL"), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("No note files found"),
        "must not report an empty scan as the outcome, stderr:\n{stderr}"
    );
}

#[test]
fn a_schema_violation_fails_the_build() {
    let output = run_on("schema-invalid.yaml");
    assert_eq!(
        output.status.code(),
        Some(1),
        "schema violation must exit 1, stderr:\n{}",
        stderr_of(&output)
    );
    let stderr = stderr_of(&output);
    assert!(stderr.contains("FAIL"), "stderr:\n{stderr}");
    assert!(stderr.contains("schema"), "stderr:\n{stderr}");
}

#[test]
fn a_valid_file_without_findings_passes_silently() {
    let output = run_on("known-tag.yaml");
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(0), "stderr:\n{stderr}");
    assert!(stderr.contains("OK:"), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("warning(s)"),
        "no warning summary expected, stderr:\n{stderr}"
    );
}

#[test]
fn an_unknown_tag_is_reported_as_one_warning_and_does_not_fail() {
    let output = run_on("unknown-tag.yaml");
    let stderr = stderr_of(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "a warning must not fail the build (RFC-018), stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("1 warning(s)"),
        "expected exactly one warning, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("deze-tag-staat-niet-in-de-vocabulaire"),
        "the offending tag must be named, stderr:\n{stderr}"
    );
}

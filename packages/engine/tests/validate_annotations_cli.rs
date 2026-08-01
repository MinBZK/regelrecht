//! End-to-end behaviour of the `validate-annotations` binary.
//!
//! The contract this suite pins down is the one RFC-018 gives the tool and the
//! one `just validate-annotations` relies on in `just check`: a schema
//! violation fails the build, and nothing else does. Orphaned notes, ambiguous
//! selectors and tag values outside the controlled vocabulary are reported as
//! warnings and leave the exit code at zero (RFC-018 Decisions 8 and 9).
//!
//! Only built with the `validate` feature, which is what gates the binary.
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

//! Gedragstest voor de `validate`-binary: de corpuspoort van pre-commit en CI.
//!
//! Wat hier vastligt is de afloopcode, niet de opmaak van de meldingen. `just
//! validate` (en daarmee de pre-commit-hook) beslist op die code of een
//! wijziging in `corpus/regulation/` mag landen; een `validate` die stilletjes
//! 0 teruggeeft op een kapot wetsbestand is een open poort.
//!
//! Alleen gebouwd met de `validate`-feature, net als de binary zelf (die heeft
//! `required-features = ["validate"]`). `just test` draait met `--all-features`.
#![cfg(feature = "validate")]

use std::path::PathBuf;
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/validate_cli/{name}"))
}

fn run(args: &[PathBuf]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_validate"))
        .args(args)
        .output()
        .expect("validate-binary kon niet starten")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Zonder bestanden is er niets gevalideerd, en dat is geen goedkeuring.
#[test]
fn zonder_argumenten_is_het_een_gebruiksfout() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("Usage"),
        "stderr: {}",
        stderr(&output)
    );
}

/// Een schema-geldig, unit-schoon bestand komt door de poort.
#[test]
fn geldig_bestand_slaagt() {
    let output = run(&[fixture("valid.yaml")]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("OK:"),
        "stderr: {}",
        stderr(&output)
    );
}

/// Een schemaovertreding die het lenient law-model wél accepteert (ontbrekende
/// top-level `url`) moet alsnog een afkeuring opleveren.
#[test]
fn schemaovertreding_faalt() {
    let output = run(&[fixture("schema_violation.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("FAIL"), "stderr: {err}");
    assert!(err.contains("schema"), "stderr: {err}");
    assert!(!err.contains("OK:"), "stderr: {err}");
}

/// Een bestand zonder `$schema` is niet te valideren en gaat er dus uit.
#[test]
fn ontbrekend_schemaveld_faalt() {
    let output = run(&[fixture("no_schema_field.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("missing $schema field"), "stderr: {err}");
}

/// De unit-controle (RFC-023) draait ná een geslaagde schemavalidatie en telt
/// mee in de afloopcode: eurocent bij dagen optellen is een FAIL, geen WARN.
#[test]
fn unit_mismatch_in_geldig_schema_faalt() {
    let output = run(&[fixture("unit_mismatch.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("units:"), "stderr: {err}");
    // Het schema keurde het bestand goed; de afkeuring komt van de units.
    assert!(err.contains("OK:"), "stderr: {err}");
}

/// Eén kapot bestand tussen goede bestanden bepaalt de afloopcode; de andere
/// bestanden worden nog wel gecontroleerd.
#[test]
fn een_kapot_bestand_kleurt_de_hele_run() {
    let output = run(&[fixture("valid.yaml"), fixture("schema_violation.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("OK:"), "stderr: {err}");
    assert!(err.contains("FAIL"), "stderr: {err}");
}

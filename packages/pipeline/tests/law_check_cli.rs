//! What the exit code of `law-check` means.
//!
//! The binary exists to gate a pipeline step, and a gate is only worth its
//! exit code. Schema errors have always turned it red; every other finding —
//! a unit clash, an uncovered lid, a binding into a law the corpus does not
//! have — was printed and then exited 0, so a step wired to this binary
//! passed over all of them. `--strict` is the switch that makes the run green
//! only when the report is empty, and these tests hold both halves: without
//! the flag nothing changes, with it a finding is enough.

use std::path::Path;
use std::process::Command;

/// A schema-valid law that carries a finding: article 1 has statutory text
/// and no model at all, which the `accounted` check reports.
const LAW_WITH_FINDINGS: &str = r#"$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.6/schema/v0.5.6/schema.json
$id: testwet
regulatory_layer: WET
publication_date: '2025-12-20'
valid_from: '2026-01-01'
bwb_id: BWBR0000001
url: https://example.com/BWBR0000001
name: Testwet
competent_authority: Onze Minister
articles:
  - number: '1'
    text: De minister stelt jaarlijks de standaardpremie vast, tenzij artikel 2 anders bepaalt.
    url: https://example.com/BWBR0000001#Artikel1
"#;

fn run(args: &[&str]) -> (Option<i32>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_law-check"))
        .args(args)
        .output()
        .expect("the binary is built by cargo test");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code(), text)
}

fn law_file(dir: &Path, yaml: &str) -> std::path::PathBuf {
    let path = dir.join("2026-01-01.yaml");
    std::fs::write(&path, yaml).expect("write law");
    path
}

/// The default is unchanged: findings are printed, the run stays green.
/// A step wired to this binary today must not turn red on an upgrade.
#[test]
fn test_a_finding_without_strict_still_exits_zero() {
    let dir = tempfile::tempdir().expect("tempdir");
    let law = law_file(dir.path(), LAW_WITH_FINDINGS);

    let (code, text) = run(&[law.to_str().expect("utf-8 path")]);
    assert_eq!(code, Some(0), "{text}");
    assert!(text.contains("schema: valid"), "{text}");
    // The finding is reported, which is the whole reason the exit code is
    // worth widening.
    assert!(text.contains("accounted"), "{text}");
}

/// With the flag, the same file is red, and for the finding rather than for
/// the schema.
#[test]
fn test_strict_turns_a_finding_into_a_red_exit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let law = law_file(dir.path(), LAW_WITH_FINDINGS);

    let (code, text) = run(&["--strict", law.to_str().expect("utf-8 path")]);
    assert_eq!(code, Some(1), "{text}");
    assert!(text.contains("schema: valid"), "{text}");
}

/// A file with nothing to report is green either way, so `--strict` cannot
/// be turned on over a clean corpus and immediately block it.
#[test]
fn test_a_clean_file_is_green_with_and_without_strict() {
    let dir = tempfile::tempdir().expect("tempdir");
    let law = law_file(
        dir.path(),
        r#"$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.6/schema/v0.5.6/schema.json
$id: testwet
regulatory_layer: WET
publication_date: '2025-12-20'
valid_from: '2026-01-01'
bwb_id: BWBR0000001
url: https://example.com/BWBR0000001
name: Testwet
competent_authority: Onze Minister
articles: []
"#,
    );
    let path = law.to_str().expect("utf-8 path");

    let (plain, plain_text) = run(&[path]);
    let (strict, strict_text) = run(&["--strict", path]);
    assert_eq!(plain, Some(0), "{plain_text}");
    assert_eq!(strict, Some(0), "{strict_text}");
    assert!(strict_text.contains("nothing to report"), "{strict_text}");
}

/// Schema errors were red before the flag existed and stay red without it.
#[test]
fn test_a_schema_error_is_red_with_or_without_strict() {
    let dir = tempfile::tempdir().expect("tempdir");
    let law = law_file(dir.path(), "articles: []\n");
    let path = law.to_str().expect("utf-8 path");

    let (plain, plain_text) = run(&[path]);
    let (strict, strict_text) = run(&["--strict", path]);
    assert_eq!(plain, Some(1), "{plain_text}");
    assert_eq!(strict, Some(1), "{strict_text}");
}

/// The flag is documented where someone looks for it.
#[test]
fn test_help_documents_the_strict_flag() {
    let (code, text) = run(&["--help"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(text.contains("--strict"), "{text}");
    assert!(
        text.contains("exit non-zero on any finding"),
        "the help must say what the flag does, not only that it exists: {text}"
    );
    // The usage line names it too, so `law-check` with no arguments shows it.
    let (code, usage) = run(&[]);
    assert_eq!(code, Some(2), "{usage}");
    assert!(usage.contains("[--strict]"), "{usage}");
}

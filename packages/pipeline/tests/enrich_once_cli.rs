//! What `enrich-once` reports to its caller when it refuses to run.
//!
//! The binary is wiring and most of it needs an agent, a corpus and a model to
//! reach. The exit code does not: every refusal in `main` has to arrive as a
//! failing status, because this command is run from scripts and from a shell
//! loop over a list of laws, and a refusal that exits zero is a run that looks
//! finished and enriched nothing.

use std::process::Command;

fn run(args: &[&str]) -> (Option<i32>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_enrich-once"))
        .args(args)
        .output()
        .expect("the binary is built by cargo test");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// An incomplete invocation is refused before anything is read or written.
#[test]
fn test_a_missing_argument_exits_two() {
    let (code, stderr) = run(&["--law", "regulation/nl/wet/wet_a/2026-01-01.yaml"]);
    assert_eq!(code, Some(2), "stderr: {stderr}");
    assert!(
        stderr.contains("--corpus"),
        "the refusal must name what is missing: {stderr}"
    );
}

/// An option the binary does not have is a typo in a script, not something to
/// carry on past with a default.
#[test]
fn test_an_unknown_option_exits_two() {
    let (code, stderr) = run(&["--corpus", ".", "--law", "x.yaml", "--diepte", "1"]);
    assert_eq!(code, Some(2), "stderr: {stderr}");
    assert!(
        stderr.contains("--diepte"),
        "the refusal must name the option it did not recognise: {stderr}"
    );
}

/// `--help` is an exit too, and it is not a successful run.
#[test]
fn test_help_prints_the_usage_line_and_exits_two() {
    let (code, stderr) = run(&["--help"]);
    assert_eq!(code, Some(2), "stderr: {stderr}");
    assert!(stderr.contains("usage: enrich-once"), "stderr: {stderr}");
}

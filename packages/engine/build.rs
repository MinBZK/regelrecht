// Build script: panicking on a malformed/missing grammar is the correct
// behavior (it must fail the build loudly), so the workspace's deny-by-default
// panic/unwrap/expect clippy lints are allowed here. This `#![allow]` also
// covers the two include!'d codegen files, which compile as part of this crate.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

include!("../../bdd/codegen/grammar_model.rs");
include!("../../bdd/codegen/gen_rust.rs");

fn main() {
    let grammar_path = Path::new("../../bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=../../bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=../../bdd/codegen/grammar_model.rs");
    println!("cargo:rerun-if-changed=../../bdd/codegen/gen_rust.rs");

    let grammar = load_grammar(grammar_path);

    // Sanity-check the translation logic at build time. Build scripts cannot
    // host #[test]s, so these asserts pin the expr/regex output for known steps
    // (the same invariants are unit-tested in grammar_model.rs when it is
    // include!'d into the test crate). If the grammar is renamed/removed these
    // lookups would fail — but the grammar is normative and version-pinned.
    sanity_check(&grammar);

    let code = emit_rust(&grammar);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("bdd_generated_steps.rs");
    std::fs::write(&dest, code).expect("write generated steps");
}

fn sanity_check(grammar: &Grammar) {
    let find = |id: &str| {
        grammar
            .steps
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("grammar missing expected step id '{id}'"))
    };

    // Quoted-string-only step -> Cucumber Expression.
    let date = find("set_calculation_date");
    assert!(
        !needs_regex(date),
        "set_calculation_date should not need regex"
    );
    assert_eq!(
        to_cucumber_expr(date),
        "the calculation date is {string}",
        "set_calculation_date expr mismatch"
    );

    // Numeric step -> anchored regex with string + numeric captures.
    let eq = find("assert_equals_number");
    assert!(needs_regex(eq), "assert_equals_number should need regex");
    assert_eq!(
        to_regex(eq),
        r#"^output "([^"]*)" equals (-?\d+(?:\.\d+)?)$"#,
        "assert_equals_number regex mismatch"
    );

    // Step with two numeric captures.
    let pos = find("set_note_hint_position");
    assert_eq!(
        to_regex(pos),
        r#"^the note hints article "([^"]*)" at position (-?\d+(?:\.\d+)?) to (-?\d+(?:\.\d+)?)$"#,
        "set_note_hint_position regex mismatch"
    );
}

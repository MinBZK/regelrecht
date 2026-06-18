// Build script: generates the cucumber BDD step bindings from bdd/grammar.yaml.
//
// The codegen sources live INSIDE this crate (build_codegen/) so the build
// script compiles in any context that copies `packages/engine/` — including the
// slim Docker build contexts for the WASM module and the admin/pipeline images,
// which do NOT copy the repo-root `bdd/` directory.
//
// The grammar itself (bdd/grammar.yaml) is only present in full-repo contexts
// (local dev + CI). When it is absent (Docker library/WASM builds), there is
// nothing to generate and nothing that needs it — the generated bindings are
// consumed solely by the `bdd` integration test, which those builds never
// compile — so we write an empty stub and move on instead of failing the build.
//
// Panicking on a MALFORMED grammar is still correct (fail loudly), so the
// workspace's deny-by-default panic/unwrap/expect clippy lints are allowed here.
// This `#![allow]` also covers the two include!'d codegen files.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

include!("build_codegen/grammar_model.rs");
include!("build_codegen/gen_rust.rs");

fn main() {
    let grammar_path = Path::new("../../bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=../../bdd/grammar.yaml");
    println!("cargo:rerun-if-changed=build_codegen/grammar_model.rs");
    println!("cargo:rerun-if-changed=build_codegen/gen_rust.rs");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("bdd_generated_steps.rs");

    // No grammar in this build context (e.g. the WASM/admin/pipeline Docker
    // builds copy only `packages/`): emit an empty stub so any `include!` of
    // the generated file still resolves, and skip generation. The `bdd` test is
    // never compiled in those contexts, so the stub is never actually used.
    if !grammar_path.exists() {
        println!(
            "cargo:warning=bdd/grammar.yaml not found ({}); skipping BDD step codegen",
            grammar_path.display()
        );
        std::fs::write(
            &dest,
            "// @generated: bdd/grammar.yaml absent — no steps.\n",
        )
        .expect("write empty generated steps stub");
        return;
    }

    let grammar = load_grammar(grammar_path);

    // Sanity-check the translation logic at build time. Build scripts cannot
    // host #[test]s, so `sanity_check` pins the expr/regex output for known
    // steps via runtime asserts that fire on every build. A malformed grammar
    // that breaks these is a real error and should fail the build.
    sanity_check(&grammar);

    let code = emit_rust(&grammar);
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

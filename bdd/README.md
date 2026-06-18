# bdd/ — canonical BDD feature language

`grammar.yaml` is the single source of truth for regelrecht's law-agnostic
Gherkin vocabulary. Step bindings for every engine are GENERATED from it:

- Rust: `packages/engine/build.rs` → `$OUT_DIR/bdd_generated_steps.rs`
- JS:   `bdd/codegen/gen-js.mjs` → `frontend/src/gherkin/grammar.generated.js`

Never hand-edit a generated file. Change `grammar.yaml`, then run
`just bdd-codegen` (which regenerates both and is checked in CI).

## Two buckets, one language
- **Bucket A — law validation**: `corpus/regulation/**/scenarios/*.feature`.
  Run against the LIVE laws. A failure means a law changed or the scenario is
  stale — a human decides. Tier `core` only.
- **Bucket B — engine conformance**: `bdd/conformance/*.feature`. Prove an
  engine speaks the whole language (incl. `notes`, `untranslatable`,
  `provenance` tiers) against synthetic `test_*` laws.

## Tiers
`core` (all engines), `notes`, `untranslatable`, `provenance`. A feature's
required tiers come from its `@tier:<name>` tags (untagged = `core`). A runner
only runs features whose tiers it supports.

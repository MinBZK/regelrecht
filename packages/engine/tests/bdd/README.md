# Rust BDD Tests

This directory contains the Rust engine's Cucumber/Gherkin BDD tests. The step
bindings are **code-generated** from the canonical, engine-agnostic grammar at
`bdd/grammar.yaml` (repo root) — see `bdd/README.md` for the language itself.

## Running the Tests

```bash
# Via just (runs both buckets)
just bdd

# Or directly via cargo
cd packages/engine && cargo test --test bdd -- --nocapture
```

## Architecture

```
tests/bdd/
├── main.rs                    # Runner: globs both feature buckets, runs cucumber (skips @wip)
├── world.rs                   # RegelrechtWorld — generic, law-agnostic test state
├── dispatch.rs                # ArgValue + World::dispatch — the single hand-written
│                              #   home for all step semantics (every grammar action)
└── helpers/
    ├── mod.rs                 # Helper module
    ├── regulation_loader.rs   # Loads all corpus YAML regulations
    └── value_conversion.rs    # Gherkin value type conversion
```

The `#[given]/#[when]/#[then]` step functions themselves are **generated** at
build time by `packages/engine/build.rs` (from `packages/engine/build_codegen/`)
into `$OUT_DIR/bdd_generated_steps.rs`, which `main.rs` includes. Each generated
step parses its captures and calls `World::dispatch(action, args, table)`. To add
or change a step phrasing, edit `bdd/grammar.yaml` — never hand-edit generated
code. There are no hand-written, per-law step files.

## Feature Files (two buckets)

The runner discovers feature files from both buckets and runs them through the
same generated bindings:

- **Bucket A — law validation**: `corpus/regulation/**/scenarios/*.feature`.
  Run against the live laws; a failure means a law changed or the scenario is
  stale (a human decides). Scenarios documenting a not-yet-implemented engine
  behavior are tagged `@wip` and skipped.
- **Bucket B — engine conformance**: `bdd/conformance/*.feature`, tagged by
  capability tier (`@tier:notes`, `@tier:untranslatable`, `@tier:provenance`;
  untagged = `core`). These exercise the whole language against synthetic
  `test_*` laws.

See `bdd/README.md` for the grammar, tiers, and codegen details.

---
title: "Testing"
description: "The testing strategies RegelRecht uses, led by Gherkin BDD scenarios run with cucumber-rs."
---

RegelRecht uses several testing strategies.

## BDD Tests (Behavior-Driven Development)

The primary testing approach uses Gherkin feature files executed by [cucumber-rs](https://github.com/cucumber-rs/cucumber).

### Two buckets, one language

Feature files live in two places, and the difference decides who fixes a failure:

- **Bucket A, law validation**: `corpus/regulation/**/scenarios/*.feature`, run against the real corpus. A failure means a law changed or the scenario went stale, and a human decides which. Scenarios tagged `@wip` are skipped.
- **Bucket B, engine conformance**: `bdd/conformance/*.feature`, tagged by tier, proving an engine speaks the whole language against synthetic `test_*` laws. CI blocks on this bucket.

### The step vocabulary is generated, not hand-written

`bdd/grammar.yaml` is the single source of truth for the step phrasings. The bindings for every engine are generated from it: Rust through `packages/engine/build.rs`, the editor and demo JavaScript through `bdd/codegen/gen-js.mjs`. Never hand-edit a generated file. Change `grammar.yaml` and run `just bdd-codegen`.

A step that is not in `grammar.yaml` does not exist. A minimal scenario:

```gherkin
Feature: Healthcare allowance

  Scenario: An adult with an active policy is entitled to zorgtoeslag
    Given the calculation date is "2025-01-01"
    Given law "zorgverzekeringswet" is loaded
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731
```

Laws that need source data (BRP, Belastingdienst, and the like) provide it with a data-table step keyed on the identifier the law looks up:

```gherkin
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres |
      | 999993653 | 2005-01-01    | Amsterdam      |
```

See `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature` for a complete, data-driven example.

### Running BDD Tests

```bash
just bdd                          # Both buckets
BDD_BUCKET=conformance just bdd   # Engine conformance only
BDD_BUCKET=corpus just bdd        # Law validation only
just bdd-demo                     # Bucket A over the demo corpus
just bdd-trace                    # With execution traces in trace_output/
```

### Deriving Tests from Legislative Intent

Test scenarios are derived from the **Memorie van Toelichting** (MvT), the explanatory memorandum that accompanies Dutch legislation. The MvT contains examples and reasoning from the legislature that serve as ground truth for expected behavior.

## Everything at once

`just check` runs what CI runs: formatting, lints, a build check, schema and annotation validation, the script test suites, and the full Rust test suite. Run it before pushing.

## Unit Tests

Rust unit tests cover the engine internals:

```bash
just test           # Whole workspace (needs Docker)
just test-no-docker # Everything that needs no external services
just test-db        # Only the container-backed crates
```

## Conformance

`just conformance` proves the Rust `law-model` conforms to the hand-authored JSON schema, which is the canonical contract. It is a corpus differential plus synthetic valid and invalid fixtures, the structural twin of the BDD conformance bucket. See [Conformance](../reference/conformance) and `packages/engine/tests/conformance/README.md`.

```bash
just conformance
```

## Mutation Testing

`cargo-mutants` checks whether the tests actually pin behavior or merely execute it. CI runs the diff variant as a gate on changed code.

```bash
just mutants       # Full run
just mutants-diff  # Only what this branch changed
```

## End-to-End

`just test-e2e` builds the engine to WASM and drives the editor with Playwright.

## Schema Validation

All law YAML files are validated against the JSON schema:

```bash
just validate                     # Validate all
just validate path/to/law.yaml    # Validate a specific file
just validate-annotations         # Validate stand-off notes
just validate-demo                # Validate the demo corpus
```

## The Demo

`just demo-check` checks the demo end to end: the laws, their scenarios, the frontend tests, and the WASM build. Run it before pushing anything demo-related.

## Pipeline Tests

```bash
just pipeline-test               # Unit tests (no Docker)
just pipeline-integration-test   # Integration tests (requires Docker)
```

## Benchmarks

Performance benchmarks using Criterion:

```bash
just bench                       # Run all benchmarks
just bench-save baseline-name    # Save a baseline
just bench-compare baseline-name # Compare against baseline
```

---
title: "Testing"
description: "The testing strategies RegelRecht uses, led by Gherkin BDD scenarios run with cucumber-rs."
---

RegelRecht uses multiple testing strategies to ensure correctness.

## BDD Tests (Behavior-Driven Development)

The primary testing approach uses Gherkin feature files executed by [cucumber-rs](https://github.com/cucumber-rs/cucumber).

### Feature Files

Located in `features/`, these describe expected law behavior. Use the step phrasings the cucumber-rs suite actually defines (`packages/engine/tests/bdd/steps/`); a minimal scenario looks like:

```gherkin
Feature: Healthcare allowance

  Scenario: Output is present for an eligible person
    Given the calculation date is "2025-01-01"
    When the law "wet_op_de_zorgtoeslag" is executed for outputs "hoogte_zorgtoeslag"
    Then the execution succeeds
    And the output "hoogte_zorgtoeslag" is "123400"
```

Laws that need source data (BRP, Belastingdienst, etc.) provide it with data-table steps such as `Given the following RVIG "personal_data" data:`. See `features/zorgtoeslag.feature` for a complete, data-driven example.

### Running BDD Tests

```bash
just bdd
```

### Deriving Tests from Legislative Intent

Test scenarios are derived from the **Memorie van Toelichting** (MvT), the explanatory memorandum that accompanies Dutch legislation. The MvT contains examples and reasoning from the legislature that serve as ground truth for expected behavior.

## Unit Tests

Rust unit tests cover the engine internals:

```bash
just test
```

## Schema Validation

All law YAML files are validated against the JSON schema:

```bash
just validate                    # Validate all
just validate path/to/law.yaml   # Validate specific file
```

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

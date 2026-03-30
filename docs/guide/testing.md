# Testing

RegelRecht uses multiple testing strategies to ensure correctness.

## BDD Tests (Behavior-Driven Development)

The primary testing approach uses Gherkin feature files executed by [cucumber-rs](https://github.com/cucumber-rs/cucumber).

### Feature Files

Located in `features/`, these describe expected law behavior:

```gherkin
Feature: Kinderbijslag

  Scenario: Parent with two children receives kinderbijslag
    Given the law "wet_kinderbijslag"
    And input "aantal_kinderen" is 2
    When the law is executed
    Then output "heeft_recht" should be true
```

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

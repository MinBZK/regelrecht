# Rust BDD Tests

This directory contains Cucumber/Gherkin BDD tests for the Rust engine.

## Running the Tests

```bash
# Via just
just rust-bdd

# Or directly via cargo
cd packages/engine && cargo test --test bdd -- --nocapture
```

## Current Status

**17 of 17 scenarios pass** — all BDD scenarios pass with the IoC (`open_terms` + `implements`) pattern.

### Scenarios

- **Bijstand** (10 scenarios) — Participatiewet eligibility and benefit calculation, including municipal adjustments via IoC
- **Erfgrensbeplanting** (4 scenarios) — BW 5:42 boundary planting with municipal overrides via IoC, including defaults
- **Zorgtoeslag** (3 scenarios) — Healthcare allowance with cross-law resolution and ministerial regulation via IoC

## Architecture

```
tests/bdd/
├── main.rs                    # Test runner entry point
├── world.rs                   # World struct with test state
├── steps/
│   ├── mod.rs                 # Module exports
│   ├── given.rs               # Given step definitions
│   ├── when.rs                # When step definitions
│   └── then.rs                # Then step definitions
└── helpers/
    ├── mod.rs                 # Helper module
    ├── regulation_loader.rs   # Loads all YAML regulations
    └── value_conversion.rs    # Gherkin value type conversion
```

## Feature Files

The tests use the shared feature files from `features/`:
- `bijstand.feature` - 10 scenarios for Participatiewet
- `zorgtoeslag.feature` - 3 scenarios for healthcare allowance
- `erfgrensbeplanting.feature` - 4 scenarios for boundary planting regulations

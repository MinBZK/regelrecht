---
title: "Conformance"
description: "The language-agnostic conformance suite that lets a third-party engine prove it executes RegelRecht law correctly, organized into conformance levels per schema version."
---

The schema is the specification; the Rust engine is one implementation of it. Nothing stops another organization from building its own engine, and for a government decision system that independence is the point. The conformance suite is how a second implementation proves it produces the right answers, without depending on the RegelRecht codebase.

Tests are plain JSON, not Rust. Each one gives a regulation, parameters, and the expected outputs; an engine passes by producing those outputs. Trace assertions are optional, so an engine that does not emit traces can still demonstrate correctness on everything else.

## Structure

Tests live under `conformance/v<schema>/`, one directory per schema version, each with a `manifest.json`. A test written against schema v0.5.4 belongs under `conformance/v0.5.4/`, because what counts as correct can change between schema versions.

The manifest groups work into **conformance levels**, from a minimal core outward:

| Level | Covers |
|-------|--------|
| `core` | Arithmetic, comparison, logical, conditional, and collection operations, plus variable resolution |
| `cross_law` | Resolving a `source` reference into another law |
| `ioc` | Open terms filled by `implements` regulations |
| `temporal` | Date operations: `AGE`, `DATE_ADD`, `DATE`, `DAY_OF_WEEK`, `DATE_DIFF` |
| `advanced` | Hooks, overrides, untranslatables, data sources, and Awb procedures |

Each level names its test files and the operations it is responsible for. An engine can claim a level once it passes every test in it, which gives a precise vocabulary for partial support: an engine might be core-and-cross-law conformant without yet handling the advanced level.

## What is enforced today

The suite is partially built. Two manifests are checked in (`v0.5.0` and `v0.5.4`); the v0.5.4 manifest added `DATE_DIFF` to the temporal level alongside the [date operations](../concepts/temporal-and-dates) it belongs with.

The guarantee that holds right now is operation coverage. Three integration tests in `packages/engine/tests/conformance_coverage.rs` check the manifest against the engine's own operation list:

- every operation the engine supports appears in some level,
- no level lists an operation the engine does not have,
- no operation lands in two levels.

So a new operation cannot be added to the engine without being classified into exactly one conformance level; CI fails otherwise. The cross-engine test corpus and a runner that executes the JSON cases against an arbitrary implementation are the remaining work (see [RFC-014](/rfcs/rfc-014)).

## Further reading

- [Execution Provenance](../concepts/execution-provenance) - the receipt format a conformant engine produces
- [Schema](./schema) - the versioned specification under test
- [RFC-014: Engine Conformance Test Suite](/rfcs/rfc-014) - full specification

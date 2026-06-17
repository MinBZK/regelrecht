---
title: "Conformance"
description: "The language-agnostic conformance suite that would let a third-party engine prove it executes RegelRecht law correctly: its level structure is specified, the JSON test cases are not yet written."
---

The schema is the specification; the Rust engine is one implementation of it. Nothing stops another organization from building its own engine, and for a government decision system that independence is the point. The conformance suite is how a second implementation would prove it produces the right answers, without depending on the RegelRecht codebase.

The suite is **specified but not yet populated**: the manifests that define its structure are checked in, but the JSON test cases themselves have not been written. This page describes the design and states plainly what is enforced today (the [next section](#what-is-enforced-today) has the details). Read the test format below as the intended shape, not as files you can open and run right now.

Tests are designed to be plain JSON, not Rust. Each one will give a regulation, parameters, and the expected outputs; an engine passes by producing those outputs. Trace assertions are optional, so an engine that does not emit traces can still demonstrate correctness on everything else.

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

Each level names the test files it will contain and the operations it is responsible for. Once the cases exist, an engine could claim a level by passing every test in it, which gives a precise vocabulary for partial support: an engine might be core-and-cross-law conformant without yet handling the advanced level. The `test_files` entries in the manifests are the planned filenames; the files themselves are not written yet.

## What is enforced today

Only the manifests exist. Two are checked in (`v0.5.0` and `v0.5.4`), and they declare the level structure and operation assignments above; the v0.5.4 manifest added `DATE_DIFF` to the temporal level alongside the [date operations](../concepts/temporal-and-dates) it belongs with. The JSON test cases the manifests reference have not been authored, and there is no runner that executes them against an engine. So the cross-implementation guarantee the suite is designed to provide does **not** hold today.

What *is* enforced is narrower: operation coverage of the manifest itself. Three integration tests in `packages/engine/tests/conformance_coverage.rs` check each manifest's `operations` lists against the engine's own operation list:

- every operation the engine supports appears in some level,
- no level lists an operation the engine does not have,
- no operation lands in two levels.

So a new operation cannot be added to the engine without being classified into exactly one conformance level; CI fails otherwise. This keeps the manifest honest as the engine grows, but it tests the manifest, not any law execution. Writing the JSON test corpus and a runner that executes the cases against an arbitrary implementation is the remaining work (see [RFC-014](/rfcs/rfc-014)).

## Further reading

- [Execution Provenance](../concepts/execution-provenance) - the receipt format a conformant engine produces
- [Schema](./schema) - the versioned specification under test
- [RFC-014: Engine Conformance Test Suite](/rfcs/rfc-014) - full specification

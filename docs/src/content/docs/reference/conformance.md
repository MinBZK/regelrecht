---
title: "Conformance"
description: "What conformance enforces today (manifest operation-coverage in CI) and the language-agnostic test suite it is the foundation for. The level structure is checked in; the JSON test cases are not yet written. Design lives in RFC-014."
---

The schema is the specification; the Rust engine is one implementation of it. Nothing stops another organization from building its own engine, and for a government decision system that independence is the point. A conformance suite is how a second implementation would prove it produces the right answers, without depending on the RegelRecht codebase.

That suite is **designed but not built**. This page is therefore in two halves, in order of how real they are:

1. [What is enforced today](#what-is-enforced-today) - a CI check that keeps the conformance manifest in step with the engine's operations. This exists and runs.
2. [The intended suite](#the-intended-suite) - the cross-implementation test format the manifests are structured for. The manifests are checked in; the JSON test cases and the runner are not written yet. The full design is [RFC-014](/rfcs/rfc-014).

If you came here expecting test cases you can run against your own engine, there are none yet. Read the second half as the target, not as files on disk.

## What is enforced today

Two manifests are checked in, `conformance/v0.5.0/manifest.json` and `conformance/v0.5.4/manifest.json`. Each declares a set of conformance levels and, per level, the operations that level is responsible for. The v0.5.4 manifest added `DATE_DIFF` to the temporal level alongside the [date operations](../concepts/temporal-and-dates) it belongs with.

What runs in CI is **operation coverage of the manifest itself**, nothing more. Three integration tests in `packages/engine/tests/conformance_coverage.rs` check each manifest's `operations` lists against the engine's own operation list:

- every operation the engine supports appears in some level,
- no level lists an operation the engine does not have,
- no operation lands in two levels.

So a new operation cannot be added to the engine without being classified into exactly one conformance level; CI fails otherwise. This keeps the manifest honest as the engine grows. But it tests the *manifest*, not any law execution: it never runs a regulation, never checks an output. The cross-implementation guarantee a conformance suite is meant to provide does **not** hold today.

## The intended suite

The manifests are shaped for a test format that does not exist yet. Documented here so the structure already in the repo is legible, and so the gap is explicit.

Tests are designed to be plain JSON, not Rust, so any engine can consume them. Each case would give a regulation, parameters, and the expected outputs; an engine passes by producing those outputs. Trace assertions would be optional, so an engine that does not emit traces can still demonstrate correctness on everything else.

Tests live under `conformance/v<schema>/`, one directory per schema version, because what counts as correct can change between schema versions. A case written against schema v0.5.4 belongs under `conformance/v0.5.4/`.

The manifest groups work into conformance levels, from a minimal core outward:

| Level | Covers |
|-------|--------|
| `core` | Arithmetic, comparison, logical, conditional, and collection operations, plus variable resolution |
| `cross_law` | Resolving a `source` reference into another law |
| `ioc` | Open terms filled by `implements` regulations |
| `temporal` | Date operations: `AGE`, `DATE_ADD`, `DATE`, `DAY_OF_WEEK`, `DATE_DIFF` |
| `advanced` | Hooks, overrides, untranslatables, data sources, and Awb procedures |

Once the cases exist, an engine could claim a level by passing every test in it, which gives a precise vocabulary for partial support: an engine might be core-and-cross-law conformant without yet handling the advanced level. The `test_files` entries in the manifests are the planned filenames; those files are not written yet, and there is no runner that executes them against an engine.

Writing the JSON test corpus and a runner that executes the cases against an arbitrary implementation is the remaining work. [RFC-014](/rfcs/rfc-014) is the full design and tracks that status.

## Further reading

- [Execution Provenance](../concepts/execution-provenance) - the receipt format a conformant engine produces
- [Schema](./schema) - the versioned specification under test
- [RFC-014: Engine Conformance Test Suite](/rfcs/rfc-014) - full specification and current status
- [Rules as Executed, section 9.1](/research/rules-as-executed#sec:engines) - the position paper on multiple engines and semantic equivalence

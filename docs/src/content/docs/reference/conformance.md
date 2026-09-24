---
title: "Conformance"
description: "What conformance enforces today (manifest operation-coverage in CI) and the language-agnostic test suite it is the foundation for. The level structure is checked in; the JSON test cases are not yet written. Design lives in RFC-014."
---

The schema is the specification; the Rust engine is one implementation of it. Nothing stops another organization from building its own engine, and for a government decision system that independence is the point. A conformance suite is how a second implementation would prove it produces the right answers, without depending on the RegelRecht codebase.

That cross-implementation suite is **designed but not built**. This page is therefore in two halves, in order of how real they are:

1. [What is enforced today](#what-is-enforced-today) - two checks that run: manifest operation coverage, and a suite binding the Rust model to the schema contract.
2. [The intended suite](#the-intended-suite) - the cross-implementation test format the manifests are structured for. The manifests are checked in; the JSON test cases and the runner are not written yet. The full design is [RFC-014](/rfcs/rfc-014).

If you came here expecting test cases you can run against your own engine, there are none yet. Read the second half as the target, not as files on disk.

## What is enforced today

Nine manifests are checked in, one per schema version from `conformance/v0.5.0/` through `v0.7.0/`, each with a `manifest.json`. Each declares a set of conformance levels and, per level, the operations that level is responsible for. The v0.5.4 manifest added `DATE_DIFF` to the temporal level alongside the [date operations](../concepts/temporal-and-dates) it belongs with, v0.5.5 added `ROUND`, `CEIL` and `FLOOR` to the core level, and v0.5.7 added `FOREACH` there alongside the other [collection operations](../concepts/collections). The newest is v0.7.0, which brings the temporal level to seven operations with `DATE_PART` and `START_OF`, for 28 in total.

What runs in CI is **operation coverage of the manifests themselves**, nothing more. `packages/engine/tests/conformance_coverage.rs` checks their `operations` lists against the engine's own operation list in three integration tests:

- every operation the engine supports appears in some level, checked against the newest manifest only,
- no level lists an operation the engine does not have, checked against every manifest,
- no operation lands in two levels, checked against every manifest.

So a new operation cannot be added to the engine without being classified into exactly one conformance level; CI fails otherwise, and those three properties keep holding as the engine grows. But it tests the *manifest*, not any law execution: it never runs a regulation, never checks an output. The cross-implementation guarantee a conformance suite is meant to provide does **not** hold today.

### Schema to law-model conformance

A second suite, run with `just conformance`, proves something different and narrower: that the Rust `law-model` conforms to the hand-authored JSON schema. The schema is the canonical, language-agnostic contract; the model is one implementation of it, and neither is generated from the other. The suite checks both directions, that the model is no more permissive than the schema and no more restrictive, in three tiers: a differential over every corpus law, synthetic valid and invalid fixtures per construct, and a coverage check deriving the fixture set from the schema itself so an unexercised property has to carry a reasoned exemption.

It is the structural twin of the BDD conformance bucket: that one proves an engine speaks the whole language behaviorally, this one proves the model accepts exactly the whole language structurally. The model is currently more permissive than the schema in a few documented ways, listed in `KNOWN_GAPS`; reconciling them is tracked separately. Details are in `packages/engine/tests/conformance/README.md`.

This still says nothing about a *second* engine. It binds one implementation to the contract, which is what makes the contract worth writing against.

## The intended suite

The manifests are shaped for a test format that does not exist yet. Documented here so the structure already in the repo is legible, and so the gap is explicit.

Tests are designed to be plain JSON, not Rust, so any engine can consume them. Each case would give a regulation, parameters, and the expected outputs; an engine passes by producing those outputs. Trace assertions would be optional, so an engine that does not emit traces can still demonstrate correctness on everything else.

Tests live under `conformance/v<schema>/`, one directory per schema version, because what counts as correct can change between schema versions. A case written against schema v0.5.4 belongs under `conformance/v0.5.4/`.

The manifest groups work into conformance levels, from a minimal core outward:

| Level | Covers |
|-------|--------|
| `core` | Arithmetic, comparison, logical, conditional, and collection operations (`IN`, `LIST`, `FOREACH`), plus variable resolution |
| `cross_law` | Resolving a `source` reference into another law |
| `ioc` | Open terms filled by `implements` regulations |
| `temporal` | Date operations: `AGE`, `DATE_ADD`, `DATE`, `DAY_OF_WEEK`, `DATE_DIFF`, `DATE_PART`, `START_OF` |
| `advanced` | Hooks, overrides, markings (formerly untranslatables), data sources, and Awb procedures |

Once the cases exist, an engine could claim a level by passing every test in it, which gives a precise vocabulary for partial support: an engine might be core-and-cross-law conformant without yet handling the advanced level. The `test_files` entries in the manifests are the planned filenames; those files are not written yet, and there is no runner that executes them against an engine.

Writing the JSON test corpus and a runner that executes the cases against an arbitrary implementation is the remaining work. [RFC-014](/rfcs/rfc-014) is the full design and tracks that status.

## Further reading

- [Execution Provenance](../concepts/execution-provenance) - the receipt format a conformant engine produces
- [Schema](./schema) - the versioned specification under test
- [RFC-014: Engine Conformance Test Suite](/rfcs/rfc-014) - full specification and current status
- [Rules as Executed, section 9.1](/research/rules-as-executed#sec:engines) - the position paper on multiple engines and semantic equivalence

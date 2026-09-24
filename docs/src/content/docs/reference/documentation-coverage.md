---
title: "Documentation Coverage"
description: "Which accepted RFCs and built features have prose documentation outside the RFCs, and what is still on the backlog."
---

This page tracks how well the prose docs cover the accepted RFCs and the implemented engine and platform features. It exists so coverage gaps are visible and tracked, rather than discovered by accident. A Draft or Proposed RFC that is not built is out of scope; its own page under [RFCs](/rfcs/) is the source of truth until it is accepted or shipped.

The tables below are **enforced**: `docs/scripts/check-rfc-coverage.mjs` reads the `status` and `implementation` frontmatter of every RFC and fails CI when an RFC that is `Accepted` **or** `Implemented` has no row here (or Backlog mention). Both conditions count, because either alone leaves a hole. An implemented construct is live in the schema whatever its acceptance status says, and a reader meets it in a law file rather than in a status tag. Keying on `Accepted` alone is how `voids` shipped in schema v0.7.0 with no prose page: the RFC defining it sits at `Proposed`. So the matrix cannot silently fall behind the RFC set: accept a new RFC without adding it, and the docs build breaks. The check guarantees a *row exists* for each Accepted RFC; it does not, and cannot, judge whether the coverage that row points at is good. That judgement stays with whoever accepts the RFC, who owns adding an accurate entry as part of acceptance.

## Accepted RFCs

| RFC | Topic | Prose coverage |
|-----|-------|----------------|
| RFC-001 | YAML schema design | [Law Format](../concepts/law-format), [Schema](./schema) |
| RFC-002 | Competent authority | [Competent Authority](../concepts/competent-authority) |
| RFC-003 | Inversion of control | [Inversion of Control](../concepts/inversion-of-control) |
| RFC-004 | Uniform operation syntax | [Law Format](../concepts/law-format) |
| RFC-005 | Stand-off notes | [Notes and Annotations](../concepts/notes-and-annotations) |
| RFC-006 | Language choice (Rust) | Backlog: the "why Rust" rationale has no prose page |
| RFC-007 | Cross-law execution | [Hooks and Reactive Execution](../concepts/hooks-and-reactive-execution), [Traceability](../concepts/traceability) |
| RFC-008 | Awb procedures | [Hooks and Reactive Execution](../concepts/hooks-and-reactive-execution) |
| RFC-010 | Federated corpus | [Federated Corpus](../concepts/federated-corpus) |
| RFC-011 | Rules language selection | Backlog: the "why custom YAML" rationale has no prose page |
| RFC-012 | Untranslatables (now `markings`) | [Markings](../concepts/markings) |
| RFC-013 | Execution provenance | [Execution Provenance](../concepts/execution-provenance) |
| RFC-014 | Conformance suite | [Conformance](./conformance) |
| RFC-016 | Collection operations | [Collections](../concepts/collections) |
| RFC-018 | Note infrastructure | [Notes and Annotations](../concepts/notes-and-annotations) |
| RFC-019 | Law end dates | [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-021 | Date comparison | [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-039 | Addressable execution traces | [Traceability](../concepts/traceability), which still describes the pre-RFC-039 shape; see the backlog |

RFC-000 (the RFC process) is documented by [rfc-000](/rfcs/rfc-000) itself; the contributing guide links to it.

## Implemented RFCs

Constructs that are live in the shipped schema while their RFC is still `Proposed`. A reader meets these in a law file, not in a status tag, so they owe prose coverage on the same terms as an Accepted RFC.

| RFC | Topic | Prose coverage |
|-----|-------|----------------|
| RFC-023 | Quantities (money, percentages, units) | [Law Format](../concepts/law-format), [Schema](./schema) |
| RFC-024 | Precision and rounding | [Law Format](../concepts/law-format), [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-031 | Markings and open norms | [Markings](../concepts/markings) |
| RFC-032 | Date parts and truncation | [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-033 | Enrichment build plan | [Pipeline](../components/pipeline) |
| RFC-036 | Absent and unknown values | [Law Format](../concepts/law-format), [Collections](../concepts/collections) |
| RFC-037 | Type checking of laws | [Law Format](../concepts/law-format) |
| RFC-038 | Callable is not presentable | [Schema](./schema) |
| RFC-040 | The schema documents itself | [Schema](./schema) |
| RFC-041 | A void is not scoped like a replacement | [Voiding an output](../concepts/hooks-and-reactive-execution#voiding-an-output) |

## Backlog

Accepted RFCs whose design rationale is not yet written up as prose:

- **RFC-006 (why Rust)** and **RFC-011 (why custom YAML)** are both ratifications of language choices. A single "Design rationale" page covering both, drawing on the alternatives each RFC weighed, would close them together.

Built features that work but are thin or absent in the docs, roughly in priority order:

- **Trace shape after RFC-039**: [Traceability](../concepts/traceability) and [Engine](../components/engine) describe the trace as the bare root step, and an engine now returns a `{trace_version, root}` document whose steps carry an address and an anchor. Both pages predate that and need rewriting against the published format.
- **Editor collaboration**: trajects (create, invite members, roles, session branches) have a full backend and UI but no user-facing guide.
- **Editor views**: the law-graph visualization with trace stepping, and the AI-suggestion panel, are gated behind feature flags and undocumented.
- **WASM API surface**: the JavaScript bindings (`execute`, `executeWithTrace`, `executeMultiple`, `resolveNote`, `registerDataSource`, …) are an integration point with no reference page.
- **Data sources**: registering tabular external data for execution is implemented but unexplained.
- **TUI screens**, the **`evaluate`/`validate` CLI binaries**, the harvester's **CVDR** source, and the pipeline's **LLM-provider** selection are each implemented and lightly or never documented.

When one of these gets a page, move it up into the table above.

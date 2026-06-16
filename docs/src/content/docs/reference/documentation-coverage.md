---
title: "Documentation Coverage"
description: "Which accepted RFCs and built features have prose documentation outside the RFCs, and what is still on the backlog."
---

This page tracks how well the prose docs cover the accepted RFCs and the implemented engine and platform features. It exists so coverage gaps are visible and tracked, rather than discovered by accident. RFCs that are still Draft or Proposed are out of scope here; their own pages under [RFCs](/rfcs/) are the source of truth until they are accepted.

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
| RFC-012 | Untranslatables | [Untranslatables](../concepts/untranslatables) |
| RFC-013 | Execution provenance | [Execution Provenance](../concepts/execution-provenance) |
| RFC-014 | Conformance suite | [Conformance](./conformance) |
| RFC-018 | Note infrastructure | [Notes and Annotations](../concepts/notes-and-annotations) |
| RFC-019 | Law end dates | [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-021 | Date comparison | [Temporal Validity and Dates](../concepts/temporal-and-dates) |

RFC-000 (the RFC process) is documented by [rfc-000](/rfcs/rfc-000) itself; the contributing guide links to it.

## Backlog

Accepted RFCs whose design rationale is not yet written up as prose:

- **RFC-006 (why Rust)** and **RFC-011 (why custom YAML)** are both ratifications of language choices. A single "Design rationale" page covering both, drawing on the alternatives each RFC weighed, would close them together.

Built features that work but are thin or absent in the docs, roughly in priority order:

- **Editor collaboration**: trajects (create, invite members, roles, session branches) have a full backend and UI but no user-facing guide.
- **Editor views**: the law-graph visualization with trace stepping, and the AI-suggestion panel, are gated behind feature flags and undocumented.
- **WASM API surface**: the JavaScript bindings (`execute`, `executeWithTrace`, `executeMultiple`, `resolveNote`, `registerDataSource`, …) are an integration point with no reference page.
- **Data sources**: registering tabular external data for execution is implemented but unexplained.
- **TUI screens**, the **`evaluate`/`validate` CLI binaries**, the harvester's **CVDR** source, and the pipeline's **LLM-provider** selection are each implemented and lightly or never documented.

When one of these gets a page, move it up into the table above.

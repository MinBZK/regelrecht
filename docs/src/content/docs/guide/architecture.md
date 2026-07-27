---
title: "System Overview"
description: "A high-level tour of the two pillars, the Corpus Juris and the Execution Engine, and how the pieces fit."
---

RegelRecht is built on two pillars: the **Corpus Juris** (a git-versioned body of all Dutch law) and the **Execution Engine** (a runtime that evaluates laws deterministically).

## Code-derived architecture views

The C4 diagrams for this system are **generated from the source tree** rather than drawn by hand, so they cannot drift from the code. The `arch-extract` build tool reads the crate graph (`cargo metadata`) and internal structure (a `syn` parse) into one model and renders these pages:

- [System context](/architecture/context) — the platform as one system (C4 level 1).
- [Containers](/architecture/container) — the ten crates and how they depend on each other (C4 level 2).
- [Components](/architecture/component) — the top-level modules inside each crate (C4 level 3).
- [Architecture hub](/architecture) — the above plus a page per crate.

Regenerate with `just arch-generate`; `just arch-check` fails if a page is stale versus the code.

## Data Flow

1. **Harvesting**: The harvester downloads laws from BWB (wetten.nl) and converts XML to YAML
2. **Enrichment**: Laws are enriched with machine-readable interpretations (currently manual + AI-assisted)
3. **Storage**: All laws live in the Corpus Juris (git repository) as versioned YAML files
4. **Execution**: The engine loads laws from the corpus and evaluates them given inputs
5. **Cross-references**: When a law references another, the engine resolves the dependency chain automatically

## Design Principles

The YAML format stays close to the original legal text structure. Same inputs always produce the same outputs. Every computed value traces back to a specific article and paragraph. Text interpretation is separate from execution. And all laws, tooling, and decisions are publicly auditable.

## Further Reading

- [Methodology](/concepts/methodology) - the execution-first validation approach
- [Engine](../components/engine) - execution engine architecture
- [RFC Index](../rfcs/) - all design decisions

# System Overview

RegelRecht is built on two pillars: the **Corpus Juris** (a git-versioned body of all Dutch law) and the **Execution Engine** (a runtime that evaluates laws deterministically).

## System Context

```mermaid
C4Context
    title RegelRecht - System Context

    Person(lawmaker, "Lawmaker", "Drafts and publishes legislation")
    Person(citizen, "Citizen", "Checks eligibility for services")
    Person(agency, "Government Agency", "Makes decisions based on law")

    System(regelrecht, "RegelRecht", "Machine-readable law platform")
    System_Ext(bwb, "BWB / wetten.nl", "Official Dutch law publication")

    Rel(lawmaker, regelrecht, "Edits laws, reviews interpretations")
    Rel(citizen, regelrecht, "Checks eligibility")
    Rel(agency, regelrecht, "Executes laws for decisions")
    Rel(bwb, regelrecht, "Source of law text")
```

## Container Diagram

```mermaid
C4Container
    title RegelRecht - Containers

    Person(user, "User")

    System_Boundary(rr, "RegelRecht") {
        Container(editor, "Editor", "Vue 3 / Vite", "Law editing and browsing")
        Container(engine, "Engine", "Rust / WASM", "Deterministic law execution")
        Container(pipeline, "Pipeline", "Rust / PostgreSQL", "Job queue and law status tracking")
        Container(harvester, "Harvester", "Rust", "Downloads laws from BWB")
        Container(admin, "Admin", "Rust + Vue", "Operations dashboard")
        ContainerDb(corpus, "Corpus Juris", "Git / YAML", "All laws in machine-readable format")
        ContainerDb(db, "PostgreSQL", "Database", "Job queue and law status")
    }

    Rel(user, editor, "Browses and edits laws")
    Rel(editor, engine, "Executes laws (WASM)")
    Rel(editor, corpus, "Reads law files")
    Rel(harvester, corpus, "Writes harvested laws")
    Rel(pipeline, db, "Manages jobs")
    Rel(pipeline, harvester, "Triggers harvesting")
    Rel(admin, db, "Monitors pipeline")
```

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

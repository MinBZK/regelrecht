# System Overview

> **Work in progress.** This document describes the target architecture for RegelRecht. Not all components exist yet; see status notes per component below.
>
> Diagrams use [Mermaid](https://mermaid.js.org/). GitHub renders them natively; otherwise paste into the [Mermaid Live Editor](https://mermaid.live).

## C4 Level 1: System Context

These diagrams follow the [C4 model](https://c4model.com/) by Simon Brown. Level 1 shows who interacts with RegelRecht and what external systems it depends on. Level 2 zooms in to show the deployable containers.

```mermaid
C4Context
    title RegelRecht — System Context

    Person(lawmaker, "Legal Specialist", "Drafts and amends laws at a ministry. Authors machine-readable interpretations of legal articles.")
    Person(citizen, "Citizen", "Wants to understand how laws apply to their personal situation.")
    Person(govagency, "Government Agency", "Takes official decisions (beschikkingen) based on law.")

    System(regelrecht, "RegelRecht", "Machine-readable law platform. Stores, edits, executes, and publishes Dutch law.")

    System_Ext(wetten, "wetten.nl", "Official Dutch law repository (BWB). Source of existing laws and publication endpoint.")
    System_Ext(llm, "LLM Service", "AI model for suggesting machine-readable interpretations and cross-law relations.")

    Rel(lawmaker, regelrecht, "Browses, edits, and tests laws")
    Rel(citizen, regelrecht, "Runs law execution to predict outcomes")
    Rel(govagency, regelrecht, "Runs law execution for official decisions")
    Rel(regelrecht, wetten, "Harvests existing laws from; publishes PDF back to")
    Rel(regelrecht, llm, "Requests interpretation suggestions")

    UpdateRelStyle(lawmaker, regelrecht, $offsetY="-20")
    UpdateRelStyle(citizen, regelrecht, $offsetY="-20")
    UpdateRelStyle(govagency, regelrecht, $offsetY="-20")
```

## C4 Level 2: Container Diagram

The deployable units that make up RegelRecht.

```mermaid
C4Container
    title RegelRecht — Containers

    Person(lawmaker, "Legal Specialist", "Drafts laws, authors machine-readable interpretations")
    Person(citizen, "Citizen", "Predicts law outcomes")
    Person(govagency, "Government Agency", "Takes official decisions")

    System_Ext(llm, "LLM Service", "AI interpretation suggestions")
    System_Ext(wetten, "wetten.nl", "Official Dutch law repository (BWB)")

    System_Boundary(regelrecht, "RegelRecht") {

        Container(editor, "Editor / Browser", "HTML, CSS, JS", "Browse laws, edit articles, author machine-readable interpretations. Two modes: browser (3-pane) and editor (2-pane).")
        Container(api, "API", "TBD", "Mediates git operations between editor and Corpus Juris. Handles clone, branch, commit, merge request.")
        ContainerDb(es, "Elasticsearch", "Elasticsearch", "Full-text search index across the complete body of law.")
        Container(monitor, "Monitor", "TBD", "Tracks dependency changes in upstream and downstream laws. Alerts lawmakers.")

        Container(cj, "Corpus Juris", "Git", "All Dutch law, version-controlled. Natural language text + machine-readable execution logic.")
        Container(ci, "CI/CD Pipeline", "GitHub Actions", "Schema validation, engine tests, bot MR creation, human-gated merge, deployment.")

        Container(engine, "Execution Engine", "Rust / WASM", "Universal law runtime. Executes machine-readable law identically in any context. Compiles to native and WebAssembly.")
        Container(harvester, "Harvester", "Rust CLI", "Downloads laws + history from wetten.nl. Converts XML to schema-compliant YAML.")
        Container(pdf, "PDF Generator", "TBD", "Produces traditional publication format for backwards compatibility with wetten.nl.")
    }

    Rel(lawmaker, editor, "Browses and edits laws")
    Rel(editor, llm, "Request suggestions")
    Rel(editor, api, "Read/write laws")
    Rel(editor, engine, "Draft laws for design-time execution", "WASM")

    Rel(api, cj, "Clone/fork, branch, commit, MR")
    Rel(api, es, "Search")
    Rel(api, monitor, "Subscribe to changes")
    Rel(monitor, editor, "Alerts on dependency changes")
    Rel(api, ci, "Push triggers pipeline")
    Rel(ci, cj, "Merge on approval + publication")

    Rel(cj, engine, "Published laws")
    Rel(citizen, engine, "Runs law execution", "WASM")
    Rel(govagency, engine, "Runs law execution", "native")

    Rel(wetten, harvester, "Fetch laws + history")
    Rel(harvester, cj, "Structured YAML")
    Rel(cj, pdf, "Generate")
    Rel(pdf, wetten, "Back-publish")
```

## Detailed Architecture

The system centers on two pillars: the **Corpus Juris** — the complete body of Dutch law, git-versioned — and the **Execution Engine** — a universal runtime that executes machine-readable law identically in any context.

```mermaid
graph TB
    subgraph sources["External Sources"]
        WETTEN["wetten.nl<br/><i>BWB Repository</i>"]
    end

    subgraph harvester["Harvester"]
        H["regelrecht-harvester<br/>Downloads laws + history<br/>XML → structured YAML"]
    end

    subgraph cj["Corpus Juris <i>(git)</i>"]
        REPO["Git Repository<br/>All Dutch law, version-controlled<br/>Natural language + machine-readable"]
    end

    subgraph editing["Law Editing Platform"]
        EDITOR["Editor / Browser<br/>Browse laws · edit articles<br/>author machine-readable interpretations"]
        API["API<br/>Git operations<br/>clone/branch/commit/MR"]
        ES["Elasticsearch<br/>Full-text search<br/>across Corpus Juris"]
        LLM["LLM<br/>Suggest interpretations<br/>relations · dependencies"]
        MONITOR["Monitor<br/>Track dependency changes<br/>upstream + downstream"]
    end

    subgraph pipeline["Legislative Pipeline"]
        CI["CI<br/>Schema validation<br/>engine tests · linting"]
        BOT["Bot creates MR<br/><i>alleen MR</i>"]
        REVIEW["Human Review<br/>Legal specialist approves"]
        CD["CD<br/>Deploy + publish"]
    end

    subgraph engine["Execution Engine <i>(Rust / WASM)</i>"]
        ENG["Universal Law Runtime<br/>Same engine, same result<br/>regardless of context"]
    end

    subgraph consumers["Consumers"]
        C_LAWMAKER["Lawmaker<br/><i>design-time execution<br/>in editor</i>"]
        C_CITIZEN["Citizen<br/><i>browser-side prediction<br/>of law outcomes</i>"]
        C_GOVT["Government Agency<br/><i>backend execution<br/>for official decisions</i>"]
    end

    subgraph output["Output"]
        PDF["PDF Generation<br/>Traditional publication format<br/>for wetten.nl compatibility"]
    end

    %% Ingestion flow
    WETTEN -->|"fetch laws<br/>+ history"| H
    H -->|"structured YAML<br/>natural language"| REPO

    %% Editing flow
    EDITOR -->|"read/write"| API
    API -->|"clone/fork<br/>branch per change"| REPO
    API --> ES
    EDITOR --> LLM
    API --> MONITOR
    MONITOR -->|"alerts on<br/>dependency changes"| EDITOR

    %% Legislative pipeline
    API -->|"push changes"| CI
    CI -->|"validated"| BOT
    BOT -->|"merge request"| REVIEW
    REVIEW -->|"approved +<br/>officially published"| CD
    CD -->|"merge"| REPO

    %% Engine loads from CJ (published law)
    REPO -->|"published laws"| ENG

    %% Engine also loads from editor (draft law on local branch)
    EDITOR -->|"draft laws<br/>local branch"| ENG

    %% Engine serves all consumers
    ENG -->|"WASM"| C_LAWMAKER
    ENG -->|"WASM"| C_CITIZEN
    ENG -->|"native"| C_GOVT
    C_LAWMAKER -.-> EDITOR

    %% Output
    REPO -->|"generate"| PDF
    PDF -.->|"back-publish to<br/>traditional format"| WETTEN
```

## Components

### Corpus Juris (git)

The single source of truth: all Dutch law, version-controlled in git. Contains both the natural language legal text and machine-readable execution logic per article. The git history captures the full legislative evolution. This is the project's core proposition — law as code, version-controlled.

**Status:** The current repo (`regulation/nl/`) contains a small number of laws. The vision is to grow this into the complete Corpus Juris.

### Execution Engine (`packages/engine`)

A universal law execution runtime, written in Rust. Executes machine-readable law articles and guarantees identical outcomes regardless of where it runs. The same engine serves:

- **Lawmakers** in the editor — design-time execution against draft laws on a local branch, comparing with published law, running Gherkin scenarios, or simulating against a (synthetic) population to see the effect of changes
- **Citizens** in the browser — predicting how laws apply to their situation ("am I eligible for zorgtoeslag?")
- **Government agencies** in backend systems — taking official decisions (beschikkingen)

Same engine, same law, same result. Compiles to both native (backend) and WebAssembly (browser).

Key internals:
- **LawExecutionService** — top-level API, loads laws, resolves cross-law references
- **RuleResolver** — law registry and output index for fast lookup
- **ArticleEngine** — single-article executor
- **Operations** — 16 operation types (arithmetic, comparison, logical, conditional)
- **RuleContext** — variable resolution with priority-based lookup
- **Trace** — execution audit trail for explainability

### Harvester (`packages/harvester`)

Backfills the Corpus Juris by crawling wetten.nl (the official BWB repository). Downloads laws by BWB ID, parses XML metadata and content, splits articles according to Dutch law hierarchy (Deel → Hoofdstuk → Paragraaf → Artikel → Lid), and writes schema-compliant YAML. Processes historical versions to reconstruct the full legislative timeline.

**Status:** In development on `feature/rust-harvester` branch.

### Editor / Browser (`frontend/`)

A single tool with two modes for legal specialists at ministries:

**Browser mode** — 3-pane layout:
- Left: Law navigation (search, favorites, recently viewed/edited)
- Middle: Law structure tree (Deel → Hoofdstuk → Paragraaf → Artikel)
- Right: Article detail with **Tekst / Machine / YAML** tabs, per Lid

**Editor mode** — 2-pane layout:
- Left: Rich text editor for legal text with formatting toolbar
- Right: Machine-readable interpretation per Lid (juridische basis, besluittype, definities, parameters, inputs, outputs, actions)
- Document tabs allow editing articles from multiple laws simultaneously (reflecting cross-law dependency work)
- "Bewerken" in browser mode opens an article in editor mode

### API

Mediates between the editor and the Corpus Juris. Handles all git operations: the editor works on a clone/fork of the CJ, with branches mapping to legislative proposals. A new law or amendment is a branch that proposes changes; merging represents official publication.

### Elasticsearch

Full-text search index across the Corpus Juris. Necessary because the complete body of Dutch law is too large for brute-force search.

### Monitor

Watches for changes in law dependencies. Since laws form extensive dependency chains (via cross-law references), a change in any upstream or downstream law needs to be flagged to the lawmaker working on a related law.

### LLM Integration

AI assistant embedded in the editor. Primary use case: suggesting machine-readable interpretations of legal text. May also suggest relations, dependencies, and cross-law references.

### Legislative Pipeline (CI/CD)

The git workflow mirrors the Dutch legislative process:

1. Lawmaker creates a branch (= legislative proposal)
2. **CI** validates: schema compliance, engine tests, linting
3. **Bot** creates a Merge Request (alleen MR — only MR, no direct pushes)
4. **Human review** — legal specialist approves
5. Final merge is gated on **official publication**
6. **CD** deploys and the change becomes part of the Corpus Juris

### PDF Generation

Backwards compatibility with the traditional publication process. If wetten.nl continues to work the old way, the system generates PDF output from the structured YAML format for traditional publication.

## Cross-Law References

Laws reference each other's outputs using the `regelrecht://` URI scheme:

```
regelrecht://{law_id}/{output_name}#{field}
```

The engine resolves these by finding the referenced law, executing the target article, and extracting the requested output field. Circular reference detection prevents infinite loops (max depth: 20 for cross-law, 50 for internal references).

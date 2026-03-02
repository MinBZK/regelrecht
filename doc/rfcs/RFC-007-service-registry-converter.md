# RFC-007: Service, Registry & Converter Architecture

**Status:** Proposed
**Date:** 2026-03-02
**Authors:** regelrecht team

## Context

De conversie van wettekst naar machine-executable YAML zit nu in WIAT (extern GitLab-project). Dat hoort bij regelrecht — het platform voor machine-leesbare regelgeving. Regelrecht moet een zelfstandig platform worden dat:

1. **Het corpus beheert** — wetten als YAML in git, met kwaliteitsniveaus via branches
2. **Wetten converteert** — LLM-gedreven conversie van tekst naar machine-readable, met validatie via de engine
3. **Wetten uitvoert** — de engine als API beschikbaar stelt

De bestaande codebase heeft al:
- `packages/engine/` — Rust law execution engine (library)
- `packages/harvester/` — BWB downloader (CLI)
- `regulation/` — YAML wetbestanden
- `frontend/` — statische HTML/CSS editor

Wat ontbreekt is een service die dit geheel aanbiedt als API, en een converter die LLM-conversie automatiseert.

### Eisen

- Alles in Rust (past bij bestaande codebase)
- LLM-agnostisch (niet gebonden aan één provider)
- Queue-driven conversie met menselijke review
- Git als single source of truth (geen database voor artefacten)

## Decision

Eén Rust (Axum) service toevoegen als `packages/service/` die drie rollen combineert:

1. **Registry API** — wetten opvragen uit de git repo (`GET /api/v1/laws/...`)
2. **Execution API** — engine in-process aanroepen (`POST /api/v1/execute`)
3. **Conversion API** — async LLM-conversie met polling (`POST /api/v1/convert/jobs`)

### Git-based registry met branch-kwaliteitsniveaus

| Branch | Inhoud | Wie schrijft |
|--------|--------|-------------|
| `main` | Tekst-only wetten (harvester) + goedgekeurde machine_readable | Harvester + mens (via PR) |
| `draft-conversions` | Tekst + LLM-gegenereerde machine_readable | Converter (automatisch) |

De converter maakt per wet een PR aan voor review; `draft-conversions` wordt periodiek gerebased op `main`.

### Converter pipeline

Queue-driven: detecteert nieuwe wetten via `git diff` tussen `main` en `draft-conversions`. Per wet:

1. Parse YAML → 2. LLM generatie → 3. Engine validatie → 4. Repair loop (max 2x) → 5. Scenario generatie → 6. Scenario uitvoering → 7. Commit op `draft-conversions`

Ad-hoc conversie via API is stateless (resultaat niet opgeslagen in registry).

### Engine concurrency

`LawExecutionService` is `Send`-safe (geen `Rc` in struct-velden). Pattern:
- `RwLock<LawExecutionService>` — read-lock voor evaluatie, write-lock voor `load_law`
- `spawn_blocking` voor CPU-bound evaluatie in async handlers

### Key technical choices

| Keuze | Beslissing | Rationale |
|-------|-----------|-----------|
| Git operaties | Subprocess `git` | Simpeler dan git2/libgit2; `Mutex<()>` voor write-serialisatie |
| Prompt templates | `include_str!()` + `format!()` | Tera pas als runtime-conditionals nodig zijn |
| Job state | In-memory `DashMap` | Ephemeral; queue-jobs zijn fire-and-forget |
| API auth | `X-API-Key` header | Geen multi-tenant, simpel |
| SSE | Later (fase 4) | Start met polling-only |
| Logging | `tracing` | Al dependency van engine |

### Scope

"Volledig corpus juris" is de north-star visie. Initiële fasen richten zich op bestaande test-wetten (zorgtoeslag, participatiewet, etc.).

## Why

### Benefits

- **Zelfstandig platform** — regelrecht wordt onafhankelijk van WIAT voor conversie
- **Eén taal** — hele stack in Rust, één build, één deployment
- **Git als registry** — versiebeheer, audit trail, review workflow, offline beschikbaar — allemaal gratis
- **Engine in-process** — geen IPC-overhead, directe validatie tijdens conversie
- **LLM-agnostisch** — generieke `LlmClient` trait; providers zijn verwisselbaar
- **Bewezen pipeline** — conversie-flow is dezelfde als in WIAT, maar in Rust

### Tradeoffs

| Tradeoff | Mitigatie |
|----------|----------|
| Geen persistente job state (verloren bij restart) | Acceptabel: ad-hoc jobs opnieuw submitten; queue-jobs zijn git commits |
| Branch-divergentie `main` ↔ `draft-conversions` | Periodiek rebasen + PR per wet |
| Git subprocess i.p.v. library | Simpeler maar minder type-safe; `Mutex` voor write-serialisatie |
| Geen SSE in eerste versie | Polling volstaat; SSE toevoegen in fase 4 |
| `include_str!()` i.p.v. template engine | Simpeler maar minder flexibel; Tera als escape hatch |

### Alternatives Considered

**Alternative 1: Database-backed registry (PostgreSQL)**
- Structuurde queries, bekende tooling
- **Rejected:** versiebeheer, diff, audit trail en review workflow moeten opnieuw gebouwd worden. Git biedt dit gratis. Geen productie-users, dus geen migratie-overhead.

**Alternative 2: Aparte microservices (registry, converter, executor)**
- Onafhankelijk schaalbaar, losse koppeling
- **Rejected:** te veel overhead voor een MVP. Eén Axum service met drie routers is voldoende. Kan later gesplitst worden.

**Alternative 3: Python service (FastAPI) met Rust engine als subprocess**
- Hergebruik WIAT-code, snellere ontwikkeling
- **Rejected:** twee talen, IPC-overhead, en de conversie-pipeline moet toch herschreven worden (andere prompt-interface, validatie-loop).

**Alternative 4: git2 (libgit2) voor git operaties**
- Type-safe Rust bindings, geen subprocess overhead
- **Rejected:** complexe API, C-library dependency, beperkte feature coverage vs. CLI git. Subprocess `git` is simpeler en battle-tested.

### Implementation Notes

- De harvester is een CLI tool; een GitHub Action of cron job triggert periodiek
- LLM API keys via environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`)
- Deploy op RIG naast de bestaande frontend (zelfde project `regel-k4c`, apart component)
- Observability via `tracing` (structured logging) + metrics voor conversie success rate en queue depth

## References

- [Architectuurvisie (volledig)](../../.claude/plans/service-registry-converter.md) — het uitgebreide ontwerp met alle secties
- [RFC-006: Language Choice](RFC-006-language-choice.md) — waarom Rust
- [WIAT conversie-code](https://gitlab.com/digilab.overheid.nl/ecosystem/wiat) — de te migreren Python pipeline
- [Axum web framework](https://github.com/tokio-rs/axum)
- [DashMap](https://github.com/xacrimon/dashmap) — concurrent HashMap

# RFC-007: Service API Layer (Registry, Execution, Ad-hoc Conversion)

**Status:** Proposed
**Date:** 2026-03-02
**Authors:** regelrecht team

## Context

The law processing pipeline (#111) provides queue-based harvesting and LLM enrichment with PostgreSQL-backed job management. What's missing is an HTTP API layer that exposes:

1. **Registry** — serve law YAML from the git repo to consumers
2. **Execution** — evaluate machine-readable laws via the engine
3. **Ad-hoc conversion** — stateless LLM conversion for the editor and WIAT (not stored in the registry)

These APIs are needed by WIAT (impact analysis) and the frontend editor (live conversion preview).

### Relationship to pipeline (#111)

This RFC does **not** replace the pipeline — it builds on top of it:

| Component | Owner | Scope |
|-----------|-------|-------|
| **Pipeline** (`packages/pipeline/`) | #111 (Tim) | Harvest + enrich queue, PostgreSQL job state, worker loops |
| **Service** (`packages/service/`) | This RFC | HTTP API layer: registry, execution, ad-hoc conversion |
| **Engine** (`packages/engine/`) | Existing | Law evaluation library (used in-process by service) |
| **Harvester** (`packages/harvester/`) | Existing | BWB scraper CLI (text-only YAML) |

The service uses `pipeline/` as a library for job submission and status queries. It adds the HTTP endpoints that external consumers need.

### Component boundaries

| Component | Does | Does not |
|-----------|------|----------|
| **Harvester** | Scrape overheid.nl → text-only YAML | Anything with machine-readability |
| **Pipeline** | Queue management, harvest orchestration, LLM enrichment, quality scoring | Serve APIs to external consumers |
| **Service** | HTTP APIs for registry, execution, ad-hoc conversion | Own the queue or enrichment logic |
| **Engine** | Evaluate machine-readable YAML with parameters | Anything with HTTP or LLM |

### Branch strategy

| Branch | Content | Writer |
|--------|---------|--------|
| `main` | Text-only laws (harvester) + approved machine_readable (merged via PR) | Harvester + human review |
| `draft-conversions` | Text + LLM-generated machine_readable | Pipeline enrichment (automatic) |

`draft-conversions` is periodically rebased onto `main`. The pipeline creates one PR per law for review.

## Decision

Add `packages/service/` as an Axum HTTP service that wraps the pipeline library and engine:

### 1. Registry API

Serve law YAML from the git repo. Consumers don't need to know about branches — the service resolves internally (main first, then draft-conversions).

```
GET  /api/v1/laws                    # List all laws
GET  /api/v1/laws/{id}               # Metadata (?date=)
GET  /api/v1/laws/{id}/yaml          # YAML content (best available version)
GET  /api/v1/laws/{id}/versions      # All versions
```

`{id}` accepts slug (`wet_op_de_zorgtoeslag`) or BWB-ID (`BWBR0018451`). Quality indicator via headers:

```
X-Quality: reviewed       # or "draft" or "text-only"
X-Has-Machine-Readable: true
```

### 2. Execution API

Run the engine in-process. No subprocess overhead.

```
POST /api/v1/execute                 # Evaluate law output
POST /api/v1/execute/scenarios       # Run test scenarios
POST /api/v1/validate                # Validate YAML against schema
```

### 3. Ad-hoc Conversion API

Stateless conversion for the editor and WIAT. Uses the pipeline's enrichment logic but does **not** store results in the registry. This is essential for:
- Pending amendments (not yet final)
- Editor experiments (change an article, see what happens)
- WIAT impact analysis (hypothetical "after" version)

```
POST /api/v1/convert/jobs
{
  "after_prose": "...",              # REQUIRED: law text (after change, or only version)
  "before_prose": "...",             # OPTIONAL: law text before change
  "before_machine": "...",           # OPTIONAL: machine-readable YAML before change
  "callback_url": "https://..."     # OPTIONAL: webhook on completion
}
```

Diff-aware: if all three fields are provided, the LLM gets before-context and only adjusts changed articles.

**Response: `202 Accepted`**
```json
{
  "job_id": "abc-123",
  "status": "pending",
  "poll_url": "/api/v1/convert/jobs/abc-123"
}
```

**Status polling:**
```
GET /api/v1/convert/jobs/{job_id}
```

SSE for real-time progress is a later addition (when the editor needs it).

### Engine concurrency

`LawExecutionService` is `Send`-safe — the struct only contains `RuleResolver` and `DataSourceRegistry` (no `Rc`). The `Rc<RefCell<TraceBuilder>>` used internally is created per-evaluation in `ResolutionContext`, not stored on the service.

Pattern for concurrent access in async Axum handlers:
- **`RwLock<LawExecutionService>`** — read-lock for `evaluate_law_output` (multiple readers), write-lock for `load_law` (exclusive)
- **`spawn_blocking`** — engine evaluation is CPU-bound; wrap in `tokio::task::spawn_blocking`

### Authentication

- **API key via `X-API-Key` header** — simple, sufficient for server-to-server and editor
- No user authentication needed (not multi-tenant)
- LLM API keys via environment variables

### Deployment

- Deploy on RIG alongside the frontend (same project `regel-k4c`, separate component `service`)
- Shares PostgreSQL with the pipeline

## Why

### Benefits

- **Standalone platform** — regelrecht becomes independent from WIAT for conversion
- **Engine in-process** — no IPC overhead, direct validation during conversion
- **Stateless ad-hoc conversion** — editor and WIAT can experiment without polluting the registry
- **Diff-aware conversion** — only re-convert changed articles (faster, cheaper)
- **Consumers don't see branches** — registry API resolves quality levels internally

### Tradeoffs

| Tradeoff | Mitigation |
|----------|------------|
| No SSE in first version | Polling is sufficient; add SSE when editor needs real-time progress |
| Ad-hoc jobs are ephemeral | Acceptable: resubmit after restart; pipeline queue jobs are persistent |
| Service adds another crate | Thin layer; most logic lives in pipeline/ and engine/ |

### Alternatives Considered

**Alternative 1: Add API endpoints directly to the pipeline binary**
- Simpler, one binary
- **Rejected:** pipeline is a worker-oriented process; the API has different concerns (serving YAML, engine execution). Separation keeps both focused.

**Alternative 2: Separate microservices (registry, executor, converter)**
- Independently scalable
- **Rejected:** too much overhead for an MVP. One Axum service with three routers is sufficient.

**Alternative 3: Python service (FastAPI) wrapping the Rust engine**
- Reuse WIAT code
- **Rejected:** two languages, IPC overhead, and the conversion pipeline needs to be rewritten anyway.

## Integration flows

### WIAT impact analysis

```
1. Fetch "before" law    → GET /laws/{id}/yaml (registry)
                           → if text-only: POST /convert/jobs + poll

2. Apply changes         → WIAT's own logic (apply_law_changes)

3. Convert "after" law   → POST /convert/jobs (diff-aware, stateless)
                           → poll until completed

4. Compare before/after  → WIAT's own logic
```

### Editor live conversion

```
1. User edits article in editor
2. Editor → POST /convert/jobs (diff-aware)
3. Editor polls status (later: SSE)
4. On completion: show machine_readable diff before/after
```

## References

- [#111 Law Processing Pipeline](https://github.com/MinBZK/regelrecht-mvp/issues/111) — pipeline architecture (Tim)
- [#114 Pipeline API & CLI](https://github.com/MinBZK/regelrecht-mvp/issues/114) — pipeline's own API (to be aligned)
- [RFC-006: Language Choice](RFC-006-language-choice.md) — why Rust
- [Detailed architecture plan](../../.claude/plans/service-api-layer.md) — full design document

# RFC-007: Service API Layer (Registry, Execution, Ad-hoc Conversion)

**Status:** Proposed
**Date:** 2026-03-02
**Authors:** Anne Schuth, Tim

## Context

The law processing pipeline (#111) provides queue-based harvesting and LLM enrichment with PostgreSQL-backed job management. What's missing is an HTTP API layer that exposes:

1. **Registry** — serve law YAML from the git repo to consumers
2. **Execution** — evaluate machine-readable laws via the engine
3. **Ad-hoc conversion** — stateless LLM conversion for the editor and WIAT (not stored in the registry)

These APIs are needed by WIAT (impact analysis) and the frontend editor (live conversion preview).

**Prerequisite:** `packages/pipeline/` does not exist yet — it is being built in #111. The registry and execution APIs (Phases 0-1) can be implemented independently. The ad-hoc conversion API (Phase 2) is blocked until #111 delivers a library-compatible pipeline crate.

## Decision

Add `packages/service/` as an Axum HTTP service that wraps the pipeline library and engine.

### Relationship to pipeline (#111)

This RFC does **not** replace the pipeline — it builds on top of it:

| Component | Owner | Scope |
|-----------|-------|-------|
| **Pipeline** (`packages/pipeline/`) | #111, planned | Harvest + enrich queue, PostgreSQL job state, worker loops |
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

### 1. Registry API

**Phase 1 (filesystem only):** Serve law YAML from `regulation/nl/` on disk via filesystem walk. No branch resolution — serves whatever is on the deployed branch.

**Target state (Phase 2+):** Multi-branch resolution (main first, then draft-conversions) via git worktrees or `libgit2`, added when the pipeline lands. Consumers won't need to know about branches — the service resolves internally.

```
GET  /api/v1/laws                    # List all laws (?offset=&limit=)
GET  /api/v1/laws/{id}               # Metadata (?date=)
GET  /api/v1/laws/{id}/yaml          # YAML content (best available version)
GET  /api/v1/laws/{id}/versions      # All versions
```

The list endpoint supports pagination: `?offset=0&limit=50` (default limit 50, max 200).

`{id}` accepts slug (`wet_op_de_zorgtoeslag`) or BWB-ID (`BWBR0018451`). Slugs contain only lowercase letters and underscores; BWB-IDs match the pattern `BWBR\d{7}`. The service disambiguates by format. Quality indicator via headers:

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

**Execute request:**
```json
{
  "law_id": "wet_op_de_zorgtoeslag",
  "output": "zorgtoeslag",
  "parameters": {
    "toetsingsinkomen": 25000,
    "aanvrager_is_alleenstaande": true
  },
  "calculation_date": "2025-01-01",
  "trace": false
}
```

Set `"trace": true` to include the full evaluation trace in the response (for debugging).

**Execute response:**
```json
{
  "law_id": "wet_op_de_zorgtoeslag",
  "output": "zorgtoeslag",
  "result": { "zorgtoeslag": 111.0 },
  "trace": null
}
```

**Validate request** (subject to 1 MB body size limit):
```json
{
  "yaml": "..."
}
```

**Validate response:**
```json
{
  "valid": true,
  "errors": []
}
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

**Request size limit:** 1 MB per request body. This covers the largest single-law texts with margin.

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

```json
{
  "job_id": "abc-123",
  "status": "generating",
  "progress": "Batch 2/4 done",
  "iteration": 1,
  "result_yaml": null               // populated on completed
}
```

**Status values:**

| Status | Meaning |
|--------|---------|
| `pending` | Job queued, not yet started |
| `generating` | LLM is producing machine-readable YAML |
| `validating` | Output is being validated against the JSON schema |
| `repairing` | Validation failed; LLM is retrying with error feedback |
| `testing` | Running BDD/scenario tests against the generated output |
| `completed` | Conversion succeeded; `result_yaml` is populated |
| `failed` | Conversion failed after max retries; `error` field has details |

**Job lifecycle:** Ad-hoc jobs are ephemeral (in-memory). Completed and failed jobs are retained for 1 hour, then evicted. Maximum 50 concurrent jobs; requests beyond this limit receive `429 Too Many Requests`.

SSE for real-time progress is a later addition (when the editor needs it).

### Engine concurrency

The `LawExecutionService` struct itself is `Send` (it only holds `RuleResolver` with `HashMap` fields, and `DataSourceRegistry` with `Vec<Box<dyn DataSource>>` where `DataSource: Send + Sync`). However, evaluation methods internally create `Rc<RefCell<TraceBuilder>>` per call, which is not `Send`. This means evaluation **must not be held across `.await` points** — it must run to completion on a single thread.

Pattern for concurrent access in async Axum handlers:
- **`Arc<RwLock<LawExecutionService>>`** — read-lock for `evaluate_law_output` (multiple readers), write-lock for `load_law` (exclusive)
- **`spawn_blocking`** — required because (1) evaluation is CPU-bound and (2) the per-call `Rc<RefCell>` is not `Send` across await points

Law reloading (write-lock) happens only on startup and when the registry detects file changes. During normal operation, all requests take read-locks and run concurrently.

### Error handling

All error responses use [RFC 7807](https://www.rfc-editor.org/rfc/rfc7807) Problem Details format:

```json
{
  "type": "https://regelrecht.nl/errors/law-not-found",
  "title": "Law not found",
  "status": 404,
  "detail": "No law with id 'wet_op_de_foo' found in registry"
}
```

Standard error codes:

| Status | When | Phase |
|--------|------|-------|
| `400` | Invalid request body, missing required fields, request too large | All |
| `401` | Missing or invalid API key | All |
| `404` | Law not found, job not found | All |
| `422` | YAML validation failed, engine evaluation error | All |
| `429` | Too many concurrent conversion jobs | 2+ |
| `500` | Internal server error | All |
| `503` | Pipeline unavailable (conversion endpoints only) | 2+ |

### Authentication

- **API key via `X-API-Key` header** — simple, sufficient for server-to-server and editor
- Single shared key, configured via `SERVICE_API_KEY` environment variable
- No user authentication needed (not multi-tenant)
- Key rotation: redeploy with new env var; no hot-reload needed for MVP
- LLM API keys via separate environment variables (`LLM_API_KEY`)

### CORS

The frontend editor runs on a different RIG deployment than the service. CORS headers are required:
- `Access-Control-Allow-Origin`: configured via `CORS_ALLOWED_ORIGINS` env var (editor URL)
- `Access-Control-Allow-Headers`: `X-API-Key, Content-Type`

### Request limits

- **Body size:** 1 MB max for all POST endpoints (covers full law texts with margin)
- **Rate limiting:** conversion endpoints (`/convert/jobs`) are rate-limited to 10 requests/minute per API key (LLM calls are expensive). Registry and execution endpoints are not rate-limited in the MVP.

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
| Ad-hoc jobs are ephemeral (in-memory) | Acceptable: 1h TTL, max 50 concurrent, resubmit after restart; pipeline queue jobs are persistent in PostgreSQL |
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

## Implementation Notes

### Crate structure

```
packages/
  pipeline/          # PLANNED (#111): queue, harvest, enrich, PostgreSQL
  service/           # NEW (this RFC): HTTP API layer
    src/
      main.rs        # Axum server startup
      config.rs      # Environment-based config

      api/
        registry.rs  # GET /laws/...
        execution.rs # POST /execute, /validate
        conversion.rs# POST /convert/jobs, GET /convert/jobs/{id}

      engine/
        wrapper.rs   # RwLock<LawExecutionService> + spawn_blocking
```

In Phases 0-1, the service depends only on `engine/` (for in-process execution). From Phase 2, it also depends on `pipeline/` as a library (for job submission and status).

### Phases

> The pipeline (#111) handles its own phasing. These phases are for the service layer only.

**Phase 0: Scaffold** — `packages/service/` with Cargo.toml, Axum skeleton, API key auth, health endpoint

**Phase 1: Registry + Execution** — Filesystem-based law lookup (no branch resolution), engine wrapper (Arc<RwLock> + spawn_blocking), registry + execute endpoints

**Phase 2: Ad-hoc Conversion** — Wire pipeline enrichment as library, `POST /convert/jobs` + polling, diff-aware mode. Blocked until #111 delivers a library-compatible pipeline crate.

**Phase 3: WIAT integration** — `RegelrechtClient` in WIAT (replaces `regelrecht_generate.py`)

## References

- [#111 Law Processing Pipeline](https://github.com/MinBZK/regelrecht-mvp/issues/111) — pipeline architecture (Tim)
- [#114 Pipeline API & CLI](https://github.com/MinBZK/regelrecht-mvp/issues/114) — pipeline's own API (to be aligned)
- [RFC-006: Language Choice](RFC-006-language-choice.md) — why Rust

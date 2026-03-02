# Service API Layer — Detailed Design

Companion document to [RFC-007](../../doc/rfcs/RFC-007-service-api-layer.md). This document covers implementation details; the RFC covers the decision rationale.

## Scope

This plan covers `packages/service/` — an HTTP API layer on top of the pipeline (#111). It does **not** cover queue management, harvesting, or enrichment (that's the pipeline's domain).

---

## 1. Crate structure

```
packages/
  pipeline/          # EXISTING (#111): queue, harvest, enrich, PostgreSQL
  service/           # NEW (this plan): HTTP API layer
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

The service depends on `pipeline/` as a library (for job submission and status) and `engine/` (for in-process execution).

---

## 2. Registry API

Serves law YAML from the git repo. Resolves quality levels internally — consumers don't see branches.

```
GET  /api/v1/laws                    # List all laws
GET  /api/v1/laws/{id}               # Metadata (?date=)
GET  /api/v1/laws/{id}/yaml          # YAML content (best available)
GET  /api/v1/laws/{id}/versions      # All versions
```

Resolution order: `main` (reviewed) → `draft-conversions` (LLM draft). Quality indicated via response headers:

```
X-Quality: reviewed | draft | text-only
X-Has-Machine-Readable: true | false
```

`{id}` accepts slug (`wet_op_de_zorgtoeslag`) or BWB-ID (`BWBR0018451`).

---

## 3. Execution API

Engine runs in-process via `RwLock<LawExecutionService>`:
- **Read-lock** for `evaluate_law_output` (concurrent readers)
- **Write-lock** for `load_law` (exclusive)
- **`spawn_blocking`** for CPU-bound evaluation in async handlers

`LawExecutionService` is `Send`-safe — no `Rc` in struct fields. The `Rc<RefCell<TraceBuilder>>` is created per-evaluation in `ResolutionContext`.

```
POST /api/v1/execute                 # Evaluate law output
POST /api/v1/execute/scenarios       # Run test scenarios
POST /api/v1/validate                # Validate YAML against schema
```

---

## 4. Ad-hoc Conversion API

Stateless conversion for editor and WIAT. Uses pipeline enrichment logic but does **not** store results in the registry.

```
POST /api/v1/convert/jobs            # Submit conversion job
GET  /api/v1/convert/jobs/{job_id}   # Poll status
```

### Request

```json
{
  "after_prose": "...",              // REQUIRED
  "before_prose": "...",             // OPTIONAL: enables diff-aware conversion
  "before_machine": "...",           // OPTIONAL: existing machine_readable
  "callback_url": "https://..."     // OPTIONAL: webhook on completion
}
```

### Response (`202 Accepted`)

```json
{
  "job_id": "abc-123",
  "status": "pending",
  "poll_url": "/api/v1/convert/jobs/abc-123"
}
```

### Status polling

```json
{
  "job_id": "abc-123",
  "status": "generating",           // pending|generating|validating|repairing|testing|completed|failed
  "progress": "Batch 2/4 done",
  "iteration": 1,
  "result_yaml": null               // populated on completed
}
```

SSE for real-time progress is a later addition.

---

## 5. Authentication & deployment

- **API key** via `X-API-Key` header (not multi-tenant)
- **LLM keys** via environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`)
- Deploy on **RIG** alongside frontend and pipeline (same project `regel-k4c`)
- Shares PostgreSQL with pipeline

---

## 6. Integration flows

### WIAT impact analysis

```
1. Fetch "before" law    → GET /laws/{id}/yaml
2. Apply changes         → WIAT's own logic
3. Convert "after" law   → POST /convert/jobs (diff-aware, stateless) → poll
4. Compare before/after  → WIAT's own logic
```

### Editor live conversion

```
1. User edits article
2. Editor → POST /convert/jobs (diff-aware)
3. Editor polls status (later: SSE)
4. On completion: show machine_readable diff
```

---

## 7. Implementation phases

> The pipeline (#111) handles its own phasing. These phases are for the service layer only.

### Phase 0: Scaffold
- `packages/service/` with Cargo.toml
- Axum skeleton + API key auth + health endpoint

### Phase 1: Registry + Execution
- Git-based law lookup
- Engine wrapper (RwLock + spawn_blocking)
- Registry + execute endpoints

### Phase 2: Ad-hoc Conversion
- Wire pipeline enrichment as library
- `POST /convert/jobs` + polling
- Diff-aware mode

### Phase 3: WIAT integration
- `RegelrechtClient` in WIAT (replaces `regelrecht_generate.py`)

---

## References

- [RFC-007](../../doc/rfcs/RFC-007-service-api-layer.md) — decision rationale
- [#111 Law Processing Pipeline](https://github.com/MinBZK/regelrecht-mvp/issues/111)
- [#114 Pipeline API & CLI](https://github.com/MinBZK/regelrecht-mvp/issues/114)

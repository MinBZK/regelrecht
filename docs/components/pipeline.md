# Pipeline

The pipeline is a PostgreSQL-backed job queue and law status tracking system that orchestrates the law processing workflow.

## Overview

- **Language**: Rust
- **Location**: `packages/pipeline/`
- **Database**: PostgreSQL
- **Key feature**: Reliable concurrent job processing with `FOR UPDATE SKIP LOCKED`

## Architecture

The pipeline coordinates two processing stages: **harvesting** (downloading laws from wetten.nl) and **enrichment** (adding machine-readable logic via LLM).

```mermaid
flowchart LR
    subgraph Pipeline
        Q[Job Queue]
        S[Law Status Tracker]
    end
    subgraph Workers
        HW[Harvest Worker]
        EW[Enrich Worker]
    end
    BWB[BWB / wetten.nl] -->|XML| HW
    HW -->|YAML| Corpus[Corpus Juris]
    HW -->|claim/complete| Q
    EW -->|claim/complete| Q
    Q --> S
    LLM[LLM Provider] -->|machine_readable| EW
    EW -->|enriched YAML| Corpus
```

## Modules

| Module | Purpose |
|--------|---------|
| `job_queue.rs` | Job creation, claiming (`FOR UPDATE SKIP LOCKED`), completion, failure with auto-retry |
| `law_status.rs` | Per-law status tracking through 8 states |
| `harvest.rs` | Harvest execution — download XML from BWB, convert to YAML |
| `enrich.rs` | Enrichment execution — call LLM to add `machine_readable` sections |
| `worker.rs` | Polling loops for harvest and enrich workers |
| `models.rs` | Data types: `Job`, `LawEntry`, `JobType`, `JobStatus`, `LawStatusValue`, `Priority` |
| `config.rs` | Configuration from environment variables |
| `db.rs` | Connection pool creation and migration runner |
| `error.rs` | Error types (`PipelineError`) |

## Job Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Pending: create_job
    Pending --> Processing: claim_job (FOR UPDATE SKIP LOCKED)
    Processing --> Completed: complete_job
    Processing --> Pending: fail_job (retries left)
    Processing --> Failed: fail_job (max attempts reached)
    Processing --> Pending: reap_orphaned_jobs (timeout)
    Processing --> Failed: reap_orphaned_jobs (no retries)
```

Workers claim jobs atomically using PostgreSQL's `FOR UPDATE SKIP LOCKED` — multiple workers can safely process jobs concurrently without blocking each other.

### Automatic Retries

When a job fails and has attempts remaining (`attempts < max_attempts`), it returns to `Pending` for retry. Default `max_attempts` is 3.

### Orphan Reaping

Jobs stuck in `Processing` beyond the orphan timeout (default: 30 minutes) are reset to `Pending` or marked `Failed`, handling crashed workers gracefully.

## Law Status Tracking

Each law in the corpus progresses through processing states:

```mermaid
stateDiagram-v2
    [*] --> Unknown
    Unknown --> Queued: harvest job created
    Queued --> Harvesting: worker claims job
    Harvesting --> Harvested: harvest succeeds
    Harvesting --> HarvestFailed: retries exhausted
    Harvested --> Enriching: enrich job claimed
    Enriching --> Enriched: enrichment succeeds
    Enriching --> EnrichFailed: retries exhausted
```

## Harvest Worker

The harvest worker:
1. Polls the queue for pending harvest jobs
2. Downloads law XML from BWB (wetten.nl)
3. Converts XML to YAML via the harvester library
4. Writes YAML to the corpus
5. Auto-creates enrich jobs for each configured LLM provider
6. Creates follow-up harvest jobs for referenced laws (respects depth limit of 1000)

## Enrich Worker

The enrich worker:
1. Polls the queue for pending enrich jobs
2. Spawns an LLM CLI process to generate `machine_readable` sections
3. Tracks progress via `.enrichment-progress.json` (polled every 10s)
4. Computes coverage score (newly enriched articles / articles needing enrichment)
5. Creates per-provider branches (e.g., `enrich/opencode`)

### LLM Providers

The LLM provider is configurable via `LLM_PROVIDER` (default: `opencode`). Provider-specific paths and models are set via environment variables (e.g., `OPENCODE_PATH`, `OPENCODE_MODEL`).

The LLM subprocess runs with a stripped environment (allowlisted vars only) for security.

## Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `DATABASE_URL` | required | PostgreSQL connection string |
| `DATABASE_MAX_CONNECTIONS` | 5 | Connection pool size |
| `REGULATION_REPO_PATH` | `./regulation-repo` | Output directory |
| `WORKER_POLL_INTERVAL_SECS` | 5 | Queue poll interval |
| `WORKER_MAX_POLL_INTERVAL_SECS` | 60 | Max backoff interval |
| `WORKER_JOB_TIMEOUT_SECS` | 1200 (20 min) | Job execution timeout |
| `WORKER_ORPHAN_TIMEOUT_SECS` | 1800 (30 min) | Orphan detection timeout |
| `LLM_PROVIDER` | `opencode` | LLM provider selection |
| `LLM_TIMEOUT_SECS` | 600 (10 min) | LLM execution timeout |

## Database Schema

Two tables with PostgreSQL enums:

**`jobs`** — Job queue with retry tracking, priority ordering, and JSONB payload/result/progress columns. Partial index `WHERE status = 'pending'` for efficient claiming.

**`law_entries`** — Per-law status tracking with foreign keys to harvest/enrich jobs and a coverage score (0.0–1.0).

Migrations run automatically at startup using an advisory lock for coordination.

## Testing

```bash
just pipeline-test               # Unit tests (no Docker)
just pipeline-integration-test   # Integration tests (Docker + testcontainers)
```

Integration tests use `testcontainers` to spin up ephemeral PostgreSQL instances — no local database setup required.

## Further Reading

- [Harvester](./harvester) — the BWB law downloader used by harvest jobs
- [Architecture](/architecture/overview) — where the pipeline fits in the system

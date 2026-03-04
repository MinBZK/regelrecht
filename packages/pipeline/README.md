# regelrecht-pipeline

PostgreSQL-backed job queue and law status tracking for the RegelRecht processing pipeline.

## What is this?

The pipeline crate provides the infrastructure that orchestrates the law processing workflow: harvesting laws from wetten.nl and enriching them with machine-readable interpretations. It sits between the [Harvester](../harvester/) and the Corpus Juris in the [system architecture](../../doc/architecture/system-overview.md).

```
wetten.nl ──→ [Harvester] ──→ [Pipeline: job queue] ──→ [Corpus Juris]
                                       │
                              tracks status per law
                              retries on failure
                              prioritizes work
```

### Why a separate job queue?

Laws are processed in two stages — **harvest** (download + convert from wetten.nl) and **enrich** (add machine-readable logic). Both stages can fail, need retries, and must be tracked per law. The pipeline crate provides:

- **Persistent job queue** — jobs survive restarts, backed by PostgreSQL
- **Concurrent-safe claiming** — multiple workers can safely compete for jobs using `FOR UPDATE SKIP LOCKED`
- **Automatic retry** — failed jobs return to the queue up to a configurable max attempts
- **Law status tracking** — each law's processing state is tracked from `unknown` through `harvested` to `enriched`
- **Transaction support** — all operations accept both a connection pool and a transaction, so callers can group operations atomically

## Architecture fit

In the [C4 container diagram](../../doc/architecture/system-overview.md), the pipeline is part of the **CI/CD Pipeline** layer. It coordinates the Harvester and future enrichment steps:

| Component | Role |
|-----------|------|
| **Pipeline** (this crate) | Job queue + status tracking. Decides *what* to process and *when*. |
| **Harvester** | Downloads laws from wetten.nl, converts XML → YAML. The pipeline creates harvest jobs; the harvester executes them. |
| **Enrichment** (future) | Adds machine-readable interpretations. The pipeline creates enrich jobs; an enrichment worker executes them. |

### Law processing flow

```
┌─────────┐     ┌───────────┐     ┌───────────┐     ┌──────────┐
│ unknown │ ──→ │  queued   │ ──→ │harvesting │ ──→ │harvested │
└─────────┘     └───────────┘     └───────────┘     └──────────┘
                                        │                 │
                                        ▼                 ▼
                                  ┌─────────────┐  ┌───────────┐     ┌──────────┐
                                  │harvest_failed│  │ enriching │ ──→ │ enriched │
                                  └─────────────┘  └───────────┘     └──────────┘
                                                         │
                                                         ▼
                                                   ┌─────────────┐
                                                   │enrich_failed│
                                                   └─────────────┘
```

## Usage

### Setup

```bash
# Start local PostgreSQL
just db-up

# Run database migrations
just db-migrate

# Run tests (uses testcontainers — no local DB needed)
just pipeline-test
```

### As a library

```rust
use regelrecht_pipeline::{
    PipelineConfig, create_pool, run_migrations,
    job_queue::{self, CreateJobRequest},
    law_status,
    JobType, LawStatusValue, Priority,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to PostgreSQL
    let config = PipelineConfig::from_env()?;
    let pool = create_pool(&config).await?;
    run_migrations(&pool).await?;

    // Create a harvest job
    let job = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "zorgtoeslagwet")
            .with_priority(Priority::new(80))
            .with_payload(serde_json::json!({
                "bwb_id": "BWBR0018451",
                "url": "https://wetten.overheid.nl/BWBR0018451"
            })),
    ).await?;

    // Track the law's status
    law_status::upsert_law(&pool, "zorgtoeslagwet", Some("Zorgtoeslagwet")).await?;
    law_status::update_status(&pool, "zorgtoeslagwet", LawStatusValue::Queued).await?;
    law_status::set_harvest_job(&pool, "zorgtoeslagwet", job.id).await?;

    Ok(())
}
```

### Worker pattern

```rust
// A worker claims and processes jobs in a loop
loop {
    let Some(job) = job_queue::claim_job(&pool, Some(JobType::Harvest)).await? else {
        tokio::time::sleep(Duration::from_secs(5)).await;
        continue;
    };

    // Update law status
    law_status::update_status(&pool, &job.law_id, LawStatusValue::Harvesting).await?;

    match do_harvest(&job).await {
        Ok(result) => {
            job_queue::complete_job(&pool, job.id, Some(result)).await?;
            law_status::update_status(&pool, &job.law_id, LawStatusValue::Harvested).await?;
        }
        Err(e) => {
            // Automatically retries if attempts < max_attempts
            job_queue::fail_job(&pool, job.id, Some(json!({"error": e.to_string()}))).await?;
            law_status::update_status(&pool, &job.law_id, LawStatusValue::HarvestFailed).await?;
        }
    }
}
```

### Using transactions

All functions accept `impl PgExecutor`, so you can pass either `&pool` (auto-commit) or `&mut tx` (transaction) for atomic operations:

```rust
let mut tx = pool.begin().await?;

let job = job_queue::create_job(&mut *tx, CreateJobRequest::new(JobType::Harvest, "my_law")).await?;
law_status::upsert_law(&mut *tx, "my_law", Some("My Law")).await?;
law_status::set_harvest_job(&mut *tx, "my_law", job.id).await?;
law_status::update_status(&mut *tx, "my_law", LawStatusValue::Queued).await?;

tx.commit().await?;
// All four operations committed atomically — or none of them.
```

## API reference

### Job queue (`job_queue`)

| Function | Description |
|----------|-------------|
| `create_job(executor, request)` | Create a new job in the queue |
| `claim_job(executor, job_type?)` | Claim the highest-priority pending job (concurrent-safe) |
| `complete_job(executor, job_id, result?)` | Mark a processing job as completed |
| `fail_job(executor, job_id, error?)` | Mark a processing job as failed (auto-retries if attempts remain) |
| `get_job(executor, job_id)` | Get a job by ID |
| `list_jobs(executor, status?)` | List jobs, optionally filtered by status |

### Law status (`law_status`)

| Function | Description |
|----------|-------------|
| `upsert_law(executor, law_id, name?)` | Create or update a law entry |
| `update_status(executor, law_id, status)` | Update a law's processing status |
| `set_harvest_job(executor, law_id, job_id)` | Link a harvest job to a law |
| `set_enrich_job(executor, law_id, job_id)` | Link an enrich job to a law |
| `set_quality_score(executor, law_id, score)` | Set quality score (0.0–1.0) |
| `get_law(executor, law_id)` | Get a law entry by ID |
| `list_laws(executor, status?)` | List laws, optionally filtered by status |

### Types

| Type | Values |
|------|--------|
| `JobType` | `Harvest`, `Enrich` |
| `JobStatus` | `Pending`, `Processing`, `Completed`, `Failed` |
| `LawStatusValue` | `Unknown`, `Queued`, `Harvesting`, `Harvested`, `HarvestFailed`, `Enriching`, `Enriched`, `EnrichFailed` |
| `Priority` | 0–100 (default: 50, higher = processed first) |

## Database

### Requirements

- PostgreSQL 13+ (uses `gen_random_uuid()`, custom enums, partial indexes)
- Connection string via `DATABASE_URL` environment variable

### Schema

Two tables:

- **`jobs`** — job queue with priority, status, retry tracking, JSONB payload/result
- **`law_entries`** — per-law status tracking with links to harvest/enrich jobs

Key design choices:
- Partial index `WHERE status = 'pending'` for efficient job claiming
- `FOR UPDATE SKIP LOCKED` prevents double-claiming without blocking
- `updated_at` trigger for automatic timestamp management
- Foreign keys with `ON DELETE SET NULL` for safe job cleanup

### Migrations

```bash
# Run migrations (requires DATABASE_URL)
just db-migrate

# Or directly:
cd packages/pipeline && cargo sqlx migrate run
```

## Development

```bash
# Start local PostgreSQL (Docker)
just db-up

# Run tests (Docker required for testcontainers)
just pipeline-test

# Check compilation (no DB needed)
just pipeline-check

# Stop local PostgreSQL
just db-down
```

### Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | (required) |
| `DATABASE_MAX_CONNECTIONS` | Max pool connections | `5` |

See `.env.example` for a local development configuration.

## License

MIT — See [LICENSE](../../LICENSE) for details.

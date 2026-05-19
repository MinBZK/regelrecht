# Admin Dashboard

The admin dashboard is a web application for operators to monitor and control the harvester pipeline.

## Overview

- **Language**: Rust (Axum) + Vue 3 (Vite)
- **Location**: `packages/admin/`
- **Production URL**: `harvester-admin.regelrecht.rijks.app`

## What it does

The admin dashboard shows pipeline status: pending jobs, running harvests and enrichments, law processing states, and coverage scores. Operators can trigger new harvest and enrichment jobs, view error details, and monitor throughput.

## Architecture

The backend is a Rust Axum server that connects to PostgreSQL (via the pipeline library) and exposes a REST API. The frontend is a Vue 3 SPA served as static files from the same server.

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Backend | Rust / Axum | REST API, OIDC auth, Prometheus metrics |
| Frontend | Vue 3 / Vite | Job and law status UI |
| Database | PostgreSQL | Shared with pipeline workers |
| Auth | OIDC (Keycloak) | Operator login |

## Key API endpoints

- `GET /api/jobs` - list jobs with pagination and status filters
- `POST /api/harvest-jobs` - enqueue a harvest job
- `POST /api/enrich-jobs` - enqueue enrichment jobs
- `GET /api/law_entries` - list law entries with status
- `GET /metrics` - Prometheus metrics

## Running locally

```bash
DATABASE_URL=postgres://user:pass@localhost:5433/regelrecht cargo run -p regelrecht-admin
```

The frontend dev server runs separately:

```bash
cd packages/admin/frontend-src && npm run dev
```

## Further reading

- [Pipeline](./pipeline) - the job queue this dashboard monitors
- [Deployment](/operations/deployment) - how the admin dashboard is deployed

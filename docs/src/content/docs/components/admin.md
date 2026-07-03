---
title: "Admin Dashboard"
description: "The web dashboard operators use to monitor and control the harvester pipeline."
---

The harvester-admin service lets operators monitor and control the harvester pipeline. It is a standalone Rust API; its dashboard UI lives inside the editor as the "Corpusinwinning" section (`frontend/src/harvester/`), reached through the editor-api `/api/harvest-admin/*` proxy. The API stays independently addressable for scripts and other services.

## Overview

- **Language**: Rust (Axum), API only
- **Location**: `packages/admin/` (API); UI in `frontend/src/harvester/`
- **Production URL**: `harvester-admin.regelrecht.rijks.app` (API); UI at `editor.regelrecht.rijks.app` → Corpusinwinning

## What it does

The admin dashboard shows pipeline status: pending jobs, running harvests and enrichments, law processing states, and coverage scores. Operators can trigger new harvest and enrichment jobs, view error details, and monitor throughput.

## Architecture

The backend is a Rust Axum server that connects to PostgreSQL (via the pipeline library) and exposes a REST API. It no longer serves a SPA; the UI is part of the editor and reaches this API through the editor-api proxy (which forwards the shared session cookie so this service enforces its own `harvester-*` role gates).

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Backend | Rust / Axum | REST API, OIDC auth, Prometheus metrics |
| Frontend | Vue 3 (in the editor, `frontend/src/harvester/`) | Job and law status UI, reached via editor-api proxy |
| Database | PostgreSQL | Shared with pipeline workers (and editor-api session store) |
| Auth | OIDC (Keycloak) | Operator login; `harvester-*` role gates |

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

The dashboard UI is served by the editor. For the full end-to-end flow
(editor + editor-api proxy + admin API + database) use:

```bash
just dev-frontend all
```

Then open the editor and choose "Corpusinwinning" from the account menu (visible to any
`harvester-*` role).

## Further reading

- [Pipeline](./pipeline) - the job queue this dashboard monitors
- [Deployment](/operations/deployment) - how the admin dashboard is deployed

---
title: "Editor API"
description: "The Rust backend behind the law editor, serving the frontend and corpus REST endpoints."
---

The editor API is an HTTP server that backs the law editor frontend. It serves the compiled frontend and provides REST endpoints for corpus access.

## Overview

- **Language**: Rust (Axum)
- **Location**: `packages/editor-api/`
- **Port**: 8000

## What it does

The editor API loads the regulation corpus (from local files or GitHub via the corpus library) and exposes it through a REST API. The editor frontend calls these endpoints to list laws, fetch and edit individual law YAML, manage annotations, run harvest jobs, and collaborate inside trajects.

## Key endpoints

The API surface has grown well beyond corpus reads. The main groups:

- **Corpus**: `GET /api/corpus/laws`, `GET/PUT /api/corpus/laws/{law_id}`, plus `/outputs`, `/scenarios`, `/scenarios/{filename}`, `/annotations`, and `POST /api/corpus/reload`.
- **Trajects** (private-repo collaboration): full CRUD under `/api/trajects` and `/api/trajects/{id}`, member and invite management (`/members`, `/invites/{email}`, `/leave`), and traject-scoped corpus access under `/api/trajects/{traject_ref}/corpus/...`.
- **Harvesting**: `/api/harvest`, `/api/harvest/batch`, `/api/harvest/search`, `/api/harvest/status`.
- **Harvester-admin proxy**: `/api/harvest-admin/{*rest}`, a single wildcard route accepting any method. It rewrites `/api/harvest-admin/<x>` to `/api/<x>` on the standalone harvester-admin service and forwards the session cookie. This is how the editor's Corpusinwinning section reaches that API; the browser never calls it directly. The route is gated on `harvester-reader` as defence in depth, and harvester-admin remains the real enforcer for writer and admin actions.
- **Tasks** (review tasks from async jobs, scoped to the requesting account): `GET /api/tasks`, `GET /api/tasks/{task_id}`, `GET /api/tasks/jobs/{job_id}`, plus `POST /api/tasks/{task_id}/resolve` and `POST /api/tasks/jobs/{job_id}/apply` at writer tier.
- **Personal notes**: `GET/POST /api/user/notes/{law_id}` and `PUT/DELETE /api/user/notes/{law_id}/{note_id}`, reads at reader tier and mutations at writer tier.
- **GitHub OAuth**: `/auth/github/login`, `/auth/github/callback`, `/auth/github/status`, `POST /auth/github/disconnect`, and `/auth/github/relay`.
- **Misc**: `/api/sources`, `/api/favorites`, `/api/feature-flags`, `/api/user/settings`, and `/health`.

See `packages/editor-api/src/main.rs` for the authoritative route table.

## Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `STATIC_DIR` | `static` | Path to compiled frontend |
| `CORPUS_REGISTRY_PATH` | `corpus-registry.yaml` | Registry manifest location |
| `CORPUS_AUTH_FILE` | `corpus-auth.yaml` | Path to authentication config |

`DATABASE_URL` (or `DATABASE_SERVER_FULL`), `PIPELINE_API_URL`, `HARVEST_ADMIN_URL`, and `CORPUS_REGISTRY_LOCAL_PATH` are also read where the traject and harvest features need them. `PIPELINE_API_URL` and `HARVEST_ADMIN_URL` are fallbacks: when the pod hostname names a deployment, the API derives both service URLs from it (`http://<deployment>-harvester-admin:8000`) and the hostname wins over a stale environment variable.

## Running locally

```bash
STATIC_DIR=../frontend/dist cargo run -p regelrecht-editor-api
```

In development, run the frontend dev server separately for hot reload (see [Editor](./frontend)).

## Further reading

- [Editor](./frontend) - the frontend this API backs
- [Corpus Library](./corpus) - the library used for law loading

---
title: "Editor API"
---

The editor API is a lightweight HTTP server that backs the law editor frontend. It serves the compiled frontend and provides REST endpoints for corpus access.

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
- **Misc**: `/api/sources`, `/api/favorites`, `/api/feature-flags`, `/api/user/settings`, and `/health`.

See `packages/editor-api/src/main.rs` for the authoritative route table.

## Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `STATIC_DIR` | `static` | Path to compiled frontend |
| `CORPUS_REGISTRY_PATH` | `corpus-registry.yaml` | Registry manifest location |
| `CORPUS_AUTH_FILE` | `corpus-auth.yaml` | Path to authentication config |

`DATABASE_URL` (or `DATABASE_SERVER_FULL`), `PIPELINE_API_URL`, and `CORPUS_REGISTRY_LOCAL_PATH` are also read where the traject and harvest features need them.

## Running locally

```bash
STATIC_DIR=../frontend/dist cargo run -p regelrecht-editor-api
```

In development, run the frontend dev server separately for hot reload (see [Editor](./frontend)).

## Further reading

- [Editor](./frontend) - the frontend this API backs
- [Corpus Library](./corpus) - the library used for law loading

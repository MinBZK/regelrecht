# Editor API

The editor API is a lightweight HTTP server that backs the law editor frontend. It serves the compiled frontend and provides REST endpoints for corpus access.

## Overview

- **Language**: Rust (Axum)
- **Location**: `packages/editor-api/`
- **Port**: 8000

## What it does

The editor API loads the regulation corpus (from local files or GitHub via the corpus library) and exposes it through a REST API. The editor frontend calls these endpoints to list laws, fetch individual law YAML, and retrieve BDD test scenarios.

## Key endpoints

- `GET /api/corpus/laws` - list all laws in the corpus
- `GET /api/corpus/laws/{id}` - fetch a specific law's YAML
- `GET /api/corpus/laws/{id}/scenarios` - fetch BDD scenarios for a law

## Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `STATIC_DIR` | `../frontend/dist` | Path to compiled frontend |
| `CORPUS_REGISTRY_PATH` | `corpus-registry.yaml` | Registry manifest location |
| `CORPUS_AUTH_FILE` | - | Path to authentication config |

## Running locally

```bash
STATIC_DIR=../frontend/dist cargo run -p regelrecht-editor-api
```

In development, run the frontend dev server separately for hot reload (see [Editor](./frontend)).

## Further reading

- [Editor](./frontend) - the frontend this API backs
- [Corpus Library](./corpus) - the library used for law loading

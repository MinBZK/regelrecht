---
title: "Harvester Admin"
description: "The API that operators use to monitor and control the harvester pipeline. Its dashboard is part of the editor."
---

The harvester-admin service is the API operators use to watch and steer the harvester pipeline: which jobs are queued or failed, which laws are in which state, and what the enrichment found that could not be translated. It is a standalone Rust API. Its dashboard lives in the editor as the section labelled Harvester, at `/harvesting` (`frontend/src/harvester/`; code comments call it the Corpusinwinning section). The section reaches this API through the editor-api `/api/harvest-admin/*` proxy. Scripts and other services can call the API directly.

## Overview

- **Language**: Rust (Axum), API only. It serves no frontend; unmatched paths answer 404.
- **Location**: `packages/admin/` (API); UI in `frontend/src/harvester/`
- **Production URL**: `harvester-admin.regelrecht.rijks.app` (API); the UI is at `editor.regelrecht.rijks.app`, under `/harvesting`
- **Port**: 8000, or `ADMIN_PORT`
- **Database**: PostgreSQL, shared with the pipeline workers and the editor API. The service runs the pipeline migrations at startup and keeps its sessions in the same store as the editor API.

## Authorization

Every `/api/*` route has one of three role gates, checked by `require_auth` in `packages/admin/src/middleware.rs`:

| Tier | Keycloak role | Covers |
|------|---------------|--------|
| Reader | `harvester-reader` | All reads |
| Writer | `harvester-writer` | Enqueuing harvest and enrich jobs |
| Admin | `harvester-admin` | Deleting jobs, resetting exhausted laws, syncing a source |

Keycloak composite roles make each tier include the ones below it. Without a session the API answers 401, with a session but without the role 403.

There are two ways in:

1. **An OIDC session.** From the editor this is the session cookie the editor-api proxy forwards. Both services share the session store, so the roles in that session count here.
2. **The `ADMIN_API_KEY` bearer token**, for scripts and services: `Authorization: Bearer <key>`. A matching key passes every tier, including admin, but only for `GET`, `POST` and `DELETE`; any other method answers 403. A bearer token that does not match answers 401 straight away and is not retried as a session. The key works whether or not OIDC is configured.

Without OIDC configuration all role gates pass. The service logs a warning at startup; this mode is for local development only.

`/metrics` has its own guard: when `METRICS_AUTH_TOKEN` is set it requires that token as a bearer token, otherwise it is open. `/health` is always open and answers 503 when the database is unreachable.

## Endpoints

Derived from the router in `packages/admin/src/main.rs`; handlers in `handlers.rs` and `corpus_handlers.rs`.

### Service

| Method | Path | Gate |
|--------|------|------|
| GET | `/health` | Public |
| GET | `/metrics` | `METRICS_AUTH_TOKEN` when set |
| GET | `/auth/login`, `/auth/callback`, `/auth/logout`, `/auth/status` | Public (OIDC flow) |
| GET | `/api/info` | Reader. Returns `DEPLOYMENT_NAME` and `COMPONENT_NAME`. |

### Jobs

| Method | Path | Gate | Notes |
|--------|------|------|-------|
| GET | `/api/jobs` | Reader | Paginated. Filters: `status`, `job_type`, `law_id`; `sort`, `order`, `limit`, `offset` |
| GET | `/api/jobs/summary` | Reader | Job counts per law, same filters minus `law_id` |
| GET | `/api/jobs/{job_id}` | Reader | One job with payload, result and error |
| DELETE | `/api/jobs` | Admin | Body `{"job_ids": [...]}`, at most 64 KiB |
| GET | `/api/dashboard-stats` | Reader | Counts per type and status, jobs run today and in the last seven days, open untranslatables, recent failures, and a 14-day daily series |
| POST | `/api/harvest-jobs` | Writer | Body `{"law_id", "priority"?, "date"?}`; `bwb_id` is accepted as a legacy name for `law_id` |
| POST | `/api/enrich-jobs` | Writer | Body `{"law_id", "priority"?}`. Enqueues one job per enrichment provider. Needs a completed harvest; answers 409 when the law is `enrich_exhausted`. |

### Laws

| Method | Path | Gate | Notes |
|--------|------|------|-------|
| GET | `/api/law_entries` | Reader | Law status and coverage. Filters: `status`; `sort`, `order`, `limit`, `offset` |
| POST | `/api/law_entries/{law_id}/reset-exhausted` | Admin | Moves a `harvest_exhausted` or `enrich_exhausted` law back to the matching `*_failed` state, so it can be retried |

### Enrichment findings

The findings enrichment writes next to a law: constructs it could not translate, and the newer markings.

| Method | Path | Gate | Notes |
|--------|------|------|-------|
| GET | `/api/untranslatables` | Reader | Filters: `law_id`, `provider`, `accepted`, `construct` |
| GET | `/api/markings` | Reader | Filters: `law_id`, `provider`, `accepted`, `about`, `resolved_by`, `resolution` |
| GET | `/api/markings/clusters` | Reader | Markings grouped by requested change (`resolution`, `resolved_by`), with the providers that asked for it; same filters |

The list endpoints also take `sort`, `order`, `limit` and `offset`.

### Corpus sources

| Method | Path | Gate | Notes |
|--------|------|------|-------|
| GET | `/api/sources` | Reader | Sources from the corpus registry |
| GET | `/api/corpus/laws` | Reader | Laws loaded from the local sources |
| POST | `/api/sources/{source_id}/sync` | Admin | Reloads the local sources from disk. A GitHub source answers 501. |

## Configuration

From `packages/admin/src/config.rs`, `main.rs` and the shared OIDC parser in `packages/auth/src/config.rs`. Secret values belong in the deployment, never in the repository.

| Variable | Default | Purpose |
|----------|---------|---------|
| `DATABASE_URL` | none | Postgres connection; `DATABASE_SERVER_FULL` is accepted as fallback. Required: startup retries ten times, then stops. |
| `ADMIN_PORT` | `8000` | Listen port |
| `ADMIN_API_KEY` | none | Bearer key for programmatic access. A warning is logged when it is shorter than 32 characters. |
| `METRICS_AUTH_TOKEN` | none | Bearer token for `/metrics`; unset leaves the endpoint open |
| `CORPUS_REGISTRY_PATH` | `corpus-registry.yaml` | Registry manifest for `/api/sources` and `/api/corpus/laws` |
| `CORPUS_REGISTRY_LOCAL_PATH` | `corpus-registry.local.yaml` | Local overrides on the manifest |
| `DEPLOYMENT_NAME`, `COMPONENT_NAME` | empty | Reported by `/api/info` |
| `OIDC_CLIENT_ID` | none | Enables OIDC when set |
| `OIDC_CLIENT_SECRET` | none | Required once `OIDC_CLIENT_ID` is set |
| `OIDC_DISCOVERY_URL` | none | Issuer discovery URL; takes priority over the Keycloak pair |
| `KEYCLOAK_BASE_URL`, `KEYCLOAK_REALM` | none | Build the issuer as `<base>/realms/<realm>` when there is no discovery URL |
| `OIDC_REQUIRED_ROLE` | `allowed-user` | Minimum role to log in; set it to `harvester-reader` |
| `BASE_URL` | derived from request headers | Public origin for OIDC redirect URLs |
| `RUST_LOG`, `LOG_FORMAT` | `info`, text | Logging, as for the other services |

The session cookie is always marked `Secure`, so an OIDC login against this service needs HTTPS. Locally, log in through the editor instead.

## Running locally

```bash
DATABASE_URL=postgres://user:pass@localhost:5433/regelrecht just admin
```

The dashboard UI is served by the editor. For the full flow (editor, editor-api proxy, this API and the database) use:

```bash
just dev-frontend all
```

That runs this API on port 8001, so it does not clash with the editor API on 8000, and points the editor API's `HARVEST_ADMIN_URL` at it. Then open the editor and choose Harvester from the account menu (visible with any `harvester-*` role or `regelrecht-admin`).

## Further reading

- [Pipeline](./pipeline): the job queue this API reads and writes
- [Editor API](./editor-api): the proxy the dashboard goes through
- [Auth and roles](/auth-and-roles/): the role model
- [Deployment](/operations/deployment): how the service is deployed

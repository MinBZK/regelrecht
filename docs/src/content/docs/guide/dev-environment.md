---
title: "Development Environment"
description: "How the local stack runs: infrastructure in Docker and application services natively with hot reload."
---

## Architecture

The development stack runs infrastructure in Docker and application services natively with hot reload:

```
┌─────────────────────────────────────────────────┐
│  Native (hot reload)                            │
│  ┌──────────────┐ ┌──────────────┐              │
│  │ Editor :3000 │ │Admin API:8000│              │
│  │ (Vite; hosts │ │(cargo watch; │              │
│  │  Beheer UI)  │ │ API only)    │              │
│  └──────────────┘ └──────────────┘              │
├─────────────────────────────────────────────────┤
│  Docker                                         │
│  ┌──────────┐ ┌────────────┐ ┌───────┐         │
│  │PostgreSQL│ │ Prometheus │ │Grafana│         │
│  │  :5433   │ │   :9090    │ │ :3002 │         │
│  └──────────┘ └────────────┘ └───────┘         │
└─────────────────────────────────────────────────┘
```

## One-Time Setup (build speed)

Run once per machine after cloning:

```bash
just dev-setup
```

It installs the [mold](https://github.com/rui314/mold) linker (a hard
requirement; the dev recipes won't link without it) plus `sccache`, and points
every git worktree at a single shared cargo `target-dir` so a new worktree
reuses the already-built dependency graph instead of cold-building from scratch.
When the repo lives on a slow mount (9p/NFS/SMB, e.g. a WSL2 or Docker-Desktop
dev container backed by a Windows drive), it relocates that target dir to fast
local storage under `~/.cache/regelrecht/`, which is usually the single biggest
build-time win. `sccache` is installed but left off locally (it disables
incremental compilation, which hurts the hot-reload loop); CI uses both.

## Starting the Dev Stack

```bash
just dev
```

This command:
1. Checks prerequisites (cargo, node, docker, cargo-watch, mold)
2. Starts infrastructure containers (PostgreSQL, Prometheus, Grafana)
3. Waits for PostgreSQL to be ready
4. Installs frontend dependencies if needed
5. Starts all application services with hot reload

## Frontend-Focused Dev Stack

When you only need to work on a frontend, `just dev-frontend` starts just the
components that frontend needs (its backend, PostgreSQL, the engine WASM, and
the Vite dev server with HMR) and skips Grafana, Prometheus, and the workers.

```bash
just dev-frontend            # all frontends at once (default)
just dev-frontend editor     # just the editor
just dev-frontend admin      # just the admin dashboard
just dev-frontend lawmaking  # just the lawmaking UI (no backend)
just dev-down                # stop it (shared with `just dev`)
```

| App | URL | Backend | DB | Notes |
|-----|-----|---------|----|----|
| editor | `http://localhost:7300` | editor-api `:8000` | yes | real SSO, needs `.env.sso-local`; hosts the harvester **Beheer** UI |
| harvester-admin | API only (UI is the editor's Beheer section) | admin API `:8000` (`:8001` when all run together) | yes | in `all`, editor-api proxies `/api/harvest-admin/*` here |
| lawmaking | `http://localhost:7500` | none | no | static, no backend |

Notes:

- **Backends run once** via `cargo run` (not `cargo watch`); Vite keeps HMR for
  the frontend. Restarts after the first build are near-instant because the
  Rust artifacts are reused (see [One-Time Setup](#one-time-setup-build-speed)).
- **The editor uses real SSO** against the central Keycloak, so it needs
  `.env.sso-local` (copy `.env.sso-local.example` and fill in the values, see
  [Auth and roles](/auth-and-roles/)). Use Chrome or Firefox: the session cookie
  is `Secure` and only those send it over `http://localhost`. The default port
  `7300` (and `7500`) are the redirect URIs already registered on the
  `regelrecht-local` Keycloak client. Override ports with `EDITOR_PORT` /
  `LAWMAKING_PORT`.
- `just dev-frontend` and `just dev` are **mutually exclusive**: they share
  `.dev-pids` and ports, so run one at a time. `just dev-down` stops either.
- In a dev container where the native backend can't reach Postgres on
  `localhost`, set `DB_HOST=host.docker.internal` in `.env` (admin / `just dev`
  paths); the editor takes that host from `DATABASE_URL` in `.env.sso-local`.

## Stopping

```bash
just dev-down
```

## Logs

```bash
tail -f .dev-admin.log           # Admin (harvester) API log
tail -f .dev-editor.log          # Editor log (hosts the harvester Beheer UI)
just dev-logs                    # Infrastructure logs
```

## Database Access

```bash
just dev-psql
```

## Full Docker Stack

For running everything in Docker without hot reload:

```bash
just local          # Start
just local-down     # Stop
just local-logs     # Logs
just local-psql     # Database access
```

## Environment Variables

Create a `.env` file in the project root:

```bash
# Optional overrides
POSTGRES_PORT=5433
GRAFANA_PORT=3002
PROMETHEUS_PORT=9090
RUST_LOG=info
```

## Pre-commit Hooks

Install pre-commit hooks:

```bash
pre-commit install
```

Hooks run automatically on commit:
- Trailing whitespace, end-of-file fixes
- YAML linting
- Rust formatting (`just format`)
- Rust linting (`just lint`)
- Schema validation (`just validate`)

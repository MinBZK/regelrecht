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
│  │   (Vite)     │ │(cargo watch) │              │
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

[Getting Started](./getting-started) lists the prerequisites. Then, once per
machine after cloning:

```bash
just dev-setup
```

It installs `sccache`, plus the [mold](https://github.com/rui314/mold) linker
on x86_64 Linux (through apt, dnf or Homebrew, whichever it finds), and points every git
worktree at a single shared cargo `target-dir` so a new worktree reuses the
already-built dependency graph instead of cold-building from scratch. That
setting lands in a gitignored `.cargo/config.toml` at the root of the main
checkout.

When the repo is on a slow mount (9p/NFS/SMB, e.g. a WSL2 or Docker-Desktop
dev container backed by a Windows drive), it relocates that target dir to fast
local storage under `~/.cache/regelrecht/`, which is usually the biggest
build-time win. `sccache` is installed but left off locally (it disables
incremental compilation, which hurts the hot-reload loop); CI uses both.

mold is the configured linker on x86_64 Linux (`packages/.cargo/config.toml`),
so builds there fail to link without it. On that platform `just dev`, and
`just dev-frontend` whenever it starts a Rust service, refuse to start when mold
is missing. On macOS and aarch64 Linux cargo uses the default linker, and
neither `just dev-setup` nor the dev recipes ask for mold.

## Starting the Dev Stack

```bash
just dev
```

| Service | URL | Description |
|---------|-----|-------------|
| Editor | http://localhost:3000 | Law editor + **Corpusinwinning** section (hot reload) |
| Admin API | http://localhost:8000 | Harvester REST API (auto-recompile; UI is the editor's Corpusinwinning section) |
| Grafana | http://localhost:3002 | Metrics dashboard |
| Prometheus | http://localhost:9090 | Metrics collection |
| PostgreSQL | localhost:5433 | Database |

This command:
1. Checks prerequisites (cargo, node, docker, cargo-watch, and mold on x86_64 Linux)
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
just dev-frontend admin      # just the harvester-admin API
just dev-frontend lawmaking  # just the lawmaking UI (no backend)
just dev-down                # stop it (shared with `just dev`)
```

| App | URL | Backend | DB | Notes |
|-----|-----|---------|----|----|
| editor | `http://localhost:7300` | editor-api `:8000` | yes | real SSO, needs `.env.sso-local`; hosts the **Corpusinwinning** section |
| harvester-admin | API only (UI is the editor's Corpusinwinning section) | admin API `:8000` (`:8001` when all run together) | yes | in `all`, editor-api proxies `/api/harvest-admin/*` here |
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
tail -f .dev-editor.log          # Editor log (hosts the Corpusinwinning section)
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

### Logging

Five binaries read these variables: editor-api, admin, and the three pipeline
binaries (harvest worker, enrich worker, pipeline API). The harvester CLI builds
its own subscriber and reads only `RUST_LOG`.

| Variable | Values | Default | Effect |
|---|---|---|---|
| `RUST_LOG` | `tracing` filter | `info` (harvester CLI: `warn`) | Which events are emitted |
| `LOG_FORMAT` | `text` (`plain`), `json` | `text` | Output format |
| `LOG_SPAN_EVENTS` | `none`, `close`, `new`, `active`, `full` | per service: `close` for editor-api, `none` elsewhere | Per-span timing lines |

`LOG_FORMAT=json` writes one JSON object per event. The event's own fields are
flattened to the top level; the enclosing spans are added as nested `span` and
`spans` keys, so a log backend can search per field. Set it per deployment in
ZAD; locally the text lines read better, so leave the variable unset. An
unrecognized value falls back to text and warns on stderr, so a typo never
silences logging.

## Architecture Explorer

`just arch-explore` builds and starts a local explorer of the codebase on port 7180 (override with `ARCH_EXPLORE_PORT`). It renders a model of the Rust workspace and the Vue frontends, from crate down to method and from app down to component, with the dependencies between them. The model comes from `packages/arch-extract/`, a developer tool that is not deployed. It is generated from the working tree on demand and never committed, so it cannot go stale; `just arch-generate` writes it to disk for inspection. `packages/arch-extract/README.md` explains how the edges are resolved and what the explorer misses.

## Pre-commit Hooks

Install [pre-commit](https://pre-commit.com/) (for example with
`uv tool install pre-commit`), then register the hooks in your clone:

```bash
pre-commit install --hook-type pre-commit --hook-type commit-msg
```

The `commit-msg` type matters. The Conventional Commits check on the commit
message runs at that stage, and a plain `pre-commit install` registers only the
`pre-commit` stage, so that check would never run locally.

On commit the hooks run, each only when a matching file changed:

- Trailing whitespace, end-of-file, merge-conflict and large-file checks
- YAML linting (yamllint, config in `.yamllint`)
- Rust formatting (`just format`) and clippy (`just lint`)
- Schema validation of corpus files (`just validate`)
- The test suites of the CI scripts and merge gates under `script/`, when that
  script or its workflow changed

`.pre-commit-config.yaml` has the full list. What to do when a hook fails is on
[Contributing](/operations/contributing#pre-commit-hooks).

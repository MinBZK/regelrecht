# Development Environment

## Architecture

The development stack runs infrastructure in Docker and application services natively with hot reload:

```
┌─────────────────────────────────────────────────┐
│  Native (hot reload)                            │
│  ┌─────────────┐ ┌──────────┐ ┌──────────────┐ │
│  │ Editor :3000 │ │Admin :3001│ │Admin API:8000│ │
│  │ (Vite)       │ │(Vite)    │ │(cargo watch) │ │
│  └─────────────┘ └──────────┘ └──────────────┘ │
├─────────────────────────────────────────────────┤
│  Docker                                         │
│  ┌──────────┐ ┌────────────┐ ┌───────┐         │
│  │PostgreSQL│ │ Prometheus │ │Grafana│         │
│  │  :5433   │ │   :9090    │ │ :3002 │         │
│  └──────────┘ └────────────┘ └───────┘         │
└─────────────────────────────────────────────────┘
```

## Starting the Dev Stack

```bash
just dev
```

This command:
1. Checks prerequisites (cargo, node, docker, cargo-watch)
2. Starts infrastructure containers (PostgreSQL, Prometheus, Grafana)
3. Waits for PostgreSQL to be ready
4. Installs frontend dependencies if needed
5. Starts all application services with hot reload

## Stopping

```bash
just dev-down
```

## Logs

```bash
tail -f .dev-admin.log           # Admin API log
tail -f .dev-admin-frontend.log  # Admin frontend log
tail -f .dev-editor.log          # Editor log
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
# Required for @minbzk/storybook package
GITHUB_TOKEN=ghp_...

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

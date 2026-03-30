# Getting Started

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [just](https://github.com/casey/just) command runner
- [Node.js](https://nodejs.org/) (for frontend development)
- [Docker](https://www.docker.com/) (for pipeline integration tests and full stack)

## Clone and Build

```bash
git clone https://github.com/MinBZK/regelrecht.git
cd regelrecht
```

## Quick Check

Run all quality checks to verify your setup:

```bash
just check
```

This runs formatting, linting, schema validation, and all tests.

## Development Stack

Start the full development environment with hot reload:

```bash
just dev
```

This starts:

| Service | URL | Description |
|---------|-----|-------------|
| Editor | http://localhost:3000 | Law editor (hot reload) |
| Admin UI | http://localhost:3001 | Admin dashboard (hot reload) |
| Admin API | http://localhost:8000 | REST API (auto-recompile) |
| Grafana | http://localhost:3002 | Metrics dashboard |
| Prometheus | http://localhost:9090 | Metrics collection |
| PostgreSQL | localhost:5433 | Database |

Stop everything with:

```bash
just dev-down
```

## Common Commands

```bash
just              # List all available commands
just format       # Check Rust formatting
just lint         # Run clippy lints
just test         # Run unit tests
just bdd          # Run BDD tests (cucumber-rs)
just validate     # Validate law YAML files
just bench        # Run performance benchmarks
```

## Project Structure

```
regelrecht/
├── packages/
│   ├── engine/       # Rust execution engine
│   ├── pipeline/     # PostgreSQL job queue
│   ├── harvester/    # BWB law downloader
│   └── admin/        # Admin API + frontend
├── frontend/         # Law editor (Vue 3 + Vite)
├── corpus/           # Machine-readable laws (YAML)
├── features/         # BDD test scenarios (Gherkin)
├── schema/           # Law format JSON schema
└── docs/             # Documentation site (VitePress) + RFCs
```

## Next Steps

- [Law Format](./law-format) — understand how laws are structured
- [Testing](./testing) — how to write and run tests
- [Architecture](../architecture/overview) — system design

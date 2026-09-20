---
title: "Getting Started"
description: "From a fresh clone to a built engine and a passing test suite."
---

## Prerequisites

- [Rust](https://rustup.rs/) (the exact version is pinned in `rust-toolchain.toml`; rustup picks it up automatically)
- [just](https://github.com/casey/just) command runner
- [Node.js](https://nodejs.org/) (for frontend development)
- [Docker](https://www.docker.com/) (for pipeline integration tests and full stack)
- [mold](https://github.com/rui314/mold) on x86_64 Linux, including dev containers. `packages/.cargo/config.toml` passes `-fuse-ld=mold` there, so linking fails without it.

Run `just dev-setup` once before your first build. It installs mold, points every worktree at one shared target directory, and moves that directory to fast local storage when the repo sits on a slow mount. See [Dev Environment](./dev-environment) for what it does and why.

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
| Editor | http://localhost:3000 | Law editor + **Corpusinwinning** section (hot reload) |
| Admin API | http://localhost:8000 | Harvester REST API (auto-recompile; UI is the editor's Corpusinwinning section) |
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
│   ├── engine/           # Rust execution engine
│   ├── law-model/        # Rust implementation of the schema contract
│   ├── pipeline/         # PostgreSQL job queue
│   ├── harvester/        # BWB law downloader
│   ├── editor-api/       # Backend for the law editor
│   └── admin/            # Harvester-admin API
├── frontend/             # Law editor (Vue 3 + Vite)
├── frontend-lawmaking/   # Law-making process visualization
├── frontend-demo/        # The demo, engine as WASM in the browser
├── corpus/               # Machine-readable laws (YAML)
├── bdd/                  # Canonical BDD grammar + conformance features
├── schema/               # Law format JSON schema
└── docs/                 # Documentation site (Astro) + RFCs
```

## Next steps

- [Law Format](/concepts/law-format) - understand how laws are structured
- [Testing](./testing) - how to write and run tests
- [Architecture](./architecture) - system design

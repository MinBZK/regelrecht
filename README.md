# regelrecht

[![CI](https://github.com/MinBZK/regelrecht/actions/workflows/ci.yml/badge.svg)](https://github.com/MinBZK/regelrecht/actions/workflows/ci.yml)
[![License: EUPL-1.2](https://img.shields.io/badge/License-EUPL--1.2-blue.svg)](https://opensource.org/licenses/EUPL-1.2)
[![Rust 2021](https://img.shields.io/badge/Rust-2021_edition-orange.svg)](https://doc.rust-lang.org/edition-guide/rust-2021/)
[![Mutation Testing](https://github.com/MinBZK/regelrecht/actions/workflows/mutation-testing.yml/badge.svg)](https://github.com/MinBZK/regelrecht/actions/workflows/mutation-testing.yml)
[![Docs](https://img.shields.io/badge/docs-regelrecht-green.svg)](https://docs.regelrecht.rijks.app)

Machine-readable Dutch law execution. regelrecht takes legal texts, encodes them as structured YAML, and runs them as deterministic decision logic.

## What does it do

- The engine takes a regulation and a set of inputs, evaluates the decision logic, and returns a result with a full explanation trail
- Laws are tested against real-world scenarios using BDD (Gherkin) tests, many derived from legislative explanatory memoranda
- A harvester downloads and tracks Dutch legislation from the official BWB repository
- Regulations can be edited through a web UI with live execution preview

## Components

### Rust packages

| Package | Description |
|---------|-------------|
| [packages/engine/](packages/engine/) | Law execution engine (also compiles to WASM) |
| [packages/harvester/](packages/harvester/) | Downloads Dutch legislation from BWB |
| [packages/pipeline/](packages/pipeline/) | PostgreSQL job queue for law processing |
| [packages/admin/](packages/admin/) | Admin dashboard API (Axum) |
| [packages/editor-api/](packages/editor-api/) | Backend API for the law editor |
| [packages/corpus/](packages/corpus/) | Git integration for the regulation corpus |
| [packages/shared/](packages/shared/) | Shared domain types across crates |
| [packages/tui/](packages/tui/) | Terminal dashboard (Ratatui) |

### Frontends and sites

| Directory | Description |
|-----------|-------------|
| [frontend/](frontend/) | Law editor UI (Vue 3 + Vite) |
| [frontend-lawmaking/](frontend-lawmaking/) | Law-making process visualization (Vue 3 + Vite) |
| [landing/](landing/) | Landing page |
| [docs/](docs/) | Documentation site (VitePress) |

### Data and testing

| Directory | Description |
|-----------|-------------|
| [corpus/regulation/](corpus/regulation/) | Dutch regulations in machine-readable YAML |
| [schema/](schema/) | Versioned JSON schema for the law format (current: v0.5.1) |
| [features/](features/) | Gherkin BDD scenarios for law execution |
| [packages/grafana/](packages/grafana/) | Grafana monitoring dashboards |

## Deployed services

| Service | URL |
|---------|-----|
| Editor | https://editor.regelrecht.rijks.app |
| Landing page | https://regelrecht.rijks.app |
| Documentation | https://docs.regelrecht.rijks.app |
| Law-making | https://lawmaking.regelrecht.rijks.app |
| Harvester admin | https://harvester-admin.regelrecht.rijks.app |
| Grafana | https://grafana.regelrecht.rijks.app |

PR preview environments are deployed automatically and cleaned up when the PR is closed.

## Getting started

Prerequisites: [Rust](https://rustup.rs/) (stable) and [just](https://github.com/casey/just).

```bash
just check    # run all quality checks (format, lint, build, validate, tests)
just test     # unit tests only
just bdd      # BDD tests only
```

See the [docs site](https://docs.regelrecht.rijks.app) for full development instructions.

## License

[EUPL-1.2](LICENSE)

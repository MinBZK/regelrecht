# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**regelrecht** is a platform for machine-readable Dutch law execution. The repo is a monorepo with multiple components:

- `packages/engine/` - Rust law execution engine
- `packages/pipeline/` - PostgreSQL-backed job queue and law status tracking
- `packages/harvester/` - Law corpus harvesting from BWB (Basis Wettelijke Regelgeving)
- `packages/admin/` - Admin dashboard (Rust API + Vue frontend)
- `packages/editor-api/` - Rust backend API for the editor frontend
- `packages/corpus/` - Shared library for working with YAML regulation files
- `packages/shared/` - Common types/utilities across packages
- `packages/tui/` - Terminal UI dashboard
- `packages/grafana/` - Grafana monitoring with provisioned dashboards
- `frontend/` - Law editor (Vue/Vite + editor-api backend)
- `frontend-lawmaking/` - Law-making process visualization (Vue/Vite)
- `landing/` - Static landing page (regelrecht.rijks.app)
- `docs/` - Documentation site (VitePress)
- `corpus/regulation/` - Dutch legal regulations in machine-readable YAML format
- `corpus-demo/` - Demo-only corpus additions (personas, demo-index, optional demo-only YAML) for the burger-demo
- `features/` - Gherkin BDD feature files (used by Rust cucumber-rs)
- `frontend-demo/` - Burger-demo Vue-app (alleen RR-Components; WASM-engine)
- `packages/demo-api/` - Axum LLM-proxy voor burger-demo (POST /api/explain)

## Development Setup

### Prerequisites
- [Rust](https://rustup.rs/) (stable toolchain)
- [just](https://github.com/casey/just) command runner

### Just Commands

**IMPORTANT FOR CLAUDE CODE:** All `just` commands have pre-authorized permissions configured in the project settings. Always use `just` commands to avoid unnecessary permission prompts.

```bash
just            # List all available commands
just format     # Check Rust formatting (cargo fmt --check)
just lint        # Run clippy lints on all packages
just build-check # Run cargo check on all packages
just validate    # Validate regulation YAML files (all, or pass specific files)
just check       # Run all quality checks (format + lint + check + validate + tests)
just test       # Run Rust unit tests
just bdd        # Run Rust BDD tests (cucumber-rs)
just test-all   # Run all tests (unit + BDD + harvester + pipeline)

# Pipeline commands
just pipeline-test              # Run pipeline unit tests (no Docker/DB required)
just pipeline-integration-test  # Run pipeline integration tests (requires Docker for testcontainers)
```

### Pre-commit Hooks

This repository uses pre-commit hooks for code quality:
- **Standard hooks**: Trailing whitespace, end-of-file fixer, YAML checks, etc.
- **yamllint**: YAML linting (config in `.yamllint`)
- **Rust formatting**: `just format` (on `.rs` files)
- **Rust linting**: `just lint` (on `.rs` files)
- **Schema validation**: `just validate` (on `corpus/regulation/**/*.yaml` files)

**NEVER use `--no-verify` when committing.** Fix the underlying problem instead of bypassing hooks.

**No branding in commits.** Do not add "Generated with Claude Code" or "Co-Authored-By: Claude" lines to commit messages.

### Git Worktrees

When using git worktrees, create them **inside the project folder** (e.g., `.worktrees/`).

```bash
git worktree add .worktrees/feature-branch feature-branch
```

## Architecture Notes

### Law Format

Laws are stored as article-based YAML files conforming to the official JSON schema:
- Schema: `https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.4.0/schema.json`

### Cross-Law References

Laws reference each other via `source` on input fields:

```yaml
source:
  regulation: "other_law_id"   # External law $id
  output: "output_name"        # Output field to retrieve
  parameters:
    bsn: $bsn                  # Parameters to pass
```

For delegated values (e.g., "bij ministeriële regeling"), laws use the IoC pattern:
higher laws declare `open_terms`, lower regulations declare `implements`.
See `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml` for a working example.

## RFC Process

This project uses an RFC process for design decisions.

- **Location**: `docs/rfcs/`
- **Process document**: See `docs/rfcs/rfc-000.md`
- **Template**: Use `docs/rfcs/template.md`

### When to Write an RFC

Write an RFC for:
- Law representation format changes
- Execution engine architecture changes
- Cross-cutting design patterns
- Integration patterns between components

## Burger-demo principes

De burger-demo (`frontend-demo/` + `corpus-demo/` + `packages/demo-api/`) is bewust geïsoleerd van het hoofdcorpus: eigen corpus-directory, WASM-in-browser executie, en een aparte Axum LLM-uitlegproxy.

- **Demo-YAML mag afwijken van de wet** mits iedere afwijking in een README bij de wet wordt gedocumenteerd.
- **Law-faithful wetten niet kopiëren** — de demo-index (`corpus-demo/demo-index.yaml`) verwijst naar `corpus/regulation/`.
- **Geen demo-hacks in de hoofd-engine of het hoofdcorpus.** Als de demo iets mist, wordt dat een issue op de engine, niet een uitzondering.
- `just validate` raakt `corpus-demo/` niet.

## Code Reviews

After completing significant code changes, proactively use the `code-reviewer` skill to review changes before committing.

**Important:** Run the code review in a subagent using the Task tool with `subagent_type: "general-purpose"`.

## Technology Stack

- **Engine**: Rust
- **BDD Testing**: cucumber-rs with Gherkin feature files
- **Code Quality**: pre-commit hooks, yamllint
- **Deployment**: RIG (Quattro/rijksapps) via GitHub Actions

## CI/CD Deployment

All components are deployed to ZAD (RIG/Quattro/rijksapps) via `.github/workflows/deploy.yml`.
CI runs via `.github/workflows/ci.yml`.

### Deployed Components

| Component | Image | Production URL |
|-----------|-------|----------------|
| editor | `regelrecht-editor` | `editor.regelrecht.rijks.app` |
| harvester-admin | `regelrecht-admin` | `harvester-admin.regelrecht.rijks.app` |
| harvester-worker | `regelrecht-harvester-worker` | (no web UI) |
| enrichworker | `regelrecht-enrich-worker` | (no web UI) |
| landing | `regelrecht-landing` | `regelrecht.rijks.app` |
| lawmaking | `regelrecht-lawmaking` | `lawmaking.regelrecht.rijks.app` |
| docs | `regelrecht-docs` | `docs.regelrecht.rijks.app` |
| grafana | `regelrecht-grafana` | `grafana.regelrecht.rijks.app` |
| demo | `regelrecht-demo` | `demo.regelrecht.rijks.app` |
| demo-api | `regelrecht-demo-api` | `demoapi.regelrecht.rijks.app` |

### How It Works

1. **PR opened/updated**: Builds changed Docker images, pushes to GHCR, deploys `prN` to ZAD
2. **PR closed**: Deletes ZAD deployment and GHCR images
3. **Push to main**: Deploys `regelrecht` (production) to ZAD

### Debugging deploy-preview failures

ZAD deploy timeouts ("Task did not complete within 300s") almost always indicate an **application error**, not a platform issue. When `deploy-preview` fails:

1. Check container logs: `zad logs <deployment>` (e.g. `zad logs pr429`)
2. Look for ERROR lines — common causes: migration conflicts, missing env vars, startup panics
3. If the DB is in a bad state (e.g. migration checksum mismatch after renumbering), delete the preview deployment (`zad deployment delete <deployment>`) and re-trigger CI to get a fresh DB
4. Do **not** blindly retry — diagnose the root cause first

### Required Secrets

- `RIG_API_KEY` - API key for ZAD Operations Manager (configured in GitHub secrets)

### ZAD CLI

Use [`zad-cli`](https://github.com/RijksICTGilde/zad-cli) to manage deployments. Configure `ZAD_API_KEY` and `ZAD_PROJECT_ID` in `.env`.

```bash
# Install / upgrade
uv tool install git+https://github.com/RijksICTGilde/zad-cli.git
uv tool upgrade zad-cli

# Add a new component
zad component add landing \
    --image ghcr.io/minbzk/regelrecht-landing:latest \
    --deployment regelrecht \
    --port 8000 \
    --service publish-on-web

# Get logs
zad logs --deployment regelrecht --lines 50

# List deployments
zad deployment list
```

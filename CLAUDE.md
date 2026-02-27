# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**regelrecht-mvp** is an MVP for machine-readable Dutch law execution. Components:
- `packages/engine/` - Rust law execution engine
- `packages/pipeline/` - PostgreSQL-backed job queue and law status tracking
- `regulation/` - Dutch legal regulations in machine-readable YAML format
- `frontend/` - Static HTML/CSS law editor prototype
- `features/` - Gherkin BDD feature files (used by Rust cucumber-rs)

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
just test-all   # Run all tests (unit + BDD + harvester)

# Pipeline commands
just pipeline-test    # Run pipeline tests (requires Docker for testcontainers)
just pipeline-check   # Check pipeline compilation (no DB needed)
just db-up            # Start local PostgreSQL for pipeline development
just db-down          # Stop local PostgreSQL
just db-migrate       # Run database migrations
```

### Pre-commit Hooks

This repository uses pre-commit hooks for code quality:
- **Standard hooks**: Trailing whitespace, end-of-file fixer, YAML checks, etc.
- **yamllint**: YAML linting (config in `.yamllint`)
- **Rust formatting**: `just format` (on `.rs` files)
- **Rust linting**: `just lint` (on `.rs` files)
- **Schema validation**: `just validate` (on `regulation/**/*.yaml` files)

**NEVER use `--no-verify` when committing.** Fix the underlying problem instead of bypassing hooks.

**No branding in commits.** Do not add "Generated with Claude Code" or "Co-Authored-By: Claude" lines to commit messages.

### Git Worktrees

When using git worktrees, create them **inside the project folder** (e.g., `.worktrees/`).

```bash
git worktree add .worktrees/feature-branch feature-branch
```

## Architecture Notes

### Directory Structure

- **packages/engine/** - Rust law execution engine (cargo workspace member)
- **packages/pipeline/** - PostgreSQL-backed job queue and law status tracking (Rust)
  - `src/job_queue.rs` - Job creation, claiming (`FOR UPDATE SKIP LOCKED`), completion, failure with auto-retry
  - `src/law_status.rs` - Per-law status tracking from `unknown` through `harvested` to `enriched`
  - `src/models.rs` - Data types: `Job`, `LawEntry`, `JobType`, `JobStatus`, `LawStatusValue`, `Priority`
  - `src/config.rs` - Configuration from `DATABASE_URL` env var
  - `src/db.rs` - Connection pool creation and migration runner
  - `src/error.rs` - Error types (`PipelineError`)
  - `migrations/` - SQL migrations (run via `just db-migrate`)
- **frontend/** - Static HTML/CSS law editor prototype
  - `index.html` - Law browser page
  - `editor.html` - Law editor page
  - `Dockerfile` - Multi-stage build (Node.js + nginx-unprivileged)
  - `nginx.conf` - Nginx config (port 8000, gzip, caching)
- **regulation/nl/** - Dutch legal regulations in machine-readable format
  - `wet/` - Formal laws (wetten)
  - `ministeriele_regeling/` - Ministerial regulations
- **features/** - Gherkin feature files for BDD testing

### Law Format

Laws are stored as article-based YAML files conforming to the official JSON schema:
- Schema: `https://raw.githubusercontent.com/MinBZK/poc-machine-law/refs/heads/main/schema/v0.2.0/schema.json`

### Cross-Law References

Laws reference each other using `regelrecht://` URIs:

**Format:** `regelrecht://{law_id}/{output_name}#{field}`

The engine resolves these URIs by finding the law by `$id` slug, finding the article by output name, executing the logic, and extracting the requested field.

## RFC Process

This project uses an RFC process for design decisions.

- **Location**: `doc/rfcs/`
- **Process document**: See `doc/rfcs/RFC-000-rfc-process.md`
- **Template**: Use `doc/rfcs/RFC-TEMPLATE.md`

### When to Write an RFC

Write an RFC for:
- Law representation format changes
- Execution engine architecture changes
- Cross-cutting design patterns
- Integration patterns between components

## Code Reviews

After completing significant code changes, proactively use the `code-reviewer` skill to review changes before committing.

**Important:** Run the code review in a subagent using the Task tool with `subagent_type: "general-purpose"`.

## Technology Stack

- **Engine**: Rust
- **BDD Testing**: cucumber-rs with Gherkin feature files
- **Code Quality**: pre-commit hooks, yamllint
- **Deployment**: RIG (Quattro/rijksapps) via GitHub Actions

## CI/CD Deployment

The frontend is automatically deployed to RIG via `.github/workflows/deploy.yml`.
CI runs via `.github/workflows/ci.yml`.

### Environments

| Environment | Deployment Name | URL |
|-------------|-----------------|-----|
| Production | `regelrecht` | https://editor-regelrecht-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl |
| PR Preview | `prN` | https://editor-prN-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl |

### How It Works

1. **PR opened/updated**: Builds Docker image, pushes to GHCR, deploys `prN` to RIG
2. **PR closed**: Deletes RIG deployment and GHCR image
3. **Push to main**: Deploys `regelrecht` (production) to RIG

### Gemini AI PR Review

Automated code reviews via `gemini-dispatch.yml` and `gemini-review.yml`:
- 5 parallel review agents: correctness, quality, security, tests, docs
- Triggers on PR open, push to PR, or `@gemini-cli /review` comment
- Draft PRs are skipped; review starts when marked ready
- Prompts in `.gemini/prompts/gemini-review-*.toml`

### Required Secrets

- `RIG_API_KEY` - API key for RIG Operations Manager (configured in GitHub secrets)
- `GEMINI_API_KEY` - Google Gemini API key for AI code reviews

### Docker Image

- Base: `nginxinc/nginx-unprivileged:alpine`
- Port: 8000 (required by RIG liveprobe)
- Registry: `ghcr.io/minbzk/regelrecht-mvp`

### RIG API

**API Docs:** https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/docs

**Base URL:** `https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/api`

#### Get Deployment Logs

```bash
# Get logs for a specific deployment (lines: 1-1000, default 10)
curl -H "X-API-Key: $RIG_API_KEY" \
  "https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/api/logs/regel-k4c?deployment=pr73&lines=50"

# Get logs for production
curl -H "X-API-Key: $RIG_API_KEY" \
  "https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/api/logs/regel-k4c?deployment=regelrecht&lines=50"
```

#### Other Commands

```bash
# Refresh project (sync config)
curl -H "X-API-Key: $RIG_API_KEY" \
  "https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/api/projects/regel-k4c/:refresh"

# Upsert deployment
curl -X POST -H "X-API-Key: $RIG_API_KEY" -H "Content-Type: application/json" \
  "https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/api/projects/regel-k4c/:upsert-deployment" \
  -d '{"deploymentName": "pr73", "components": [{"reference": "editor", "image": "ghcr.io/minbzk/regelrecht-mvp:pr-73"}]}'

# Delete deployment
curl -X DELETE -H "X-API-Key: $RIG_API_KEY" \
  "https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/api/projects/regel-k4c/pr73"
```

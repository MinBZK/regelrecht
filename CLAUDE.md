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
- `docs/` - Astro site serving both the landing page (regelrecht.rijks.app) and the docs (docs.regelrecht.rijks.app)
- `corpus/regulation/` - Dutch legal regulations in machine-readable YAML format
- `features/` - Gherkin BDD feature files (used by Rust cucumber-rs)

## Development Setup

### Prerequisites
- [Rust](https://rustup.rs/) (stable toolchain)
- [just](https://github.com/casey/just) command runner
- [mold](https://github.com/rui314/mold) linker — required by `packages/.cargo/config.toml`; the dev recipes will not link without it

### Build speed (run once)

Run `just dev-setup`. It points all worktrees at one shared target dir (no cold
build per worktree) and, when the repo is on a slow mount (9p/NFS/SMB — e.g. a
WSL2/Docker-Desktop dev container with the repo on a Windows drive), relocates
that target to fast local storage under `~/.cache/regelrecht/`. The slow-mount
I/O is usually the dominant cost — bigger than mold or debuginfo. It also
installs `mold` (required by `packages/.cargo/config.toml`) + `sccache`.
`sccache` is left off locally because it disables incremental compilation
(`CARGO_INCREMENTAL=0`), which slows the `just dev` hot-reload loop. CI uses
mold + sccache.

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

### Commit & PR Title Conventions

PR titles are linted by `.github/workflows/pr-title.yml` (a failing title blocks
merge). The format is **Conventional Commits**: `type(scope): subject`, where
`(scope)` is optional. Commit messages should follow the same shape.

- **Allowed types**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`,
  `test`, `chore`, `build`, `ci`.
- **Allowed scopes** (optional): `engine`, `admin`, `pipeline`, `harvester`,
  `editor`, `corpus`, `frontend`, `lawmaking`, `docs`, `grafana`, `ci`,
  `schema`, `deps`, `dev`. An unlisted scope fails the lint.
- **The subject MUST start with a lowercase letter** (`subjectPattern:
  ^[a-z].*$`). This is the easiest rule to trip on: `docs: RFC-…` fails because
  "RFC" is uppercase — write `docs: verwijzingen naar RFC's …` instead. Editing
  the PR title re-runs the check; no new commit needed.

Per the global convention these subjects are written in **Dutch** (PR
descriptions too), while code identifiers stay English.

### Test Data

**Never use real secret or private information in tests.** This is a public
repository — anything in a test fixture is published. Do not put names of
private repositories, internal hostnames, credentials, tokens, real BSNs/personal
data, or any reference to a private working environment into test fixtures,
assertions, comments, or sample data. Use clearly-fictional placeholders instead
(e.g. `example-org/regelrecht-corpus-example`). When a test needs to model a
private/traject-owned source, anonymize the identifiers — the test should prove
the behavior, not leak where the real data lives.

### Git Worktrees

When using git worktrees, create them **inside the project folder** (e.g., `.worktrees/`).

```bash
git worktree add .worktrees/feature-branch feature-branch
```

## Architecture Notes

### Law Format

Laws are stored as article-based YAML files conforming to the official JSON schema:
- Schema: `schema/latest/schema.json` (symlink to the current version directory in this repo)

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

## Frontend / UI Components

**All user interface MUST be built with components from the MinBZK design system: https://github.com/MinBZK/storybook** (the NDD `ndd-*` web components). Do not hand-roll custom UI elements when a design-system component exists. For the required component hierarchy, nesting rules, and layout patterns, use the `storybook-component-hierarchy` skill.

### When you can't build what you want with these components

Do **not** silently improvise. Follow these steps in order:

1. **Reconsider the design choice.** Investigate whether the design should be different. Try to conform to existing choices already made elsewhere in regelrecht (look at how other views/components in `frontend/` and `frontend-lawmaking/` solved similar problems) before introducing anything new.
2. **If you still can't proceed: stop and ask.** Tell the user explicitly that you need to take a shortcut, describe what's missing, and ask for permission. The user can then say whether it should be done differently or whether they will request a new feature/component from the design system. Do not take the shortcut without approval.

### Reporting additional CSS

If you needed **any additional CSS styling** on top of the design-system components (overrides, custom spacing, layout hacks, etc.), you **must report this explicitly** to the user — list exactly what custom CSS you added and why. Custom styling on top of the design system is a signal that may need a design-system change, so it must never be hidden.

## RFC Process

This project uses an RFC process for design decisions.

- **Location**: `docs/src/content/rfcs/`
- **Process document**: See `docs/src/content/rfcs/rfc-000.md`
- **Template**: Use `docs/src/content/rfcs/template.md`

### When to Write an RFC

Write an RFC for:
- Law representation format changes
- Execution engine architecture changes
- Cross-cutting design patterns
- Integration patterns between components

### RFC Metadata (frontmatter)

RFC metadata lives in YAML **frontmatter**, not a bold-labelled body preamble.
The fields are `title`, `status`, `implementation`, `date`, `authors`,
optional `depends_on`, and optional `short_title`. The docs site
(`docs/src/pages/rfcs/`, parsed by `docs/src/lib/rfcs.ts`) renders `status` and
`implementation` as NDD tags and the rest as a header line — there is no rehype
preamble plugin.

Two orthogonal fields, both required on every RFC so an absent tag never reads
as "unknown":

- **`status`** — lifecycle only: `Draft | Proposed | Accepted | Rejected | Superseded`.
  A built-and-merged RFC is `Accepted`, not `Draft`; "Draft" means the design
  itself is unsettled.
- **`implementation`** — build state: `Implemented | Partially implemented | Not implemented`.
  Independent of `status` (code can land ahead of acceptance). Ground the value
  in the actual codebase, not the RFC's aspirations.

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
| lawmaking | `regelrecht-lawmaking` | `lawmaking.regelrecht.rijks.app` |
| docs | `regelrecht-docs` | `docs.regelrecht.rijks.app` + `regelrecht.rijks.app` (landing) |
| grafana | `regelrecht-grafana` | `grafana.regelrecht.rijks.app` |

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
zad component add docs \
    --image ghcr.io/minbzk/regelrecht-docs:latest \
    --deployment regelrecht \
    --port 8000 \
    --service publish-on-web

# Get logs
zad logs --deployment regelrecht --lines 50

# List deployments
zad deployment list
```

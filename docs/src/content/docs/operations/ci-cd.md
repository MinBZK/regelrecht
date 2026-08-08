---
title: "CI/CD Pipeline"
description: "What runs on every push and pull request, and how only the checks relevant to changed files run."
---

Continuous integration runs on every push to `main` and every pull request via `.github/workflows/ci.yml`. Only checks relevant to changed files run, so CI stays fast.

## What CI checks

### Code quality (on Rust/YAML changes)

- **Formatting** - `just format` (rustfmt check)
- **Linting** - `just lint` (clippy)
- **YAML validation** - yamllint + schema validation on corpus files
- **Pre-commit hooks** - trailing whitespace, end-of-file, merge conflicts

### Tests (on Rust changes)

CI runs `just test`, which is `cargo test --workspace` over every crate. A new
crate is covered without anyone having to add it to a list; the pipeline and
editor-api suites use testcontainers for PostgreSQL, so the runner needs Docker.

`just test-no-docker` is the same coverage minus those container-backed suites,
for a machine without Docker. It is what `just check` runs.

The BDD suite (`just bdd`, cucumber-rs with Gherkin scenarios) covers two
buckets and is **not** part of `just test`; the target carries `test = false` so
it only runs when called by name. `BDD_BUCKET` picks the bucket: `all` (the
default, what `just bdd` runs), `corpus` or `conformance`.

### BDD conformance (on relevant changes)

The **BDD conformance** job runs bucket B — `bdd/conformance/*.feature` against
the synthetic `test_*` laws — as `BDD_BUCKET=conformance cargo test --test bdd`,
and hangs on the `Test` gate, so it blocks a merge. That bucket proves the engine
speaks the whole feature language and depends on nothing outside the repo.

Bucket A (`corpus/regulation/**/scenarios/*.feature`) stays out of CI. It asserts
what the live laws currently produce, so a failure there means a law changed or a
scenario went stale; a human decides what that is worth. Run it locally with
`BDD_BUCKET=corpus`.

### WASM build (on engine changes)

Builds the engine for the WebAssembly target to catch compilation issues early.

### Security audit (always runs)

- **Rust** - `cargo-deny` checks for known vulnerabilities and license issues
- **Frontend** - `npm ci` for the editor and admin dashboard

### Schema protection (on PRs)

Released schema versions in `schema/v*.*.*` are immutable. CI fails if a PR tries to modify or delete a released schema. Only `schema/latest/` can be updated freely.

### Provenance checks (on corpus/engine changes)

The `provenance-checks` job verifies that every corpus YAML file uses a tag-based `$schema` URL (`refs/tags/schema-vX.Y.Z`) and that the referenced schema version is known. This catches files that still use the old `refs/heads/main` format. See [RFC-013](/rfcs/rfc-013) for context.

### Component-specific checks

- **Admin** - format, lint, cargo check, tests, frontend build
- **Editor API** - format, lint, cargo check

## Change detection

CI uses path filters to determine which checks to run:

| Change group | Triggers on changes to |
|---|---|
| `ci` | `packages/corpus/`, `packages/engine/`, `packages/harvester/`, `packages/pipeline/`, `frontend/`, `corpus/regulation/`, `features/`, `schema/`, `script/` |
| `admin` | `packages/admin/` |
| `editor-api` | `packages/editor-api/`, `packages/corpus/`, `packages/pipeline/`, `packages/harvester/` |
| `docs` | `docs/` |

The `ci` group includes `frontend/`, so frontend changes also trigger the Rust checks (the editor is shipped as one image built from `frontend/` plus the `editor-api` Rust binary that serves it). Docs-only changes skip the Rust checks and run just the docs accessibility gate (`just docs-a11y`).

## Further reading

- [Deployment](./deployment) - what happens after CI passes
- [Testing](/guide/testing) - how to run tests locally

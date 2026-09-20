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
for a machine without Docker. `just check` runs the full `test`.

The BDD suite (`just bdd`, cucumber-rs with Gherkin scenarios) covers two
buckets and is **not** part of `just test`; the target carries `test = false` so
it only runs when called by name. `BDD_BUCKET` picks the bucket: `all` (the
default, what `just bdd` runs), `corpus` or `conformance`.

### BDD conformance (on relevant changes)

The **BDD conformance** job runs bucket B (`bdd/conformance/*.feature` against
the synthetic `test_*` laws) as `BDD_BUCKET=conformance cargo test --test bdd`.
It hangs on the `Test` gate, so it blocks a merge. That bucket proves the engine
speaks the whole feature language and depends on nothing outside the repo.

Bucket A (`corpus/regulation/**/scenarios/*.feature`) stays out of CI. It asserts
what the live laws currently produce, so a failure there means a law changed or a
scenario went stale; a human decides what that is worth. Run it locally with
`BDD_BUCKET=corpus`.

### WASM build (on engine changes)

Builds the engine for the WebAssembly target to catch compilation issues early.

### Licence and supply-chain audit (always runs)

- **Rust** - `cargo-deny bans licenses sources`: banned crates, licence compliance, and an allowlist of dependency sources
- **Frontend** - `license-checker` over the npm workspace at the repo root, refusing the copyleft licences the project cannot ship

Neither scans for known vulnerabilities. That is `just audit-advisories` (RustSec plus npm advisories), which runs periodically from `security-advisories.yml` rather than on every push.

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
| `ci` | `packages/`, `frontend/`, `corpus/regulation/`, `corpus/demo/`, `bdd/`, `schema/`, `script/`, the PoC trees, and the root build files (`Justfile`, `package.json`, `rust-toolchain.toml`) |
| `admin` | `packages/admin/` |
| `editor-api` | `packages/editor-api/`, `packages/corpus/`, `packages/pipeline/`, `packages/harvester/` |
| `docs` | `docs/` |

The `ci` group includes `frontend/`, so frontend changes also trigger the Rust checks (the editor is shipped as one image built from `frontend/` plus the `editor-api` Rust binary that serves it). Docs-only changes skip the Rust checks and run just the docs accessibility gate (`just docs-a11y`).

## Merge gates

Passing the checks above is not enough on its own. Four gates decide whether a pull request can merge, and three of them block.

| Gate | What it requires |
|------|------------------|
| **Werkpakket genoemd** | The PR body ends with a `Werkpakket:` line naming a werkpakket from the roadmap. `geen` is allowed with a reason. See [Contributing](./contributing). |
| **Claude review completed** | The automated review ran to completion for this commit and left no finding marked Critical. There is no override: fix the finding and push again. |
| **Security update approved** | A Dependabot security update needs an approving review from someone with write access, on the commit being merged. Security updates skip the five-day cooldown, so a human looks at them instead. |
| **Mutation Testing (diff)** | Reports on whether the tests pin the behavior the change introduces. |

A green review job is not the same as a review that happened, and the gate checks both: that the review workflow matches the copy on the default branch, and that the step recording a completed run did not skip. Fork PRs and Dependabot are handled separately rather than waved through.

## Further reading

- [Contributing](./contributing) - the PR process, including the required trailer
- [Deployment](./deployment) - what happens after CI passes
- [Testing](/guide/testing) - how to run tests locally

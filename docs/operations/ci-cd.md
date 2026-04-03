# CI/CD Pipeline

Continuous integration runs on every push to `main` and every pull request via `.github/workflows/ci.yml`. Only checks relevant to changed files run, keeping CI fast.

## What CI checks

### Code quality (on Rust/YAML changes)

- **Formatting** - `just format` (rustfmt check)
- **Linting** - `just lint` (clippy)
- **YAML validation** - yamllint + schema validation on corpus files
- **Pre-commit hooks** - trailing whitespace, end-of-file, merge conflicts

### Tests (on Rust changes)

- **Unit tests** - `just test`
- **BDD tests** - `just bdd` (cucumber-rs with Gherkin scenarios)
- **Harvester tests** - `just harvester-test`
- **Pipeline integration tests** - `just pipeline-integration-test` (uses testcontainers for PostgreSQL)

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
| `ci` | `packages/corpus/`, `packages/engine/`, `packages/harvester/`, `packages/pipeline/`, `corpus/regulation/`, `features/`, `schema/` |
| `admin` | `packages/admin/` |
| `editor-api` | `packages/editor-api/` |

Unrelated changes (docs-only, frontend-only) skip Rust checks entirely.

## Further reading

- [Deployment](./deployment) - what happens after CI passes
- [Testing](/guide/testing) - how to run tests locally

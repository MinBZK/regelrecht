# Justfile voor regelrecht-mvp
# Gebruik: just <task>

# Default task - toon beschikbare tasks
default:
    @just --list

# --- Quality checks ---

# Check Rust formatting
format:
    cd packages && cargo fmt --check --all

# Run clippy lints
lint:
    cd packages && cargo clippy --all-features -- -D warnings

# Run cargo check
build-check:
    cd packages && cargo check --all-features

# Validate regulation YAML files
validate *FILES:
    script/validate.sh {{FILES}}

# Run all quality checks (format + lint + check + validate + tests)
# Note: pipeline-integration-test excluded — it requires Docker (testcontainers)
check: format lint build-check validate test harvester-test pipeline-test admin-fmt admin-lint admin-check admin-test admin-frontend

# --- Tests ---

# Run Rust unit and integration tests
test:
    cd packages/engine && cargo test --all-features

# Run Rust BDD tests
bdd:
    cd packages/engine && cargo test --test bdd -- --nocapture

# Run harvester tests
harvester-test:
    cd packages/harvester && cargo test

# Run pipeline unit tests (no Docker/DB required)
pipeline-test:
    cd packages/pipeline && cargo test --lib

# Run pipeline integration tests (requires Docker for testcontainers)
pipeline-integration-test:
    cd packages/pipeline && cargo test --test '*'

# Run all tests (engine + harvester + pipeline unit + pipeline integration)
test-all: test harvester-test pipeline-test pipeline-integration-test

# --- Mutation testing ---

# Run mutation testing on engine (in-place because tests use relative paths to regulation/)
mutants *ARGS:
    cd packages/engine && cargo mutants --in-place --timeout-multiplier 3 {{ARGS}}

# --- Security ---

# Run security audit on all dependencies (vulnerabilities, licenses, sources)
audit:
    cargo deny check
    cd frontend && npm audit
    cd frontend && npx license-checker --production --failOn "GPL-2.0;GPL-3.0;AGPL-1.0;AGPL-3.0;SSPL-1.0;BUSL-1.1"
    cd packages/admin/frontend-src && npm audit

# --- Admin ---

# Run admin API locally (requires DATABASE_SERVER_FULL env var)
admin:
    cd packages && cargo run --package regelrecht-admin

# Build admin frontend (requires GITHUB_TOKEN env var for npm)
admin-frontend:
    cd packages/admin/frontend-src && npm ci && npm run build

# Check admin Rust code
admin-check:
    cd packages && cargo check --package regelrecht-admin

# Lint admin Rust code
admin-lint:
    cd packages && cargo clippy --package regelrecht-admin -- -D warnings

# Format check admin Rust code
admin-fmt:
    cd packages && cargo fmt --check --package regelrecht-admin

# Run admin tests
admin-test:
    cd packages && cargo test --package regelrecht-admin

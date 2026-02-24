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
check: format lint build-check validate test-all admin-fmt admin-lint admin-check admin-test admin-frontend

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

# Run all tests (engine + harvester + pipeline)
test-all: test harvester-test pipeline-test

# --- Mutation testing ---

# Run mutation testing on engine (in-place because tests use relative paths to regulation/)
mutants *ARGS:
    cd packages/engine && cargo mutants --in-place --timeout-multiplier 3 {{ARGS}}

# --- Security ---

# Run security audit on all dependencies (vulnerabilities, licenses, sources)
audit:
    cargo deny check
    cargo deny --manifest-path packages/admin/Cargo.toml check
    cd frontend && npm audit
    cd frontend && npx license-checker --production --failOn "GPL-2.0;GPL-3.0;AGPL-1.0;AGPL-3.0;SSPL-1.0;BUSL-1.1"
    cd packages/admin/frontend-src && npm audit

# --- Admin ---

# Run admin API locally (requires DATABASE_SERVER_FULL env var)
admin:
    cargo run --manifest-path packages/admin/Cargo.toml

# Build admin frontend (requires GITHUB_TOKEN env var for npm)
admin-frontend:
    cd packages/admin/frontend-src && npm ci && npm run build

# Check admin Rust code
admin-check:
    cargo check --manifest-path packages/admin/Cargo.toml

# Lint admin Rust code
admin-lint:
    cargo clippy --manifest-path packages/admin/Cargo.toml -- -D warnings

# Format check admin Rust code
admin-fmt:
    cd packages/admin && cargo fmt --check

# Run admin tests
admin-test:
    cargo test --manifest-path packages/admin/Cargo.toml

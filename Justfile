# Justfile voor regelrecht
# Gebruik: just <task>

set dotenv-load := true

# CI uses RUSTFLAGS=-Dwarnings; ci_flags mirrors that for quality/test recipes
# but not for dev (hot-reload), where in-flight warnings would kill cargo watch.
ci_flags := "RUSTFLAGS=-Dwarnings"

# Default task - toon beschikbare tasks
default:
    @just --list

# --- Quality checks ---

# Check Rust formatting
format:
    cd packages && cargo fmt --check --all

# Run clippy lints
lint:
    cd packages && {{ci_flags}} cargo clippy --all-features

# Run cargo check
build-check:
    cd packages && {{ci_flags}} cargo check --all-features

# Validate regulation YAML files
validate *FILES:
    script/validate.sh {{FILES}}

# Run all quality checks (format + lint + check + validate + tests)
# Note: pipeline-integration-test excluded — it requires Docker (testcontainers)
check: format lint build-check validate test harvester-test pipeline-test admin-fmt admin-lint admin-check admin-test admin-frontend editor-api-fmt editor-api-lint editor-api-check

# --- Tests ---

# Run Rust unit and integration tests
test:
    cd packages/engine && {{ci_flags}} cargo test --all-features

# Run Rust BDD tests
bdd:
    cd packages/engine && {{ci_flags}} cargo test --test bdd -- --nocapture

# Run harvester tests
harvester-test:
    cd packages/harvester && {{ci_flags}} cargo test

# Run pipeline unit tests (no Docker/DB required)
pipeline-test:
    cd packages/pipeline && {{ci_flags}} cargo test --lib

# Run pipeline integration tests (requires Docker for testcontainers)
pipeline-integration-test:
    cd packages/pipeline && {{ci_flags}} cargo test --test '*'

# Run all tests (engine + harvester + pipeline unit + pipeline integration)
test-all: test harvester-test pipeline-test pipeline-integration-test

# --- Mutation testing ---

# Run mutation testing on engine (in-place because tests use relative paths to corpus/)
mutants *ARGS:
    cd packages/engine && cargo mutants --in-place --timeout-multiplier 3 {{ARGS}}

# --- Benchmarks ---

_bench_flags := "--bench uri_parsing --bench variable_resolution --bench operations --bench article_evaluation --bench law_loading --bench priority --bench service_e2e"

# Run criterion benchmarks (skips unit test harness, runs only criterion benches)
bench *ARGS:
    cd packages/engine && cargo bench {{_bench_flags}} {{ARGS}}

# Run benchmarks and save baseline
bench-save NAME:
    cd packages/engine && cargo bench {{_bench_flags}} -- --save-baseline {{NAME}}

# Compare against saved baseline (run `just bench-save <name>` first to create one)
bench-compare BASE:
    cd packages/engine && cargo bench {{_bench_flags}} -- --baseline {{BASE}}

# --- Security ---

# Run security audit on all dependencies (vulnerabilities, licenses, sources)
audit:
    cd packages && cargo deny check --config ../deny.toml
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
    cd packages && {{ci_flags}} cargo check --package regelrecht-admin

# Lint admin Rust code
admin-lint:
    cd packages && {{ci_flags}} cargo clippy --package regelrecht-admin

# Format check admin Rust code
admin-fmt:
    cd packages && cargo fmt --check --package regelrecht-admin

# Run admin tests
admin-test:
    cd packages && {{ci_flags}} cargo test --package regelrecht-admin

# --- Editor API ---

# Run editor API locally
editor-api:
    cd packages && cargo run --package regelrecht-editor-api

# Check editor API Rust code
editor-api-check:
    cd packages && {{ci_flags}} cargo check --package regelrecht-editor-api

# Lint editor API Rust code
editor-api-lint:
    cd packages && {{ci_flags}} cargo clippy --package regelrecht-editor-api

# Format check editor API Rust code
editor-api-fmt:
    cd packages && cargo fmt --check --package regelrecht-editor-api

# --- Development (native with hot reload) ---

compose := "docker compose -f docker-compose.dev.yml"
compose-local := compose + " -f dev/compose.local.yaml"
compose-native := compose + " -f dev/compose.native.yaml"

# Start development: infra in Docker, services native with hot reload
dev:
    node dev/dev.mjs

# Stop dev: kill native processes and stop infra
dev-down:
    node dev/dev-down.mjs

# Follow infra logs in dev mode
dev-logs *ARGS:
    {{ compose-native }} logs -f {{ARGS}}

# Connect to the dev database via psql
dev-psql:
    {{ compose-native }} exec postgres psql -U regelrecht regelrecht_pipeline

# --- Full local stack (everything in Docker) ---

# Run the complete stack in Docker (no hot reload)
local:
    #!/usr/bin/env bash
    set -euo pipefail
    logfile=$(mktemp)
    printf "\033[1m=> Building and starting full stack…\033[0m "
    if {{ compose-local }} up --build -d > "$logfile" 2>&1; then
        printf "\033[32mdone\033[0m\n\n"
        echo "  Editor:     http://localhost:${FRONTEND_PORT:-3000}"
        echo "  Admin:      http://localhost:${ADMIN_PORT:-8000}"
        echo "  Grafana:    http://localhost:${GRAFANA_PORT:-3001}"
        echo "  Prometheus: http://localhost:${PROMETHEUS_PORT:-9090}"
        echo "  PostgreSQL: internal (use 'just local-psql' to connect)"
        echo ""
        printf "  \033[2mLogs:\033[0m just local-logs\n"
        printf "  \033[2mStop:\033[0m just local-down\n"
    else
        printf "\033[31mfailed\033[0m\n\n"
        cat "$logfile"
        rm -f "$logfile"
        exit 1
    fi
    rm -f "$logfile"

# Stop the full local stack
local-down:
    {{ compose-local }} down

# Follow logs from local services (optionally filter: just local-logs admin)
local-logs *ARGS:
    {{ compose-local }} logs -f {{ARGS}}

# Rebuild and restart a specific local service (e.g., just local-restart admin)
local-restart SERVICE:
    {{ compose-local }} up --build -d {{SERVICE}}

# Show status of local services
local-ps:
    {{ compose-local }} ps

# Connect to the local database via psql
local-psql:
    {{ compose-local }} exec postgres psql -U regelrecht regelrecht_pipeline

# Remove all local data (volumes)
local-clean:
    {{ compose-local }} down -v

# --- Documentation ---

# Install docs dependencies (requires GITHUB_TOKEN for @minbzk/storybook)
# Token is only needed for install, not for dev/build/preview.
docs-install:
    #!/usr/bin/env bash
    set -euo pipefail
    # Try macOS keychain first, then fall back to environment variable
    TOKEN="${GITHUB_TOKEN:-$(security find-generic-password -a "$USER" -s github-packages-read -w 2>/dev/null || echo "")}"
    if [ -z "$TOKEN" ]; then
        printf "\033[31mNo GITHUB_TOKEN found.\033[0m\n"
        printf "Create a classic PAT at https://github.com/settings/tokens\n"
        printf "with only the read:packages scope. Then:\n\n"
        printf "  macOS:  security add-generic-password -a \"\$USER\" -s github-packages-read -w \"ghp_YOUR_TOKEN\"\n"
        printf "  Linux:  export GITHUB_TOKEN=ghp_YOUR_TOKEN\n"
        exit 1
    fi
    cd docs && GITHUB_TOKEN="$TOKEN" npm ci

# Start docs dev server (VitePress)
docs:
    cd docs && npm run dev

# Build docs for production
docs-build:
    cd docs && npm run build

# Preview production docs build
docs-preview:
    cd docs && npm run preview

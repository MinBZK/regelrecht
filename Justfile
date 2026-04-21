# Justfile voor regelrecht
# Gebruik: just <task>

set dotenv-load := true

# CI uses RUSTFLAGS=-Dwarnings; ci_flags mirrors that for quality/test recipes
# but not for dev (hot-reload), where in-flight warnings would kill cargo watch.
ci_flags := "RUSTFLAGS=-Dwarnings"

# Default task - toon beschikbare tasks
default:
    @just --list

# --- Cases (scope manifests) ---

# List available case manifests in cases/
case-list:
    node script/case-load.js list

# Load a case manifest: regenerate .scope/ and corpus-registry.local.yaml
case-load SLUG:
    node script/case-load.js load {{SLUG}}

# Clear any loaded case scope (removes .scope/ and corpus-registry.local.yaml)
case-clear:
    node script/case-load.js clear

# --- WASM ---

# Build WASM module for browser use
wasm-build:
    cargo build --manifest-path packages/engine/Cargo.toml --target wasm32-unknown-unknown --release --features wasm
    wasm-bindgen --target web --out-dir frontend/public/wasm/pkg packages/target/wasm32-unknown-unknown/release/regelrecht_engine.wasm

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

# Run BDD tests with execution trace output (writes box-drawing traces to trace_output/)
bdd-trace:
    rm -rf trace_output
    cd packages/engine && {{ci_flags}} TRACE=1 cargo test --test bdd -- --nocapture

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
pidfile := ".dev-pids"

# Start development: infra in Docker, services native with hot reload
dev:
    #!/usr/bin/env bash
    set -euo pipefail

    bold="\033[1m"  dim="\033[2m"  reset="\033[0m"
    green="\033[32m"  red="\033[31m"  cyan="\033[36m"  yellow="\033[33m"

    # -- preflight checks --
    missing=()
    command -v cargo >/dev/null || missing+=("cargo (rustup.rs)")
    command -v node >/dev/null  || missing+=("node")
    command -v docker >/dev/null || missing+=("docker")
    if ! cargo watch --version >/dev/null 2>&1; then
        printf "${yellow}=> Installing cargo-watch…${reset} "
        if cargo install cargo-watch --quiet 2>/dev/null; then
            printf "${green}done${reset}\n"
        else
            missing+=("cargo-watch (cargo install cargo-watch)")
        fi
    fi
    if [ ${#missing[@]} -gt 0 ]; then
        printf "${red}Missing dependencies:${reset}\n"
        for dep in "${missing[@]}"; do echo "  - $dep"; done
        exit 1
    fi

    # -- start infra --
    logfile=$(mktemp)
    printf "${bold}=> Starting infra (postgres, prometheus, grafana)…${reset} "
    if {{ compose-native }} up -d postgres prometheus grafana > "$logfile" 2>&1; then
        printf "${green}done${reset}\n"
    else
        printf "${red}failed${reset}\n"
        cat "$logfile"; rm -f "$logfile"; exit 1
    fi
    rm -f "$logfile"

    # -- wait for postgres --
    printf "${bold}=> Waiting for postgres…${reset} "
    for i in $(seq 1 30); do
        if {{ compose-native }} exec -T postgres pg_isready -U regelrecht -d regelrecht_pipeline >/dev/null 2>&1; then
            printf "${green}ready${reset}\n"
            break
        fi
        if [ "$i" -eq 30 ]; then
            printf "${red}timeout${reset}\n"; exit 1
        fi
        sleep 1
    done

    # -- install frontend deps if needed --
    admin_fe=true
    editor_fe=true

    if [ ! -d packages/admin/frontend-src/node_modules ]; then
        printf "${bold}=> Installing admin frontend deps…${reset} "
        if (cd packages/admin/frontend-src && npm ci --silent) > /dev/null 2>&1; then
            printf "${green}done${reset}\n"
        else
            printf "${yellow}skipped${reset} (set GITHUB_TOKEN in .env for private packages)\n"
            admin_fe=false
        fi
    fi
    if [ ! -d frontend/node_modules ]; then
        printf "${bold}=> Installing editor frontend deps…${reset} "
        if (cd frontend && npm ci --silent) > /dev/null 2>&1; then
            printf "${green}done${reset}\n"
        else
            printf "${yellow}skipped${reset} (set GITHUB_TOKEN in .env for private packages)\n"
            editor_fe=false
        fi
    fi

    # -- start native services in background --
    rm -f {{ pidfile }}

    printf "${bold}=> Starting admin API (cargo watch on :8000)…${reset}\n"
    DATABASE_URL="postgres://regelrecht:regelrecht_dev@localhost:${POSTGRES_PORT:-5433}/regelrecht_pipeline" \
    RUST_LOG="${RUST_LOG:-info}" \
      cargo watch -C packages -x 'run --package regelrecht-admin' \
      > .dev-admin.log 2>&1 &
    echo "$!" >> {{ pidfile }}

    if [ "$admin_fe" = true ]; then
        printf "${bold}=> Starting admin frontend (vite on :3001)…${reset}\n"
        (cd packages/admin/frontend-src && npx vite) > .dev-admin-frontend.log 2>&1 &
        echo "$!" >> {{ pidfile }}
    fi

    if [ "$editor_fe" = true ]; then
        printf "${bold}=> Starting editor frontend (vite on :3000)…${reset}\n"
        (cd frontend && npx vite) > .dev-editor.log 2>&1 &
        echo "$!" >> {{ pidfile }}
    fi

    # -- wait for services to come up --
    printf "${bold}=> Waiting for services…${reset} "
    sleep 4
    printf "${green}done${reset}\n"

    printf "\n"
    printf "${bold}${green}  Dev stack is running with hot reload${reset}\n\n"
    if [ "$editor_fe" = true ]; then
        echo "  Editor:     http://localhost:3000     (hot reload)"
    fi
    if [ "$admin_fe" = true ]; then
        echo "  Admin UI:   http://localhost:3001     (hot reload, proxies API to :8000)"
    fi
    echo   "  Admin API:  http://localhost:8000     (auto-recompile on save)"
    echo   "  Grafana:    http://localhost:${GRAFANA_PORT:-3002}"
    echo   "  Prometheus: http://localhost:${PROMETHEUS_PORT:-9090}"
    echo   "  PostgreSQL: localhost:${POSTGRES_PORT:-5433}"
    printf "\n"
    printf "  ${dim}Admin API log:${reset}      tail -f .dev-admin.log\n"
    if [ "$admin_fe" = true ]; then
        printf "  ${dim}Admin frontend log:${reset} tail -f .dev-admin-frontend.log\n"
    fi
    if [ "$editor_fe" = true ]; then
        printf "  ${dim}Editor log:${reset}         tail -f .dev-editor.log\n"
    fi
    printf "  ${dim}Infra logs:${reset}         just dev-logs\n"
    printf "  ${dim}Database:${reset}           just dev-psql\n"
    printf "  ${dim}Stop everything:${reset}    just dev-down\n"

# Stop dev: kill native processes and stop infra
dev-down:
    #!/usr/bin/env bash
    set -euo pipefail
    bold="\033[1m"  reset="\033[0m"  green="\033[32m"

    printf "${bold}=> Stopping native services…${reset} "
    if [ -f {{ pidfile }} ]; then
        while read -r pid; do
            # Kill children first (node, cargo, etc.), then the parent
            pkill -P "$pid" 2>/dev/null || true
            kill "$pid" 2>/dev/null || true
        done < {{ pidfile }}
        rm -f {{ pidfile }}
        sleep 1
    fi
    rm -f .dev-admin.log .dev-admin-frontend.log .dev-editor.log
    printf "${green}done${reset}\n"

    printf "${bold}=> Stopping infra…${reset} "
    {{ compose-native }} down > /dev/null 2>&1
    printf "${green}done${reset}\n"

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

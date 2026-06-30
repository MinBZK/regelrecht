# Justfile voor regelrecht
# Gebruik: just <task>

set dotenv-load := true

# CI uses RUSTFLAGS=-Dwarnings; ci_flags mirrors that for quality/test recipes
# but not for dev (hot-reload), where in-flight warnings would kill cargo watch.
# We also pass the mold link-arg here: an explicit RUSTFLAGS overrides the
# target.rustflags in packages/.cargo/config.toml, so without it these recipes
# would fall back to the slow default linker. (dev has no RUSTFLAGS, so it picks
# up mold straight from .cargo/config.toml.)
#
# The mold link-arg is Linux-only, mirroring the [target.x86_64-unknown-linux-gnu]
# scoping in packages/.cargo/config.toml — otherwise quality/test recipes would
# force `-fuse-ld=mold` on macOS where mold typically isn't installed.
ci_flags := if os() == "linux" {
    "RUSTFLAGS='-Dwarnings -C link-arg=-fuse-ld=mold'"
} else {
    "RUSTFLAGS=-Dwarnings"
}

# Default task - toon beschikbare tasks
default:
    @just --list

# --- WASM ---

# Build WASM module for browser use
wasm-build:
    # Pin the target dir explicitly. A CLI --target-dir overrides any shared
    # [build] target-dir from `just dev-setup` (root .cargo/config.toml), so the
    # artifact always lands at packages/target — no metadata lookup (and no jq/
    # python3 dependency) needed, and it works with or without dev-setup.
    cargo build --manifest-path packages/engine/Cargo.toml --target wasm32-unknown-unknown --release --features wasm --target-dir packages/target
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

# Validate note sidecar files (RFC-005, RFC-016)
# Orphaned/ambiguous notes and unknown tags are warnings, not errors.
validate-annotations *FILES:
    script/validate-annotations.sh {{FILES}}

# Schema↔model conformance suite — proves the Rust law-model conforms to the
# canonical schema.json. Standalone helper; `just test` already runs it via
# --all-features. Needs the `validate` feature (jsonschema). See
# packages/engine/tests/conformance/README.md.
conformance:
    cd packages && {{ci_flags}} cargo test -p regelrecht-engine --features validate --test conformance

# Run all quality checks (format + lint + check + validate + validate-annotations + tests)
# Note: pipeline-integration-test excluded — it requires Docker (testcontainers)
# Note: the conformance suite runs as part of `test` (cargo test --all-features).
check: format lint build-check validate validate-annotations test harvester-test pipeline-test admin-fmt admin-lint admin-check admin-test admin-frontend editor-api-fmt editor-api-lint editor-api-check

# --- Tests ---

# Run Rust unit and integration tests
test:
    cd packages/engine && {{ci_flags}} cargo test --all-features

# Run Rust BDD tests
bdd:
    cd packages/engine && {{ci_flags}} cargo test --test bdd -- --nocapture

# Regenerate all BDD step bindings from bdd/grammar.yaml
bdd-codegen:
    node bdd/codegen/gen-js.mjs
    @echo "Rust bindings regenerate automatically via build.rs on next cargo build"

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
    # One npm workspace at the repo root covers all three frontends + the shared
    # package, so audit/license-check the whole hoisted tree once. (license-checker
    # drops --production because the workspace root has no production deps of its
    # own; the whole-tree scan is strictly broader coverage.)
    npm audit
    npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-1.0;AGPL-3.0;SSPL-1.0;BUSL-1.1"

# --- Admin ---

# Run admin API locally (requires DATABASE_SERVER_FULL env var)
admin:
    cd packages && cargo run --package regelrecht-admin

# Build admin frontend (npm workspace: install at root, build the admin workspace)
admin-frontend:
    npm ci && npm run build -w packages/admin/frontend-src

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

# Run editor API with real SSO Rijk login (loads .env.sso-local; see auth-and-roles.md)
editor-sso:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -f .env.sso-local ]; then
        echo "Missing .env.sso-local — copy .env.sso-local.example and fill in the Keycloak values." >&2
        exit 1
    fi
    set -a; . ./.env.sso-local; set +a
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

# Run editor API integration tests (requires Docker for testcontainers).
# Excluded from `just check` for the same reason `pipeline-integration-test` is.
editor-api-integration-test:
    cd packages && {{ci_flags}} cargo test --package regelrecht-editor-api --test '*'

# --- Development (native with hot reload) ---

compose := "docker compose -f docker-compose.dev.yml"
compose-local := compose + " -f dev/compose.local.yaml"
compose-native := compose + " -f dev/compose.native.yaml"
pidfile := ".dev-pids"

# One-time build-speed setup: install mold + sccache, share one target dir across worktrees
dev-setup:
    #!/usr/bin/env bash
    set -euo pipefail
    bold="\033[1m"  dim="\033[2m"  reset="\033[0m"
    green="\033[32m"  yellow="\033[33m"  red="\033[31m"

    sudo_if_needed() { if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi; }

    install_one() {
        local bin="$1"; shift
        command -v "$bin" >/dev/null 2>&1 && { printf "${green}%s already installed${reset}\n" "$bin"; return 0; }
        printf "${bold}=> Installing %s…${reset} " "$bin"
        if "$@" >/dev/null 2>&1; then printf "${green}done${reset}\n"; return 0; else printf "${red}failed${reset} (install %s manually)\n" "$bin"; return 1; fi
    }

    # Track whether mold ended up available — the shared-target setup is harmless
    # without it, but mold linking (the headline feature) needs it on PATH.
    mold_ok=false
    if command -v apt-get >/dev/null 2>&1; then
        sudo_if_needed apt-get update -qq || true
        if install_one mold sudo_if_needed apt-get install -y mold; then mold_ok=true; fi
        install_one sccache sudo_if_needed apt-get install -y sccache || true
    elif command -v dnf >/dev/null 2>&1; then
        if install_one mold sudo_if_needed dnf install -y mold; then mold_ok=true; fi
        install_one sccache sudo_if_needed dnf install -y sccache || true
    elif command -v brew >/dev/null 2>&1; then
        if install_one mold brew install mold; then mold_ok=true; fi
        install_one sccache brew install sccache || true
    else
        printf "${yellow}No supported package manager found.${reset}\n"
        printf "  Install mold:    https://github.com/rui314/mold\n"
        printf "  Install sccache: cargo install sccache --locked\n"
    fi
    # `install_one` returns 0 when the binary is already present, so a pre-existing
    # mold also counts as OK regardless of which package-manager branch ran.
    command -v mold >/dev/null 2>&1 && mold_ok=true
    # sccache may not be packaged everywhere — fall back to cargo install.
    command -v sccache >/dev/null 2>&1 || install_one sccache cargo install sccache --locked

    # Shared target dir across all worktrees. .worktrees/ lives inside the main
    # checkout, so cargo's upward config search finds this root .cargo/config.toml
    # from every worktree. It is gitignored (machine-specific absolute path), so
    # CI keeps its own packages/target.
    root="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")"

    # A Rust build writes tens of thousands of small files. On a slow/remote
    # mount (9p in WSL2/Docker-Desktop dev containers, NFS, SMB, …) that I/O
    # dominates build time — far more than the linker does. Detect that and put
    # the shared target on fast local storage instead of inside the repo.
    fstype="$(stat -f -c %T "$root" 2>/dev/null || echo unknown)"
    case "$fstype" in
        9p|v9fs|nfs|nfs4|cifs|smb*|smb2|fuseblk|fuse.*|vboxsf|virtiofs|prl_fs)
            cache="${XDG_CACHE_HOME:-$HOME/.cache}/regelrecht"
            shared="$cache/target-$(printf '%s' "$root" | cksum | cut -d' ' -f1)"
            printf "${yellow}=> Repo is on a slow filesystem (%s); using fast local target dir.${reset}\n" "$fstype"
            ;;
        *)
            shared="$root/.cargo/target"
            ;;
    esac
    mkdir -p "$root/.cargo" "$shared"
    {
        echo "# Generated by 'just dev-setup'. Gitignored — machine-local, do not commit."
        echo "[build]"
        echo "target-dir = \"$shared\""
    } > "$root/.cargo/config.toml"
    printf "${green}=> Shared target dir:${reset} %s\n" "$shared"

    printf "\n"
    if [ "$mold_ok" = true ]; then
        printf "${bold}${green}Done.${reset} Mold linking + shared target are active for all worktrees.\n"
    else
        printf "${bold}${yellow}Partly done.${reset} Shared target is set up, but ${red}mold is missing${reset} — dev builds will fail to link.\n"
        printf "${dim}Install mold manually (https://github.com/rui314/mold), then re-run 'just dev-setup'.${reset}\n"
    fi
    printf "${dim}Old per-worktree packages/target dirs can now be removed to reclaim disk.${reset}\n"
    printf "${dim}Optional: enable sccache locally (disables incremental, best for cold/flag-varying builds):${reset}\n"
    printf "  export RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0\n"

# Start development: infra in Docker, services native with hot reload
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    export COMPOSE="{{ compose-native }}"  PIDFILE="{{ pidfile }}"
    source script/dev-lib.sh

    dev_preflight --rust --node --watch

    dev_compose_up postgres prometheus grafana
    dev_wait_postgres

    if dev_ensure_deps packages/admin/frontend-src "admin frontend"; then admin_fe=true; else admin_fe=false; fi
    if dev_ensure_deps frontend "editor frontend"; then editor_fe=true; else editor_fe=false; fi

    rm -f "$PIDFILE"

    db_url="postgres://regelrecht:regelrecht_dev@${DB_HOST:-localhost}:${POSTGRES_PORT:-5433}/regelrecht_pipeline"
    dev_start "admin API (cargo watch on :8000)" .dev-admin.log \
      "DATABASE_URL='$db_url' RUST_LOG='${RUST_LOG:-info}' cargo watch -C packages -x 'run --package regelrecht-admin'"

    if [ "$admin_fe" = true ]; then
        dev_start "admin frontend (vite on :3001)" .dev-admin-frontend.log \
          "cd packages/admin/frontend-src && npx vite"
    fi
    if [ "$editor_fe" = true ]; then
        dev_start "editor frontend (vite on :3000)" .dev-editor.log \
          "cd frontend && npx vite"
    fi

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

# Vite ports default to 7300/7400/7500 (the redirect URIs already registered on
# the regelrecht-local Keycloak client); override via EDITOR_PORT / ADMIN_FE_PORT
# / LAWMAKING_PORT. No grafana / prometheus / workers are started.
#
# Frontend-focused dev: start only what a frontend needs (backend, DB, WASM, vite). No arg = all frontends; APP = editor | admin | lawmaking | all
dev-frontend APP="all":
    #!/usr/bin/env bash
    set -euo pipefail
    export COMPOSE="{{ compose-native }}"  PIDFILE="{{ pidfile }}"
    source script/dev-lib.sh

    app="{{ APP }}"
    case "$app" in
        editor|admin|lawmaking|all) ;;
        *) printf "${red}Unknown app '%s'${reset} — use: editor | admin | lawmaking | all\n" "$app"; exit 1 ;;
    esac

    run_editor=false; run_admin=false; run_lawmaking=false
    case "$app" in
        editor)    run_editor=true ;;
        admin)     run_admin=true ;;
        lawmaking) run_lawmaking=true ;;
        all)       run_editor=true; run_admin=true; run_lawmaking=true ;;
    esac

    # Vite (browser-facing) ports — overridable. Editor stays 7300 unless you
    # also update BASE_URL in .env.sso-local and the Keycloak redirect URI.
    editor_port="${EDITOR_PORT:-7300}"
    admin_fe_port="${ADMIN_FE_PORT:-7400}"
    lawmaking_port="${LAWMAKING_PORT:-7500}"
    # In 'all', editor-api keeps :8000 and admin moves to :8001 to avoid the clash.
    admin_api_port=8000
    if [ "$app" = all ]; then admin_api_port=8001; fi

    if [ "$run_editor" = true ] || [ "$run_admin" = true ]; then
        dev_preflight --rust --node
    else
        dev_preflight --node
    fi

    # editor / all run editor-api with OIDC against the central Keycloak.
    if [ "$run_editor" = true ] && [ ! -f .env.sso-local ]; then
        printf "${red}Missing .env.sso-local${reset} — copy .env.sso-local.example and fill in the Keycloak values (see auth-and-roles.md).\n"
        exit 1
    fi

    # DB is needed by editor (session store) and admin; lawmaking has no backend.
    if [ "$run_editor" = true ] || [ "$run_admin" = true ]; then
        dev_compose_up postgres
        dev_wait_postgres
    fi

    rm -f "$PIDFILE"

    if [ "$run_editor" = true ]; then
        dev_ensure_wasm
        if dev_ensure_deps frontend "editor frontend"; then
            # Plain `cargo run` (not cargo-watch): a frontend-dev loop doesn't
            # rebuild the backend, and cargo-watch's recursive watch hangs on a
            # 9p-mounted worktree. Matches the `editor-sso` recipe. Source
            # .env.sso-local inside the backgrounded shell so the OIDC /
            # DATABASE_URL / BASE_URL it sets scope to editor-api only.
            dev_start "editor-api (:8000, OIDC)" .dev-editor-api.log \
              "set -a; . ./.env.sso-local; set +a; cd packages && cargo run --package regelrecht-editor-api"
            dev_start "editor vite (:${editor_port})" .dev-editor.log \
              "cd frontend && API_PORT=8000 npx vite --port ${editor_port} --strictPort --host 0.0.0.0"
        fi
    fi

    if [ "$run_admin" = true ]; then
        if dev_ensure_deps packages/admin/frontend-src "admin frontend"; then
            db_url="postgres://regelrecht:regelrecht_dev@${DB_HOST:-localhost}:${POSTGRES_PORT:-5433}/regelrecht_pipeline"
            dev_start "admin API (:${admin_api_port})" .dev-admin.log \
              "cd packages && DATABASE_URL='$db_url' RUST_LOG='${RUST_LOG:-info}' ADMIN_PORT=${admin_api_port} cargo run --package regelrecht-admin"
            dev_start "admin vite (:${admin_fe_port})" .dev-admin-frontend.log \
              "cd packages/admin/frontend-src && API_PORT=${admin_api_port} npx vite --port ${admin_fe_port} --strictPort --host 0.0.0.0"
        fi
    fi

    if [ "$run_lawmaking" = true ]; then
        if dev_ensure_deps frontend-lawmaking "lawmaking frontend"; then
            dev_start "lawmaking vite (:${lawmaking_port})" .dev-lawmaking.log \
              "cd frontend-lawmaking && npx vite --port ${lawmaking_port} --strictPort --host 0.0.0.0"
        fi
    fi

    printf "${bold}=> Waiting for services…${reset} "
    sleep 4
    printf "${green}done${reset}\n\n"

    printf "${bold}${green}  Frontend dev stack (%s) is running (vite HMR; backends run-once)${reset}\n\n" "$app"
    if [ "$run_editor" = true ]; then
        echo "  Editor:     http://localhost:${editor_port}     (vite HMR → editor-api :8000, SSO)"
    fi
    if [ "$run_admin" = true ]; then
        echo "  Admin UI:   http://localhost:${admin_fe_port}     (vite HMR → admin API :${admin_api_port})"
    fi
    if [ "$run_lawmaking" = true ]; then
        echo "  Lawmaking:  http://localhost:${lawmaking_port}     (vite HMR, no backend)"
    fi
    if [ "$run_editor" = true ] || [ "$run_admin" = true ]; then
        echo "  PostgreSQL: localhost:${POSTGRES_PORT:-5433}"
    fi
    printf "\n"
    printf "  ${dim}Logs:${reset}            tail -f .dev-*.log\n"
    printf "  ${dim}Stop everything:${reset} just dev-down\n"

# Stop dev: kill native processes and stop infra (works for dev and dev-frontend)
dev-down:
    #!/usr/bin/env bash
    set -euo pipefail
    export COMPOSE="{{ compose-native }}"  PIDFILE="{{ pidfile }}"
    source script/dev-lib.sh
    dev_stop

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

# Install docs dependencies
docs-install:
    cd docs && npm ci

# Start docs dev server (Astro, with HMR) at http://localhost:4321
docs:
    cd docs && npm run dev

# Build docs for production
docs-build:
    cd docs && npm run build

# Preview production docs build (Pagefind search works here, not in dev)
docs-preview:
    cd docs && npm run preview

# Run the accessibility gate (build + mermaid-alt + heading-order + pa11y-ci htmlcs+axe, WCAG 2.1 AA)
docs-a11y:
    cd docs && npm run a11y

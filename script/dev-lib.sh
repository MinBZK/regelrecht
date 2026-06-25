#!/usr/bin/env bash
# Shared helpers for the `just dev` and `just dev-frontend` recipes.
#
# This file is *sourced* (not executed) by those recipes, so they can share one
# implementation of preflight checks, infra start-up, dependency installation,
# backgrounded service launching, and teardown. Before sourcing, the caller must
# export:
#   COMPOSE  - the `docker compose -f … -f …` invocation (i.e. compose-native)
#   PIDFILE  - path to the file tracking backgrounded native-service PIDs
# All functions assume the repo root as CWD (just runs recipe bodies from there).

# ANSI colors. Stored as literal escapes and expanded by printf's format string.
bold="\033[1m"  dim="\033[2m"  reset="\033[0m"
green="\033[32m"  red="\033[31m"  yellow="\033[33m"

# dev_preflight [--rust] [--node] [--watch]
# Verify the tools the recipe needs. Always checks docker. --node also checks
# node; --rust also checks cargo and requires mold (the linker configured in
# packages/.cargo/config.toml); --watch additionally auto-installs cargo-watch
# (only `just dev` hot-reloads the backend). Exits 1 listing every missing dep.
dev_preflight() {
    local want_rust=false want_node=false want_watch=false arg
    for arg in "$@"; do
        case "$arg" in
            --rust)  want_rust=true ;;
            --node)  want_node=true ;;
            --watch) want_watch=true ;;
        esac
    done

    local missing=()
    command -v docker >/dev/null || missing+=("docker")
    [ "$want_node" = true ] && { command -v node >/dev/null || missing+=("node"); }

    if [ "$want_rust" = true ]; then
        command -v cargo >/dev/null || missing+=("cargo (rustup.rs)")
        # mold is the linker in packages/.cargo/config.toml; without it dev
        # builds fail to link.
        command -v mold >/dev/null 2>&1 || missing+=("mold (run 'just dev-setup')")
    fi

    if [ "$want_watch" = true ] && ! cargo watch --version >/dev/null 2>&1; then
        printf "${yellow}=> Installing cargo-watch…${reset} "
        if cargo install cargo-watch --quiet 2>/dev/null; then
            printf "${green}done${reset}\n"
        else
            missing+=("cargo-watch (cargo install cargo-watch)")
        fi
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        printf "${red}Missing dependencies:${reset}\n"
        local dep
        for dep in "${missing[@]}"; do echo "  - $dep"; done
        exit 1
    fi
}

# dev_compose_up <service…> — start the given infra services, failing loudly
# (with captured logs) if compose errors.
dev_compose_up() {
    local logfile
    logfile=$(mktemp)
    printf "${bold}=> Starting infra (%s)…${reset} " "$*"
    if $COMPOSE up -d "$@" > "$logfile" 2>&1; then
        printf "${green}done${reset}\n"
    else
        printf "${red}failed${reset}\n"
        cat "$logfile"; rm -f "$logfile"; exit 1
    fi
    rm -f "$logfile"
}

# dev_wait_postgres — block (up to 30s) until postgres accepts connections.
dev_wait_postgres() {
    printf "${bold}=> Waiting for postgres…${reset} "
    local i
    for i in $(seq 1 30); do
        if $COMPOSE exec -T postgres pg_isready -U regelrecht -d regelrecht_pipeline >/dev/null 2>&1; then
            printf "${green}ready${reset}\n"
            return 0
        fi
        if [ "$i" -eq 30 ]; then
            printf "${red}timeout${reset}\n"; exit 1
        fi
        sleep 1
    done
}

# dev_ensure_deps <dir> <label> — ensure the npm workspace is installed. All
# frontends share one root node_modules (npm workspace), so this runs a single
# root `npm ci` the first time and is a no-op afterwards. <dir> is kept for the
# call-site label only. Returns non-zero (with a warning) on failure so the
# caller can skip that frontend instead of aborting the whole stack.
dev_ensure_deps() {
    local dir="$1" label="$2"
    # The shared package's workspace symlink is the "deps installed" sentinel.
    [ -e "node_modules/@regelrecht/frontend-shared" ] && return 0
    printf "${bold}=> Installing workspace deps (for %s)…${reset} " "$label"
    if npm ci --silent > /dev/null 2>&1; then
        printf "${green}done${reset}\n"
        return 0
    fi
    printf "${yellow}skipped${reset} (npm ci failed)\n"
    return 1
}

# dev_ensure_wasm — build the engine WASM the editor frontend loads at runtime,
# if it isn't built yet. (frontend/src/composables/useEngine.js fetches it.)
dev_ensure_wasm() {
    [ -f frontend/public/wasm/pkg/regelrecht_engine.js ] && return 0
    printf "${bold}=> Building engine WASM (just wasm-build)…${reset}\n"
    just wasm-build
}

# dev_start <label> <logfile> <cmd-string> — run <cmd-string> in the background
# via `setsid bash -c`, redirect its output to <logfile>, and record its PID in
# $PIDFILE for `just dev-down`. The command is a string so callers can inline
# per-service env vars and `cd` without leaking them into the recipe shell.
# `setsid` makes the process its own session/group leader (PID == PGID), so
# dev_stop can kill the whole subtree — npm → node, cargo → child — in one shot.
dev_start() {
    local label="$1" logfile="$2" cmd="$3"
    printf "${bold}=> Starting %s…${reset}\n" "$label"
    setsid bash -c "$cmd" > "$logfile" 2>&1 &
    echo "$!" >> "$PIDFILE"
}

# dev_stop — kill everything recorded in $PIDFILE, remove dev logs, stop infra.
# Shared by `just dev-down`; works for whichever of dev / dev-frontend ran.
dev_stop() {
    printf "${bold}=> Stopping native services…${reset} "
    if [ -f "$PIDFILE" ]; then
        local pid
        while read -r pid; do
            # Each recorded PID is a setsid session leader (PID == PGID), so the
            # negative-PID kill takes out the whole process tree (vite's node
            # grandchild, cargo's child, …). Fall back to a plain kill.
            kill -- -"$pid" 2>/dev/null || kill "$pid" 2>/dev/null || true
        done < "$PIDFILE"
        rm -f "$PIDFILE"
        sleep 1
    fi
    rm -f .dev-*.log
    printf "${green}done${reset}\n"

    printf "${bold}=> Stopping infra…${reset} "
    $COMPOSE down > /dev/null 2>&1
    printf "${green}done${reset}\n"
}

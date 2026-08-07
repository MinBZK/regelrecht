#!/usr/bin/env bash
# Tests voor script/target-dir.sh. Geen git en geen cargo nodig: de twee paden
# die het script normaal uit git haalt komen uit TARGET_DIR_ROOT/_COMMON.
set -uo pipefail

SCRIPT="$(cd "$(dirname "$0")" && pwd)/target-dir.sh"
failures=0

run() {
    local root="$1" common="$2" mode="$3"
    TARGET_DIR_ROOT="$root" TARGET_DIR_COMMON="$common" bash "$SCRIPT" "$mode" 2>&1
}

check() {
    local name="$1" cond="$2"
    if eval "$cond"; then
        echo "ok: $name"
    else
        echo "FAIL: $name"
        failures=$((failures + 1))
    fi
}

tmp="$(mktemp -d)"
main="$tmp/repo"
tree="$tmp/repo/.worktrees/feature"
mkdir -p "$main" "$tree"

# --- isolated schrijft een worktree-lokale config ---------------------------
run "$tree" "$main" isolated > /dev/null
check "isolated schrijft een config" "[ -f '$tree/.cargo/config.toml' ]"
check "isolated wijst naar de worktree zelf" \
    "grep -qF 'target-dir = \"$tree/packages/target\"' '$tree/.cargo/config.toml'"

# --- shared haalt hem weer weg ---------------------------------------------
run "$tree" "$main" shared > /dev/null
check "shared verwijdert de config" "[ ! -e '$tree/.cargo/config.toml' ]"

# --- shared laat een handgeschreven config staan ---------------------------
mkdir -p "$tree/.cargo"
echo '[build]' > "$tree/.cargo/config.toml"
out="$(run "$tree" "$main" shared)"
check "shared raakt een vreemde config niet aan" "[ -f '$tree/.cargo/config.toml' ]"
check "shared meldt waarom hij niets deed" "grep -q 'niet door deze schakelaar' <<< \"\$out\""
rm -rf "$tree/.cargo"

# --- isolated laat een handgeschreven config ook staan ---------------------
mkdir -p "$tree/.cargo"
echo '[build]' > "$tree/.cargo/config.toml"
out="$(run "$tree" "$main" isolated)"
check "isolated raakt een vreemde config niet aan" \
    "[ \"\$(cat '$tree/.cargo/config.toml')\" = '[build]' ]"
check "isolated meldt waarom hij niets deed" "grep -q 'overschrijf' <<< \"\$out\""
rm -rf "$tree/.cargo"

# --- de hoofdcheckout wordt geweigerd --------------------------------------
out="$(run "$main" "$main" isolated)"
check "hoofdcheckout wordt geweigerd" "grep -q 'hoofdcheckout' <<< \"\$out\""
check "hoofdcheckout krijgt geen config" "[ ! -e '$main/.cargo/config.toml' ]"

# --- een onbekende modus is een gebruiksfout -------------------------------
run "$tree" "$main" onzin > /dev/null 2>&1
code=$?
check "onbekende modus faalt" "[ $code -ne 0 ]"

rm -rf "$tmp"

if [ "$failures" -gt 0 ]; then
    echo "$failures test(s) gefaald"
    exit 1
fi
echo "alle tests geslaagd"

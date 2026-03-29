#!/usr/bin/env bash
# Batch validate corpus YAML files with parallel execution and summary reporting.
#
# Usage:
#   batch_validate.sh                          # validate all corpus files
#   batch_validate.sh corpus/regulation/nl/wet  # validate specific directory
#   batch_validate.sh --parallel 8             # use 8 parallel workers
#   batch_validate.sh --filter "*.yaml"        # custom glob filter
#   batch_validate.sh --json                   # output JSON report
#
# Prerequisites:
#   - Rust toolchain (for building the validate binary)
#   - just (for initial build)

set -euo pipefail

# Defaults
PARALLEL_JOBS=$(sysctl -n hw.logicalcpu 2>/dev/null || nproc 2>/dev/null || echo 4)
CORPUS_DIR=""
FILTER="*.yaml"
JSON_OUTPUT=false
VALIDATE_BIN=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --parallel|-p)
            PARALLEL_JOBS="$2"
            shift 2
            ;;
        --filter|-f)
            FILTER="$2"
            shift 2
            ;;
        --json|-j)
            JSON_OUTPUT=true
            shift
            ;;
        --help|-h)
            echo "Usage: batch_validate.sh [OPTIONS] [CORPUS_DIR]"
            echo ""
            echo "Options:"
            echo "  --parallel, -p N   Number of parallel workers (default: CPU count)"
            echo "  --filter, -f GLOB  File glob pattern (default: *.yaml)"
            echo "  --json, -j         Output JSON report"
            echo "  --help, -h         Show this help"
            exit 0
            ;;
        *)
            CORPUS_DIR="$1"
            shift
            ;;
    esac
done

# Find repo root
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)

# Default corpus directory
if [[ -z "$CORPUS_DIR" ]]; then
    CORPUS_DIR="$REPO_ROOT/corpus/regulation"
fi

# Build validate binary once (release mode for speed)
echo "Building validate binary..." >&2
cd "$REPO_ROOT/packages"
cargo build --bin validate --release --quiet 2>&1 >&2
VALIDATE_BIN="$REPO_ROOT/packages/target/release/validate"
cd "$REPO_ROOT"

if [[ ! -x "$VALIDATE_BIN" ]]; then
    echo "ERROR: validate binary not found at $VALIDATE_BIN" >&2
    exit 1
fi

# Find all YAML files
mapfile -t FILES < <(find "$CORPUS_DIR" -name "$FILTER" -type f | sort)
TOTAL=${#FILES[@]}

if [[ $TOTAL -eq 0 ]]; then
    echo "No files found matching $FILTER in $CORPUS_DIR" >&2
    exit 0
fi

echo "Validating $TOTAL files with $PARALLEL_JOBS parallel workers..." >&2

# Validate files in parallel, capture results
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

validate_file() {
    local file="$1"
    local result_file="$TMPDIR/$(echo "$file" | tr '/' '_')"
    if output=$("$VALIDATE_BIN" "$file" 2>&1); then
        echo "PASS|$file|$output" > "$result_file"
    else
        echo "FAIL|$file|$output" > "$result_file"
    fi
}
export -f validate_file
export VALIDATE_BIN TMPDIR

printf '%s\n' "${FILES[@]}" | xargs -P "$PARALLEL_JOBS" -I{} bash -c 'validate_file "$@"' _ {}

# Aggregate results
PASS=0
FAIL=0
ERRORS=()

for result_file in "$TMPDIR"/*; do
    [[ -f "$result_file" ]] || continue
    IFS='|' read -r status file output < "$result_file"
    if [[ "$status" == "PASS" ]]; then
        ((PASS++))
    else
        ((FAIL++))
        ERRORS+=("$file: $output")
    fi
done

# Output
if $JSON_OUTPUT; then
    echo "{"
    echo "  \"total\": $TOTAL,"
    echo "  \"passed\": $PASS,"
    echo "  \"failed\": $FAIL,"
    echo "  \"errors\": ["
    for i in "${!ERRORS[@]}"; do
        comma=""
        [[ $i -lt $((${#ERRORS[@]} - 1)) ]] && comma=","
        echo "    \"${ERRORS[$i]}\"$comma"
    done
    echo "  ]"
    echo "}"
else
    echo ""
    echo "=== Validation Summary ==="
    echo "Total:  $TOTAL"
    echo "Passed: $PASS"
    echo "Failed: $FAIL"
    if [[ $FAIL -gt 0 ]]; then
        echo ""
        echo "Failures:"
        for err in "${ERRORS[@]}"; do
            echo "  - $err"
        done
    fi
fi

# Exit with failure if any files failed
[[ $FAIL -eq 0 ]]

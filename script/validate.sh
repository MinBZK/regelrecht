#!/usr/bin/env bash
# Validate regulation YAML files against the law schema.
#
# Usage:
#   script/validate.sh corpus/central/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml
#   script/validate.sh corpus/central/regulation/nl/**/*.yaml
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# If no files given, validate all regulation YAML files
if [ $# -eq 0 ]; then
    FILES=()
    while IFS= read -r -d '' f; do
        FILES+=("$f")
    done < <(find "$REPO_ROOT/corpus/central/regulation" -name '*.yaml' -print0 | sort -z)
    set -- "${FILES[@]}"
fi

exec cargo run --manifest-path "$REPO_ROOT/packages/engine/Cargo.toml" --features validate --bin validate -- "$@"

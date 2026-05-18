#!/usr/bin/env bash
# Validate note sidecar files against the annotation schema (RFC-005, RFC-016).
#
# Resolves every note against its target law and reports orphaned/ambiguous
# notes and unknown tag values as warnings (not errors).
#
# Usage:
#   script/validate-annotations.sh                       # all note files
#   script/validate-annotations.sh corpus/annotations/zorgtoeslagwet/annotations.yaml
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

exec cargo run --manifest-path "$REPO_ROOT/packages/engine/Cargo.toml" \
    --features validate --bin validate-annotations -- "$@"

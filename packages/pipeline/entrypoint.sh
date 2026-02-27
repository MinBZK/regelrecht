#!/bin/sh
set -e

OUTPUT_DIR="${REGULATION_REPO_PATH:-/data/regulation-repo}"
mkdir -p "$OUTPUT_DIR"

exec regelrecht-harvest-worker

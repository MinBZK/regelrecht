#!/bin/sh
set -e

# Set HOME so git config works for the app user
export HOME=/data

OUTPUT_DIR="${REGULATION_REPO_PATH:-/data/regulation-repo}"
mkdir -p "$OUTPUT_DIR"

exec regelrecht-harvest-worker

#!/bin/sh
set -e

# Set HOME so git config works for the app user
export HOME=/tmp/app-home
mkdir -p "$HOME"

# Create output directory at runtime (not in Dockerfile) because
# RIG runs containers with a read-only root filesystem.
OUTPUT_DIR="${REGULATION_REPO_PATH:-/tmp/regulation-repo}"
mkdir -p "$OUTPUT_DIR"
export REGULATION_REPO_PATH="$OUTPUT_DIR"

exec regelrecht-harvest-worker

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

# Create corpus repo directory at runtime (read-only root filesystem)
CORPUS_DIR="${CORPUS_REPO_PATH:-/tmp/corpus-repo}"
mkdir -p "$CORPUS_DIR"
export CORPUS_REPO_PATH="$CORPUS_DIR"

exec regelrecht-harvest-worker

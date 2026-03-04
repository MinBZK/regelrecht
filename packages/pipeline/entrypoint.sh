#!/bin/sh
set -e

# Set HOME so git config works for the app user
export HOME=/tmp/app-home
mkdir -p "$HOME"

OUTPUT_DIR="${REGULATION_REPO_PATH:-/data/regulation-repo}"

# Try to create the output directory; if /data is a root-owned volume,
# fall back to a writable location under /tmp.
if ! mkdir -p "$OUTPUT_DIR" 2>/dev/null; then
    OUTPUT_DIR="/tmp/regulation-repo"
    export REGULATION_REPO_PATH="$OUTPUT_DIR"
    mkdir -p "$OUTPUT_DIR"
    echo "WARN: /data not writable, using $OUTPUT_DIR" >&2
fi

exec regelrecht-harvest-worker

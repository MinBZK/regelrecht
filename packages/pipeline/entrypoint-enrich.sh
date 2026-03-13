#!/bin/sh
set -e

# Set HOME so git config and CLI tools work for the app user.
# RIG runs containers with a read-only root filesystem, so we
# use /tmp for all writable state.
export HOME=/tmp/app-home
mkdir -p "$HOME"

# Create output directory at runtime (read-only root filesystem)
OUTPUT_DIR="${REGULATION_REPO_PATH:-/tmp/regulation-repo}"
mkdir -p "$OUTPUT_DIR"
export REGULATION_REPO_PATH="$OUTPUT_DIR"

# Create corpus repo directory at runtime
CORPUS_DIR="${CORPUS_REPO_PATH:-/tmp/corpus-repo}"
mkdir -p "$CORPUS_DIR"
export CORPUS_REPO_PATH="$CORPUS_DIR"

# --- OpenCode/VLAM auth ---
# Write auth.json from VLAM_API_KEY secret so opencode can authenticate
# with the VLAM API. The provider config (opencode.json) is baked into
# the image; only the key is injected at runtime.
if [ -n "$VLAM_API_KEY" ]; then
  mkdir -p "$HOME/.local/share/opencode"
  # Use printf to safely write the key without shell interpolation issues.
  # The key is treated as a raw string — no shell expansion occurs.
  printf '{"vlam":{"type":"api","key":"%s"}}' "$VLAM_API_KEY" \
    > "$HOME/.local/share/opencode/auth.json"
  chmod 600 "$HOME/.local/share/opencode/auth.json"
fi

# Set up opencode config in user-writable location
mkdir -p "$HOME/.config/opencode"
cp /etc/opencode/opencode.json "$HOME/.config/opencode/opencode.json"
ln -sf /opt/opencode-plugins/node_modules "$HOME/.config/opencode/node_modules"
printf '{"dependencies":{"@ai-sdk/openai-compatible":"*"}}' \
  > "$HOME/.config/opencode/package.json"

# --- Claude auth ---
# ANTHROPIC_API_KEY is read directly from env by claude CLI — no file needed.

exec regelrecht-enrich-worker

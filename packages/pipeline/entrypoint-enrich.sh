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
  # Use Node.js to safely JSON-encode the API key, avoiding shell injection
  # and broken JSON from keys containing quotes, backslashes, or percent signs.
  node -e "
    const fs = require('fs');
    const key = process.env.VLAM_API_KEY;
    const data = {vlam: {type: 'api', key: key}};
    fs.writeFileSync(process.argv[1], JSON.stringify(data));
  " "$HOME/.local/share/opencode/auth.json"
  chmod 600 "$HOME/.local/share/opencode/auth.json"
fi

# Set up opencode config in user-writable location.
# If VLAM_BASE_URL is set, override the demo URL baked into opencode.json.
mkdir -p "$HOME/.config/opencode"
if [ -n "$VLAM_BASE_URL" ]; then
  node -e "
    const fs = require('fs');
    const config = JSON.parse(fs.readFileSync('/etc/opencode/opencode.json', 'utf8'));
    config.provider.vlam.options.baseURL = process.env.VLAM_BASE_URL;
    fs.writeFileSync(process.argv[1], JSON.stringify(config, null, 2));
  " "$HOME/.config/opencode/opencode.json"
else
  cp /etc/opencode/opencode.json "$HOME/.config/opencode/opencode.json"
fi
ln -sf /opt/opencode-plugins/node_modules "$HOME/.config/opencode/node_modules"
printf '{"dependencies":{"@ai-sdk/openai-compatible":"*"}}' \
  > "$HOME/.config/opencode/package.json"

# --- Claude auth ---
# ANTHROPIC_API_KEY is read directly from env by claude CLI — no file needed.

exec regelrecht-enrich-worker

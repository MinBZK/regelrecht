#!/bin/sh
# Start the demo: nginx for the static app and, when configured, the backend
# behind the "why" button next to it on localhost.
#
# The backend needs two things: a way to reach the model
# (CLAUDE_CODE_OAUTH_TOKEN or ANTHROPIC_API_KEY) and the password that unlocks
# the button (DEMO_WHY_PASSWORD). Missing either is not an error. nginx then
# answers 502 on /api/why, the app hides the feature, and the demo is the static
# site it always was.
set -eu

if [ -n "${DEMO_WHY_PASSWORD:-}" ] && [ -n "${CLAUDE_CODE_OAUTH_TOKEN:-}${ANTHROPIC_API_KEY:-}" ]; then
  echo "why: starting on 127.0.0.1:7401"
  # The CLI keeps its config under $HOME; the nginx user has none it can write.
  mkdir -p /tmp/demo-why-home
  HOME=/tmp/demo-why-home PORT=7401 node /app/why/why.mjs &
else
  echo "why: no DEMO_WHY_PASSWORD or no token, the why button stays off"
fi

exec nginx -g 'daemon off;'

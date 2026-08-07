#!/bin/sh
set -eu

# GF_SECURITY_SECRET_KEY is required for signing session cookies and other
# secrets. Without it, Grafana uses a hardcoded default that is insecure.
if [ -z "${GF_SECURITY_SECRET_KEY:-}" ]; then
  echo "WARNING: GF_SECURITY_SECRET_KEY is not set — Grafana will use its insecure default."
  echo "WARNING: Set it via ZAD env_vars. Generate one with: openssl rand -hex 32"
fi

# Map ZAD-provided OIDC env vars to Grafana's generic OAuth config.
# ZAD injects: OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_URL, OIDC_REALM
# Note: OIDC_DISCOVERY_URL is also provided but Grafana doesn't support a
# single discovery URL — we construct the individual endpoints manually.

if [ -z "${OIDC_CLIENT_ID:-}" ] || [ -z "${OIDC_CLIENT_SECRET:-}" ] || [ -z "${OIDC_URL:-}" ] || [ -z "${OIDC_REALM:-}" ]; then
  echo "WARNING: OIDC env vars not set — starting Grafana without OIDC authentication."
  echo "WARNING: Set OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_URL, OIDC_REALM to enable OIDC."
  export GF_AUTH_GENERIC_OAUTH_ENABLED=false
else
  export GF_AUTH_GENERIC_OAUTH_ENABLED=true
  export GF_AUTH_GENERIC_OAUTH_CLIENT_ID="${OIDC_CLIENT_ID}"
  export GF_AUTH_GENERIC_OAUTH_CLIENT_SECRET="${OIDC_CLIENT_SECRET}"
  export GF_AUTH_GENERIC_OAUTH_AUTH_URL="${OIDC_URL}/realms/${OIDC_REALM}/protocol/openid-connect/auth"
  export GF_AUTH_GENERIC_OAUTH_TOKEN_URL="${OIDC_URL}/realms/${OIDC_REALM}/protocol/openid-connect/token"
  export GF_AUTH_GENERIC_OAUTH_API_URL="${OIDC_URL}/realms/${OIDC_REALM}/protocol/openid-connect/userinfo"
  # Disable local login form when OIDC is the auth path
  export GF_AUTH_DISABLE_LOGIN_FORM=true
fi

# Always set a strong random admin password to avoid the insecure admin/admin
# default. The login form is disabled when OIDC is active, so the admin account
# is only reachable over the loopback API inside the container.
if [ -z "${GF_SECURITY_ADMIN_PASSWORD:-}" ]; then
  export GF_SECURITY_ADMIN_PASSWORD
  GF_SECURITY_ADMIN_PASSWORD=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
fi

# Mattermost webhook URL for alert notifications.
# Must be set as env var on the grafana component in ZAD.
if [ -z "${MATTERMOST_WEBHOOK_URL:-}" ]; then
  echo "WARNING: MATTERMOST_WEBHOOK_URL not set — alerts will not be delivered to Mattermost."
  # Set a placeholder so Grafana provisioning doesn't fail on empty variable.
  export MATTERMOST_WEBHOOK_URL="http://localhost:0/webhook-not-configured"
fi

# Start Grafana in the background
/run.sh "$@" &
GRAFANA_PID=$!
trap 'kill -TERM $GRAFANA_PID' TERM INT

# Wait for Grafana to exit
wait $GRAFANA_PID

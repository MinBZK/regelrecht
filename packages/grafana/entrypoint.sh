#!/bin/sh
set -eu

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
  # Disable local login form and admin when OIDC is the auth path
  export GF_AUTH_DISABLE_LOGIN_FORM=true
  export GF_SECURITY_DISABLE_INITIAL_ADMIN_CREATION=true
fi

# Mattermost webhook URL for alert notifications.
# Must be set as env var on the grafana component in ZAD.
if [ -z "${MATTERMOST_WEBHOOK_URL:-}" ]; then
  echo "WARNING: MATTERMOST_WEBHOOK_URL not set — alerts will not be delivered to Mattermost."
  # Set a placeholder so Grafana provisioning doesn't fail on empty variable.
  export MATTERMOST_WEBHOOK_URL="http://localhost:0/webhook-not-configured"
fi

exec /run.sh "$@"

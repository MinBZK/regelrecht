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

# Start Grafana in the background
/run.sh "$@" &
GRAFANA_PID=$!
trap 'kill -TERM $GRAFANA_PID' TERM INT

# Configure Git Sync for dashboard version control if GITHUB_PAT is set.
if [ -n "${GITHUB_PAT:-}" ]; then
  echo "Waiting for Grafana to become ready..."
  GRAFANA_READY=false
  for i in $(seq 1 30); do
    if wget -q -O /dev/null "http://localhost:${GF_SERVER_HTTP_PORT:-8000}/api/health" 2>/dev/null; then
      GRAFANA_READY=true
      break
    fi
    sleep 2
  done

  if [ "$GRAFANA_READY" = false ]; then
    echo "ERROR: Grafana did not become ready in 60s — skipping Git Sync setup"
  else
  # Create repository CRD for Git Sync
  REPO_DIR=$(mktemp -d)
  cat > "${REPO_DIR}/repository.yaml" <<GITEOF
apiVersion: provisioning.grafana.app/v0alpha1
kind: Repository
metadata:
  name: regelrecht-dashboards
spec:
  sync:
    enabled: true
    intervalSeconds: 60
    target: folder
  workflows:
    - write
    - branch
  title: Regelrecht Dashboards
  type: github
  github:
    url: ${GITHUB_REPO_URL:-https://github.com/MinBZK/regelrecht}
    branch: ${GITHUB_BRANCH:-main}
    path: packages/grafana/dashboards/
secure:
  token:
    create: "${GITHUB_PAT}"
GITEOF

  export GRAFANA_SERVER="http://localhost:${GF_SERVER_HTTP_PORT:-8000}"
  export GRAFANA_USER=admin
  export GRAFANA_PASSWORD="${GF_SECURITY_ADMIN_PASSWORD:-admin}"
  export GRAFANA_ORG_ID=1

  echo "Configuring Git Sync..."
  grafanactl resources push --path "${REPO_DIR}" 2>&1 || echo "WARNING: Git Sync configuration failed"
  rm -rf "${REPO_DIR}"
  fi # GRAFANA_READY
else
  echo "WARNING: GITHUB_PAT not set — Git Sync for dashboards is disabled."
fi

# Wait for Grafana to exit
wait $GRAFANA_PID

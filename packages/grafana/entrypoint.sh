#!/bin/sh
set -eu

# Map ZAD-provided OIDC env vars to Grafana's generic OAuth config.
# ZAD injects: OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_URL, OIDC_REALM
# Note: OIDC_DISCOVERY_URL is also provided but Grafana doesn't support a
# single discovery URL — we construct the individual endpoints manually.

export GF_AUTH_GENERIC_OAUTH_CLIENT_ID="${OIDC_CLIENT_ID}"
export GF_AUTH_GENERIC_OAUTH_CLIENT_SECRET="${OIDC_CLIENT_SECRET}"
export GF_AUTH_GENERIC_OAUTH_AUTH_URL="${OIDC_URL}/realms/${OIDC_REALM}/protocol/openid-connect/auth"
export GF_AUTH_GENERIC_OAUTH_TOKEN_URL="${OIDC_URL}/realms/${OIDC_REALM}/protocol/openid-connect/token"
export GF_AUTH_GENERIC_OAUTH_API_URL="${OIDC_URL}/realms/${OIDC_REALM}/protocol/openid-connect/userinfo"

exec /run.sh "$@"

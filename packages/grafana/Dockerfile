FROM grafana/grafana-oss:11.6.0

# ZAD requires port 8000 for liveprobe
ENV GF_SERVER_HTTP_PORT=8000

# Disable anonymous access
ENV GF_AUTH_ANONYMOUS_ENABLED=false

# Disable local admin account when OIDC is available (set in entrypoint.sh).
# When OIDC vars are missing, the entrypoint keeps the default admin/admin
# account as a temporary fallback. Do NOT expose publicly without OIDC.
# NOTE: GF_SECURITY_SECRET_KEY must be injected at deploy time via ZAD env_vars
# to keep sessions stable across pod restarts.

# Enable OIDC via Keycloak using ZAD-provided env vars.
# ZAD injects OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_URL, and OIDC_REALM
# into all components automatically.
ENV GF_AUTH_GENERIC_OAUTH_ENABLED=true
ENV GF_AUTH_GENERIC_OAUTH_NAME=Keycloak
ENV GF_AUTH_GENERIC_OAUTH_ALLOW_SIGN_UP=true
ENV GF_AUTH_GENERIC_OAUTH_SCOPES="openid email profile"
# Map Keycloak roles to Grafana roles — users in 'grafana-admin' group get Admin
ENV GF_AUTH_GENERIC_OAUTH_ROLE_ATTRIBUTE_PATH="contains(groups[*], 'grafana-admin') && 'Admin' || 'Viewer'"

# Enable unified alerting
ENV GF_UNIFIED_ALERTING_ENABLED=true
ENV GF_ALERTING_ENABLED=false

# Copy provisioning files and entrypoint
COPY provisioning/ /etc/grafana/provisioning/
COPY entrypoint.sh /entrypoint.sh

EXPOSE 8000

ENTRYPOINT ["/entrypoint.sh"]

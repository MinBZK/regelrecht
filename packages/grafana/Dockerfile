FROM grafana/grafana-oss:11.6.0

# ZAD requires port 8000 for liveprobe
ENV GF_SERVER_HTTP_PORT=8000

# Disable anonymous access
ENV GF_AUTH_ANONYMOUS_ENABLED=false

# Disable local admin account — OIDC is the sole auth path.
ENV GF_SECURITY_DISABLE_INITIAL_ADMIN_CREATION=true

# Enable OIDC via Keycloak using ZAD-provided env vars.
# ZAD injects OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_DISCOVERY_URL,
# OIDC_URL, and OIDC_REALM into all components automatically.
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

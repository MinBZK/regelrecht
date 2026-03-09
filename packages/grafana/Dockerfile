FROM grafana/grafana-oss:11.6.0

# ZAD requires port 8000 for liveprobe
ENV GF_SERVER_HTTP_PORT=8000

# Disable anonymous access
ENV GF_AUTH_ANONYMOUS_ENABLED=false

# Disable local admin account — OIDC is the sole auth path.
# Admin role is assigned via Keycloak role mapping below.
ENV GF_SECURITY_DISABLE_INITIAL_ADMIN_CREATION=true

# GF_SECURITY_SECRET_KEY must be injected at deploy time via ZAD env_vars
# to keep sessions stable across pod restarts.

# Enable OIDC via Keycloak
# GF_AUTH_GENERIC_OAUTH_CLIENT_ID and GF_AUTH_GENERIC_OAUTH_CLIENT_SECRET
# must be injected at deploy time via ZAD env_vars.
ENV GF_AUTH_GENERIC_OAUTH_ENABLED=true
ENV GF_AUTH_GENERIC_OAUTH_NAME=Keycloak
ENV GF_AUTH_GENERIC_OAUTH_ALLOW_SIGN_UP=true
ENV GF_AUTH_GENERIC_OAUTH_SCOPES="openid email profile"
# Map Keycloak roles to Grafana roles — users in 'grafana-admin' group get Admin
ENV GF_AUTH_GENERIC_OAUTH_ROLE_ATTRIBUTE_PATH="contains(groups[*], 'grafana-admin') && 'Admin' || 'Viewer'"
# Keycloak URLs default to production realm.
# Override GF_AUTH_GENERIC_OAUTH_*_URL via ZAD env_vars for other environments.
ENV GF_AUTH_GENERIC_OAUTH_AUTH_URL=https://keycloak.rijksapp.nl/realms/regel-k4c-odcn-production/protocol/openid-connect/auth
ENV GF_AUTH_GENERIC_OAUTH_TOKEN_URL=https://keycloak.rijksapp.nl/realms/regel-k4c-odcn-production/protocol/openid-connect/token
ENV GF_AUTH_GENERIC_OAUTH_API_URL=https://keycloak.rijksapp.nl/realms/regel-k4c-odcn-production/protocol/openid-connect/userinfo

# Enable unified alerting
ENV GF_UNIFIED_ALERTING_ENABLED=true
ENV GF_ALERTING_ENABLED=false

# Copy provisioning files
COPY provisioning/ /etc/grafana/provisioning/

EXPOSE 8000

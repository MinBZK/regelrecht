FROM grafana/grafana-oss:11.6.0

# ZAD requires port 8000 for liveprobe
ENV GF_SERVER_HTTP_PORT=8000

# Disable anonymous access and local admin (OIDC is the sole auth path)
ENV GF_AUTH_ANONYMOUS_ENABLED=false
ENV GF_SECURITY_DISABLE_INITIAL_ADMIN_CREATION=true

# Enable OIDC via Keycloak (client_id, client_secret, root_url set at deploy time)
ENV GF_AUTH_GENERIC_OAUTH_ENABLED=true
ENV GF_AUTH_GENERIC_OAUTH_NAME=Keycloak
ENV GF_AUTH_GENERIC_OAUTH_ALLOW_SIGN_UP=true
ENV GF_AUTH_GENERIC_OAUTH_SCOPES="openid email profile"
ENV GF_AUTH_GENERIC_OAUTH_AUTH_URL=https://keycloak.rijksapp.nl/realms/regel-k4c-odcn-production/protocol/openid-connect/auth
ENV GF_AUTH_GENERIC_OAUTH_TOKEN_URL=https://keycloak.rijksapp.nl/realms/regel-k4c-odcn-production/protocol/openid-connect/token
ENV GF_AUTH_GENERIC_OAUTH_API_URL=https://keycloak.rijksapp.nl/realms/regel-k4c-odcn-production/protocol/openid-connect/userinfo

# Enable unified alerting
ENV GF_UNIFIED_ALERTING_ENABLED=true
ENV GF_ALERTING_ENABLED=false

# Copy provisioning files
COPY provisioning/ /etc/grafana/provisioning/

EXPOSE 8000

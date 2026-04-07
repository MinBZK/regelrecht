FROM grafana/grafana-oss:12.4.0@sha256:b0ae311af06228bcfd4a620504b653db80f5b91e94dc3dc2a5b7dab202bcde20

# Install grafanactl for Git Sync configuration
USER root
RUN wget -q "https://github.com/grafana/grafanactl/releases/download/v0.1.9/grafanactl_Linux_x86_64.tar.gz" -O /tmp/grafanactl.tar.gz \
    && echo "b66e7f1b5b7b871c43be240d0247c353021433f407c9fbeabdcbee3c71b41271  /tmp/grafanactl.tar.gz" | sha256sum -c - \
    && tar -xzf /tmp/grafanactl.tar.gz -C /usr/local/bin grafanactl \
    && chmod +x /usr/local/bin/grafanactl \
    && rm /tmp/grafanactl.tar.gz
USER grafana

# ZAD requires port 8000 for liveprobe
ENV GF_SERVER_HTTP_PORT=8000
ENV GF_SERVER_ROOT_URL=https://grafana.regelrecht.rijks.app
ENV GF_ANALYTICS_CHECK_FOR_UPDATES=false
ENV GF_ANALYTICS_CHECK_FOR_PLUGIN_UPDATES=false

# Disable anonymous access
ENV GF_AUTH_ANONYMOUS_ENABLED=false

# Disable local admin account when OIDC is available (set in entrypoint.sh).
# When OIDC vars are missing, the entrypoint keeps the default admin/admin
# account as a temporary fallback. Do NOT expose publicly without OIDC.
# NOTE: GF_SECURITY_SECRET_KEY must be injected at deploy time via ZAD env_vars
# to keep sessions stable across pod restarts.

# OIDC is conditionally enabled in entrypoint.sh based on whether
# ZAD injects OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_URL, OIDC_REALM.
# These static OIDC settings apply when OIDC is enabled:
ENV GF_AUTH_GENERIC_OAUTH_NAME=Keycloak
ENV GF_AUTH_GENERIC_OAUTH_ALLOW_SIGN_UP=false
ENV GF_AUTH_GENERIC_OAUTH_SCOPES="openid email profile"
# Map Keycloak roles to Grafana roles — users in 'grafana-admin' group get Admin
ENV GF_AUTH_GENERIC_OAUTH_ROLE_ATTRIBUTE_PATH="contains(groups[*], 'grafana-admin') && 'Admin' || 'Viewer'"

# Enable unified alerting
ENV GF_UNIFIED_ALERTING_ENABLED=true
ENV GF_ALERTING_ENABLED=false

# Enable Git Sync (experimental) for dashboard version control
ENV GF_FEATURE_TOGGLES_ENABLE=provisioning,kubernetesDashboards

# Copy provisioning files and entrypoint (includes GF_SECURITY_ADMIN_PASSWORD default fix)
COPY provisioning/ /etc/grafana/provisioning/
COPY entrypoint.sh /entrypoint.sh

EXPOSE 8000

ENTRYPOINT ["/entrypoint.sh"]

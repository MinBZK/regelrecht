#!/bin/sh
# Generate runtime environment config from platform variables.
# Placed in /docker-entrypoint.d/ so the nginx base image executes
# it automatically before starting nginx.
cat > /usr/share/nginx/html/env-config.js <<EOF
window.__ENV = {
  DEPLOYMENT_NAME: "${DEPLOYMENT_NAME:-}",
  COMPONENT_NAME: "${COMPONENT_NAME:-}"
};
EOF

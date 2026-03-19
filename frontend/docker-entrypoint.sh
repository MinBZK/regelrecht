#!/bin/sh
# Generate runtime environment config from platform variables
cat > /usr/share/nginx/html/env-config.js <<EOF
window.__ENV = {
  DEPLOYMENT_NAME: "${DEPLOYMENT_NAME:-}",
  COMPONENT_NAME: "${COMPONENT_NAME:-}"
};
EOF

exec nginx -g "daemon off;"

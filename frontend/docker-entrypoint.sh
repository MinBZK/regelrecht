#!/bin/sh
# Generate runtime environment config from platform variables.
# Written to /tmp/ because RIG mounts the root filesystem read-only.
# Nginx serves this file via an alias directive.
cat > /tmp/env-config.js <<EOF
window.__ENV = {
  DEPLOYMENT_NAME: "${DEPLOYMENT_NAME:-}",
  COMPONENT_NAME: "${COMPONENT_NAME:-}"
};
EOF

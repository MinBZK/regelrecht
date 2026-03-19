// This file is overwritten at container startup by docker-entrypoint.sh
// with actual platform environment variables. This default exists for
// local development where no env vars are injected.
window.__ENV = {
  DEPLOYMENT_NAME: "",
  COMPONENT_NAME: ""
};

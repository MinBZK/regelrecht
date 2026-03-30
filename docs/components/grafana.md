# Grafana Monitoring

Grafana provides monitoring dashboards and alerting for the regelrecht platform.

## Overview

- **Technology**: Grafana OSS 12.x
- **Location**: `packages/grafana/`
- **Production URL**: `grafana.regelrecht.rijks.app`

## What it does

A pre-configured Grafana instance with provisioned dashboards, a Prometheus datasource, and alerting rules. Packaged as a Docker image with all configuration baked in.

## Provisioned resources

- **Dashboard**: `regelrecht-overview` - main platform overview
- **Datasource**: Prometheus (for metrics from the Axum backend)
- **Alerting rules**: configured in `provisioning/alerting/`

## Authentication

Uses OIDC (Keycloak) for authentication when `OIDC_CLIENT_ID` and related environment variables are set. Falls back to local auth when OIDC is not configured.

## Running locally

Part of the `just dev` stack (available at `http://localhost:3002`), or standalone:

```bash
docker build -t regelrecht-grafana packages/grafana/
docker run -p 3000:8000 -e GF_SECURITY_SECRET_KEY=dev regelrecht-grafana
```

## Further reading

- [Admin Dashboard](./admin) - the other monitoring interface
- [Deployment](/operations/deployment) - how Grafana is deployed

---
title: "Grafana Monitoring"
description: "The provisioned Grafana instance: one overview dashboard over the harvest and enrichment pipeline, and a daily summary posted to Mattermost."
---

Grafana shows the state of the harvest and enrichment pipeline and posts a daily summary of it to Mattermost. It is not a crate: `packages/grafana/` holds a Dockerfile, an entrypoint script and provisioning files, and everything is baked into the image.

## Overview

- **Technology**: Grafana OSS 12.4 (`grafana/grafana-oss:12.4.0`)
- **Location**: `packages/grafana/`
- **Production URL**: `grafana.regelrecht.rijks.app`
- **Port**: 8000 (ZAD's liveness probe expects it)

## Where the numbers come from

The only datasource is Prometheus, provisioned with a fixed uid (`PBFA97CFB590B2093`) and pointing at the platform's Prometheus in the `rig-prd-operations` namespace. The metrics come from `GET /metrics` on the [Harvester Admin](./admin) API; the local scrape configuration is in `dev/prometheus.yml`, the production one belongs to the platform. `packages/admin/src/metrics.rs` builds those metrics from the pipeline database on each scrape, behind a short cache: job counts per status, law counts per status, average job duration, and failed and exhausted counts split by harvest and enrichment.

## The dashboard

`provisioning/dashboards/json/regelrecht-overview.json` is provisioned into the folder "Regelrecht" as **Regelrecht Overview** (uid `regelrecht-overview`), with a default window of the last 24 hours. Deletion from the UI is disabled.

| Panel | Type | Query |
|-------|------|-------|
| Jobs per status | bar gauge | `regelrecht_jobs` |
| Laws per status | bar gauge | `regelrecht_laws` |
| Failed jobs | stat | `regelrecht_jobs{status="failed"}` |
| Pending jobs | stat | `regelrecht_jobs{status="pending"}` |
| Avg job duration (24h) | stat | `regelrecht_job_duration_avg_seconds` |
| Completed jobs | stat | `regelrecht_jobs{status="completed"}` |
| Jobs over time | time series | `regelrecht_jobs` |
| Avg job duration over time | time series | `regelrecht_job_duration_avg_seconds` |

The finer metrics (`regelrecht_jobs_failed_harvest_24h`, `regelrecht_exhausted_enrich` and the rest) have no panel. Only the alert below reads them.

## The daily summary

`provisioning/alerting/alerts.yaml` defines one rule, **Dagelijkse Regelrecht Update**, in the rule group `regelrecht` (evaluated every 10 minutes). It is a report dressed as an alert: its condition is `$A + $B + 1`, which is always above zero, so it always fires. What arrives in Mattermost is a table built from the annotations:

- laws harvested and laws enriched (`regelrecht_laws` by status)
- failed harvest and enrichment jobs, over the last 24 hours and in total
- laws exhausted for harvesting and for enrichment
- pending and completed jobs

All queries filter on `deployment="regelrecht"`, so only production is reported, never a preview.

Delivery goes through the contact point `mattermost`, a Slack-type receiver (Mattermost accepts Slack-compatible webhooks) posting as "Regelrecht" to `MATTERMOST_WEBHOOK_URL`. The rule has its own notification route with a 24-hour repeat interval and the mute timing `niet-middernacht`, which mutes 01:00 to 24:00 Europe/Amsterdam. Together that yields one message a day, shortly after midnight, through both CET and CEST. Resolve messages are switched off. A query error puts the rule into the alerting state; missing data does not.

## Authentication

`entrypoint.sh` switches Keycloak login on only when all four ZAD-injected OIDC variables are present. It builds the auth, token and userinfo endpoints from `OIDC_URL` and `OIDC_REALM`, because Grafana cannot take a single discovery URL, and then hides the local login form. Users in the Keycloak group `grafana-admin` become Grafana Admin, everyone else Viewer. Anonymous access is off.

When any of the four is missing, Grafana starts without OIDC and logs a warning. The local admin account then remains the only way in, with a random password generated at startup unless `GF_SECURITY_ADMIN_PASSWORD` is set. Do not publish an instance in that state.

## Configuration

| Variable | Required | Purpose |
|----------|----------|---------|
| `OIDC_CLIENT_ID` | for SSO | Keycloak client id |
| `OIDC_CLIENT_SECRET` | for SSO | Keycloak client secret |
| `OIDC_URL` | for SSO | Keycloak base URL |
| `OIDC_REALM` | for SSO | Keycloak realm |
| `GF_SECURITY_SECRET_KEY` | yes | Signs sessions; without it Grafana falls back to its insecure built-in key and the entrypoint warns |
| `GF_SECURITY_ADMIN_PASSWORD` | no | Local admin password; random per start when unset |
| `MATTERMOST_WEBHOOK_URL` | for alerts | Incoming webhook for the daily summary; unset, the entrypoint puts in a placeholder and nothing is delivered |
| `GITHUB_PAT` | no | Turns on experimental Git Sync of dashboards (see below) |
| `GITHUB_REPO_URL`, `GITHUB_BRANCH` | no | Repository and branch for Git Sync; default `https://github.com/MinBZK/regelrecht` and `main` |

ZAD also injects `OIDC_DISCOVERY_URL`, which Grafana does not use. The image sets the rest itself: port, root URL, HSTS and a content security policy with `frame-ancestors 'none'`, and unified alerting.

With `GITHUB_PAT` set, the entrypoint registers a Git Sync repository through `grafanactl` that syncs a `dashboards` directory in the Grafana package every 60 seconds. That directory does not exist in the repository; the provisioned dashboard lives under `provisioning/dashboards/json/` and does not depend on Git Sync.

## Running locally

`just dev` starts Prometheus and Grafana from `docker-compose.dev.yml` and serves Grafana at `http://localhost:3002`, logging in as `admin`/`admin`. That service uses the stock Grafana image with the dashboard and alerting provisioning mounted in, and a datasource from `dev/grafana-datasource-local.yaml` that points at the local Prometheus. It does not use this package's Dockerfile or entrypoint, so OIDC is off.

To test the production image itself:

```bash
docker build -t regelrecht-grafana packages/grafana/
docker run -p 3000:8000 -e GF_SECURITY_SECRET_KEY=dev regelrecht-grafana
```

Its datasource points at the in-cluster Prometheus, so panels stay empty outside the cluster.

## Further reading

- [Harvester Admin](./admin) - serves the `/metrics` endpoint Prometheus scrapes
- [Deployment](/operations/deployment) - how Grafana is deployed

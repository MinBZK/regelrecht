---
title: "PoC Portal"
description: "The Rust service that serves several proof-of-concepts behind one hostname, each behind its own password, and marks every page inside a PoC with how far its model of the law has been checked."
---

The PoC portal is the service behind `poc.regelrecht.rijks.app`. It puts several proof-of-concepts under one hostname, one path prefix each, and asks for a separate password per PoC. The PoCs are case studies built with policy teams; they are not the corpus and not law in force.

## Overview

- **Language**: Rust (Axum)
- **Location**: `packages/poc-portal/` (crate `regelrecht-poc-portal`)
- **Production URL**: `poc.regelrecht.rijks.app`
- **Port**: 8000
- **Register**: `pocs/registry.yaml`, compiled into the binary

## What it does

The platform publishes each deployed component on its own subdomain, so three PoCs as three components would get three hostnames and could not share one. The routing between PoCs therefore happens inside a single container: this one.

A request to `/<slug>/...` belongs to the PoC with that slug. Without a valid cookie for that slug, the portal answers `401` with a password form in the body. It does not redirect, so a deep link keeps its address and signing in continues to the page the visitor asked for. After the right password the portal sets a cookie scoped to `/<slug>` and valid for 30 days.

The index at `/` shows every PoC in the register as a card, plus a card for the [Demo](./demo), which is the one entry that needs no password.

Every HTML page served from inside a PoC gets a strip at the top that the portal splices in after `<body>`. It names the PoC, its status and its `voorbehoud` (the caveat, in the PoC's own words), and links back to the index. The strip is there because a computed amount looks equally convincing whether the rules behind it were checked with lawyers or sketched in an afternoon, and a screenshot or a forwarded deep link skips the index where that would otherwise be said.

## Architecture

| Module | Purpose |
|--------|---------|
| `registry.rs` | Parses and validates the register |
| `config.rs` | Reads the environment at startup and refuses to start when anything is missing |
| `gate.rs` | Password check and the signed cookie |
| `app.rs` | The router: index, password form, static PoCs, proxied PoCs, the strip |
| `proxy.rs` | Reverse proxy to a PoC that runs as its own component |
| `pagina.rs` | The index and the password screen, rendered as HTML with `nldd-*` components |

### The register

`pocs/registry.yaml` is the single place where the PoCs are described. The cards, the served and proxied paths, the password variable per PoC and the deploy filters in `script/deploy-filters.mjs` all derive from it. It is embedded with `include_str!`, so the binary cannot disagree with the register it was built with. To add a PoC or change its status, use the `poc-add` skill, which also covers the deploy wiring the register does not contain.

Each entry has a public half and an internal half. `titel` and `samenvatting` appear on the index and on the password screen, both of which anyone can reach, the latter by guessing a slug. `titel_intern`, `samenvatting_intern`, `tags` and the status are shown only behind the password.

The portal refuses a register with an invalid or duplicate slug, a `voorbehoud` shorter than 30 characters, a static PoC without a `bron`, a proxied PoC without an `upstream`, or a proxied PoC with the assistant switched on. `status` has no default: it is one of `verkenning`, `in-ontwikkeling` or `gevalideerd`, and there is deliberately no value for production.

### Two kinds of PoC

- **`statisch`**: a Vue app built into this image with `POC_BASE=/<slug>/` and served from `/app/static/<slug>` with its own single-page fallback. Today these are `frontend-poc-terugbetaalregimes/` and `frontend-poc-nieuwkomersbekostiging/`, each with its case corpus under `corpus-poc/`. They run the engine as WASM in the browser.
- **`proxy`**: a PoC that runs as its own component, not published on the web, and is forwarded to in-cluster at `http://{deployment}-{upstream}:8000`. The deployment name comes from the pod's hostname, so a preview reaches its own upstream and not production's. Outside the cluster, `POC_UPSTREAM_<SLUG>` sets the address. The path is forwarded unchanged and cookies pass both ways, because the proxied app keeps its own session.

The one proxied PoC today is napp, in `packages/poc-napp/` with its frontend in `frontend-poc-napp/`. It runs the subsidy process from a bill as a chain of requests, decisions, payment orders and objections, on an Axum backend with SQLite and a reconstruction of the bill as its law. It has its own README.

### The gate

One shared password per PoC, read from `POC_PW_<SLUG>` (the slug in capitals, hyphens as underscores). The portal will not start when a PoC in the register has no password, since that PoC would otherwise be open to anyone. Passwords are compared as SHA-256 digests in constant time.

The cookie holds an expiry and an HMAC-SHA256 over the slug and that expiry. The key is `POC_COOKIE_SECRET`, at least 32 characters, and deliberately not derived from the passwords: rotating one password should not invalidate the cookies of every other PoC. Because the slug is inside the signed message, a cookie for one PoC cannot be renamed into a cookie for another. The cookie grants nothing more than "this browser knew the password"; a PoC's own login, such as napp's, sits behind it unchanged.

The redirect after signing in only accepts a path under the PoC's own prefix, so the form cannot be used as an open redirect. The content security policy is `POC_CSP` from `packages/auth/src/security_headers.rs`.

### The policy assistant

A static PoC can have `assistent: true`. The image then also carries `packages/poc-assistent/`, a Node server that runs the Claude Code CLI headless with MCP tools specific to one case, and `start.sh` starts one process per case on localhost. The portal forwards `/<slug>/api/...` to it. Without a `CLAUDE_CODE_OAUTH_TOKEN` or `ANTHROPIC_API_KEY` no assistant starts, `/api` answers `503`, and the app hides its assistant panel.

### Image and deployment

`packages/poc-portal/Dockerfile` builds the engine to WASM, the portal's design-system bundle from `frontend-poc-portal/`, each static PoC with its own base path, and the Rust binary. The portal and napp deploy as two ZAD components, `poc` and `napp`; see [Deployment](/operations/deployment). Only `poc` is published on the web.

## Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `POC_COOKIE_SECRET` | none, required | Key for the cookie signature, at least 32 characters |
| `POC_PW_<SLUG>` | none, required per PoC | The password for that PoC |
| `POC_STATIC_DIR` | `/app/static` | Root of the built static PoCs and of `/_assets` |
| `POC_PORT` | `8000` | Listening port |
| `POC_UPSTREAM_<SLUG>` | derived from `HOSTNAME` | Address of a proxied PoC, for local development |
| `POC_ASSISTENT_<SLUG>` | set by `start.sh` | Address of the assistant for that PoC |

Passwords live only in the platform's environment, never in the repository.

## Further reading

- [Shared Frontend Package](./frontend-shared) - the saved-state, variant and runner modules the static PoCs use
- [Demo](./demo) - the one public entry on the index
- [Deployment](/operations/deployment) - the `poc` and `napp` components

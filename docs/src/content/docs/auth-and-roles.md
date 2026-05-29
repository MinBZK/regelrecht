---
title: "Authentication & Roles"
---

This page describes the role model that gates the editor and harvester-admin
services, how the roles are wired up in Keycloak, and the per-deployment
configuration each service needs.

## The four-layer model

Access is governed by a layered set of realm roles. The hierarchy is encoded
in Keycloak as **composite roles**, a higher role contains the lower one, so
a user holding a higher role automatically holds all lower roles in their
token. The application code never has to check `role-A OR role-B`; each
protected route asks for exactly one role.

```
                  regelrecht-admin
                 /              \
        editor-admin            harvester-admin
              │                       │
        editor-writer           harvester-writer
              │                       │
        editor-reader           harvester-reader
```

A line means "the role above contains the role below" (configured as a
Keycloak composite role).

There are two applications today:

- **editor**: `packages/editor-api` + `frontend/` (law and scenario editor)
- **harvester**: `packages/admin` (harvester job queue & corpus sync dashboard)

### Role reference

| Role | Grants |
|---|---|
| `editor-reader` | Editor: read user-scoped data (favorites, settings) and harvest search. |
| `editor-writer` | Editor: edit laws & scenarios, manage favorites/settings, enqueue harvests. Inherits `editor-reader`. |
| `editor-admin` | Editor: corpus reload, feature-flag changes. Inherits `editor-writer`. |
| `harvester-reader` | Harvester admin: read jobs, sources, law entries, platform info. |
| `harvester-writer` | Harvester admin: enqueue harvest and enrich jobs. Inherits `harvester-reader`. |
| `harvester-admin` | Harvester admin: delete jobs, reset exhausted entries, sync sources. Inherits `harvester-writer`. |
| `regelrecht-admin` | Everything across both apps. Inherits `editor-admin` and `harvester-admin`. |

## Specific (orthogonal) rights

Some functions need their own gate, separate from the writer/reader ladder.
Example: not every `editor-writer` should be able to publish a law. Model
these as their own realm role (e.g. `editor-publish`) and add them to the
relevant `<app>-admin` composite. Users who need the right without the full
admin role get it granted explicitly on top of their writer role.

```
                 editor-admin
                /      |       \
       editor-writer   editor-publish   (and any others)
              |
       editor-reader
```

To add a new specific right:

1. Create a new realm role `<app>-<verb>` in Keycloak.
2. Add it to the composite `<app>-admin` (so admin keeps inheriting everything).
3. Optionally assign it to users who need it without the admin role.
4. In the application code: `route_layer(require_role("<app>-<verb>"))` on the
   protected route.

No changes are needed to existing routes, the pattern is composable.

## JWT shape

The application reads `realm_access.roles` from the ID token. With composite
roles, the token contains the *effective* set of roles after expansion:

```json
// regelrecht-admin user
"realm_access": {
  "roles": [
    "regelrecht-admin",
    "editor-admin", "editor-writer", "editor-reader",
    "harvester-admin", "harvester-writer", "harvester-reader"
  ]
}

// editor-writer user (no publish right)
"realm_access": {
  "roles": ["editor-writer", "editor-reader"]
}
```

## Keycloak setup

1. **Create the roles** in the realm: all seven plus any specific rights you
   need.
2. **Configure composite roles**:
   - `editor-writer` → contains `editor-reader`.
   - `editor-admin` → contains `editor-writer` (+ every editor specific right).
   - `harvester-writer` → contains `harvester-reader`.
   - `harvester-admin` → contains `harvester-writer` (+ every harvester
     specific right).
   - `regelrecht-admin` → contains `editor-admin` and `harvester-admin`.
3. **Realm-roles mapper on the ID token**: each OIDC client (editor,
   harvester-admin) needs a "Realm roles" mapper that injects `realm_access`
   into the **ID token** (not just the access token). Keycloak only adds it
   to the access token by default; without this mapper the service falls
   back to parsing the access token, which is noisier in the logs.
4. **Assign roles to users**.

## Per-deployment configuration

Each service is gated on a **minimum role** at login time, configured via
`OIDC_REQUIRED_ROLE`. Per-route checks layer finer-grained roles on top.

| Component | `OIDC_REQUIRED_ROLE` |
|---|---|
| `editor` | `editor-reader` |
| `harvester-admin` | `harvester-reader` |

If `OIDC_REQUIRED_ROLE` is unset or empty, the service falls back to
`allowed-user` and logs a warning on startup. This default keeps the
pre-RBAC migration path working out of the box; **always set the value
explicitly in production** so the login gate matches the per-app reader
role (`editor-reader` / `harvester-reader`) once the migration completes.

### Setting the env var on ZAD

The `zad-actions/deploy@v4` GitHub action used in `.github/workflows/deploy.yml`
takes only `image` and `clone-from`; env vars are set out-of-band per
component via the ZAD CLI or dashboard. For preview deploys, `clone-from:
regelrecht` carries the value from the production deployment automatically,
you only need to set it once per environment.

```bash
# Production
zad component edit editor --deployment regelrecht \
    --env OIDC_REQUIRED_ROLE=editor-reader
zad component edit harvester-admin --deployment regelrecht \
    --env OIDC_REQUIRED_ROLE=harvester-reader
```

### Pre-existing sessions at deploy time

Sessions created before this code shipped carry `authenticated = true` but no
`SESSION_KEY_ROLES` key. The per-route role check distinguishes "key absent"
(pre-RBAC session) from "key present but empty list" (a legitimately
mis-configured Keycloak): the former returns 401, which triggers the OIDC
re-login redirect, the callback then populates `SESSION_KEY_ROLES` from the
JWT and the session self-heals. **No session flush is required at deploy.**

## Migration from the legacy `allowed-user` role

Earlier deployments used a single `allowed-user` realm role checked at login,
with no per-route gating. To migrate without locking anyone out:

1. **Keycloak (hard prerequisite)**: create the seven new roles, set up
   composites, attach the ID-token mapper, and grant every existing user an
   appropriate new role (most editor users → `editor-writer`). This must be
   fully rolled out before Step 2, any user without one of the new roles
   will get **403 on every API request** once the new code is live, because
   the per-route middleware checks for `editor-reader` / `harvester-reader`
   etc., not `allowed-user`.
2. **Deploy the new code**. If `OIDC_REQUIRED_ROLE` is unset on the existing
   deployment, the new code falls back to `allowed-user` and logs a warning,
   so the login redirect keeps working during the rolling deploy (provided
   step 1 is complete). Per-route checks gate on the new roles immediately,
   so users without one of the new roles will see 403 on every protected
   request until step 1 is rolled out for them. Setting the env var
   explicitly to `allowed-user` is still recommended for clarity. Keep the
   `allowed-user` role granted to all migrated users.
3. **Switch `OIDC_REQUIRED_ROLE`** on each component to its new value
   (`editor-reader` / `harvester-reader`).
4. **Remove the `allowed-user` role** from the realm.

## Operational notes

### Role-change propagation

The role set is read from the JWT at login and cached in the session
(`SESSION_KEY_ROLES`) for the lifetime of that session. Per-request middleware
reads this cached list rather than re-parsing the token, which means:

- **Role changes in Keycloak only take effect on the next login.** Granting a
  user a new role (e.g. promoting `editor-writer` to `editor-admin`) requires
  the user to log out and back in before the new role is honoured by the
  application.
- **Role revocation has the same delay.** Removing a role in Keycloak does
  *not* immediately revoke access, the live session continues to carry the
  expanded role list until it expires.

For emergency revocation (compromised account, immediate downgrade) the
session store must also be cleared so the cached role list cannot be reused.
Sessions live in the PostgreSQL `tower_sessions.session` table on each
service's database:

```sql
-- Revoke all live sessions for a specific user (by Keycloak sub):
-- The session stores the Keycloak subject under the key `person_sub`
-- (see SESSION_KEY_SUB in packages/auth/src/handlers.rs).
DELETE FROM tower_sessions.session
WHERE data::jsonb -> 'data' ->> 'person_sub' = '<keycloak-sub>';

-- Nuclear option — invalidate every active session on the service:
TRUNCATE tower_sessions.session;
```

After deleting the session row(s), the affected user is forced through the
OIDC login again, which re-reads roles from Keycloak.

### Auth-disabled mode (dev/local only)

When the OIDC environment variables are not configured (`OIDC_CLIENT_ID`
unset), each service starts with **all per-route auth checks bypassed**.
Every tier (reader, writer, *and* admin) is reachable without a session.
This mode exists for local development convenience (no Keycloak required)
and emits a `warn!` line at startup:

```
OIDC authentication is DISABLED — editor is unprotected.
All routes (editor-reader/writer/admin tiers) bypass auth checks.
Do NOT run this configuration in production.
```

The same applies to the harvester-admin service. **Never deploy a service
without OIDC configured**: the warning is the only safeguard, and the
admin-tier routes (corpus reload, feature-flag toggles, job deletion, source
sync) are fully open in this mode.

## Programmatic access (admin API key)

The harvester-admin service accepts a bearer API key on **GET** and **DELETE**
requests (`ADMIN_API_KEY` env var). This is an out-of-band trust path, the holder
is treated as a `regelrecht-admin`-equivalent for those methods. POST is never
allowed via the API key path; use a user session with `harvester-writer` or
`harvester-admin` for mutations. The editor service has no API key path.

## Implementation pointers

- Shared crate: `packages/auth/`, `require_role(role)` middleware factory.
- Editor routes: `packages/editor-api/src/main.rs`, router split into
  public / reader / writer / admin groups.
- Harvester-admin routes: `packages/admin/src/main.rs`, router split into
  reader / writer / admin groups; `require_auth(role)` in
  `packages/admin/src/middleware.rs` keeps the API-key bypass.
- Roles are persisted in the session at login (`SESSION_KEY_ROLES`) so
  per-request checks don't re-parse the JWT.

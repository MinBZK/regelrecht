# Authentication & Roles

This page describes the role model that gates the editor and harvester-admin
services, how the roles are wired up in Keycloak, and the per-deployment
configuration each service needs.

## The four-layer model

Access is governed by a layered set of realm roles. The hierarchy is encoded
in Keycloak as **composite roles** — a higher role contains the lower one, so
a user holding a higher role automatically holds all lower roles in their
token. The application code never has to check `role-A OR role-B`; each
protected route asks for exactly one role.

```
                  platform-admin
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

- **editor** — `packages/editor-api` + `frontend/` (law and scenario editor)
- **harvester** — `packages/admin` (harvester job queue & corpus sync dashboard)

### Role reference

| Role | Grants |
|---|---|
| `editor-reader` | Editor: read user-scoped data (favorites, settings) and harvest search. |
| `editor-writer` | Editor: edit laws & scenarios, manage favorites/settings, enqueue harvests. Inherits `editor-reader`. |
| `editor-admin` | Editor: corpus reload, feature-flag changes. Inherits `editor-writer`. |
| `harvester-reader` | Harvester admin: read jobs, sources, law entries, platform info. |
| `harvester-writer` | Harvester admin: enqueue harvest and enrich jobs. Inherits `harvester-reader`. |
| `harvester-admin` | Harvester admin: delete jobs, reset exhausted entries, sync sources. Inherits `harvester-writer`. |
| `platform-admin` | Everything across both apps. Inherits `editor-admin` and `harvester-admin`. |

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

No changes are needed to existing routes — the pattern is composable.

## JWT shape

The application reads `realm_access.roles` from the ID token. With composite
roles, the token contains the *effective* set of roles after expansion:

```json
// platform-admin user
"realm_access": {
  "roles": [
    "platform-admin",
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
   - `platform-admin` → contains `editor-admin` and `harvester-admin`.
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

A missing or empty `OIDC_REQUIRED_ROLE` is a startup error when OIDC is
otherwise configured — there is no implicit default. This prevents a
misconfigured deployment from silently accepting anyone with any role.

### Setting the env var on ZAD

The `zad-actions/deploy@v4` GitHub action used in `.github/workflows/deploy.yml`
takes only `image` and `clone-from`; env vars are set out-of-band per
component via the ZAD CLI or dashboard. For preview deploys, `clone-from:
regelrecht` carries the value from the production deployment automatically —
you only need to set it once per environment.

```bash
# Production
zad component edit editor --deployment regelrecht \
    --env OIDC_REQUIRED_ROLE=editor-reader
zad component edit harvester-admin --deployment regelrecht \
    --env OIDC_REQUIRED_ROLE=harvester-reader
```

## Migration from the legacy `allowed-user` role

Earlier deployments used a single `allowed-user` realm role checked at login,
with no per-route gating. To migrate without locking anyone out:

1. **Keycloak**: create the seven new roles, set up composites, attach the
   ID-token mapper, and grant existing users an appropriate role
   (most editor users → `editor-writer`).
2. **Deploy the new code** with `OIDC_REQUIRED_ROLE=allowed-user` set
   explicitly on both components (the new code rejects an empty value), *and*
   grant `allowed-user` to all migrated users so login still succeeds during
   the transition.
3. **Switch `OIDC_REQUIRED_ROLE`** on each component to its new value
   (`editor-reader` / `harvester-reader`).
4. **Remove the `allowed-user` role** from the realm.

## Programmatic access (admin API key)

The harvester-admin service accepts a bearer API key on **GET** and **DELETE**
requests (`API_KEY` env var). This is an out-of-band trust path — the holder
is treated as a `platform-admin`-equivalent for those methods. POST is never
allowed via the API key path; use a user session with `harvester-writer` or
`harvester-admin` for mutations. The editor service has no API key path.

## Implementation pointers

- Shared crate: `packages/auth/` — `require_role(role)` middleware factory.
- Editor routes: `packages/editor-api/src/main.rs` — router split into
  public / reader / writer / admin groups.
- Harvester-admin routes: `packages/admin/src/main.rs` — router split into
  reader / writer / admin groups; `require_auth(role)` in
  `packages/admin/src/middleware.rs` keeps the API-key bypass.
- Roles are persisted in the session at login (`SESSION_KEY_ROLES`) so
  per-request checks don't re-parse the JWT.

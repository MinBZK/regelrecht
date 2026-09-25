---
title: "Shared Frontend Package"
description: "The npm workspace package with the code the Vue frontends share: API calls, sign-in state, the color scheme, and the Gherkin runner the editor, the demo and the PoCs run scenarios with."
---

`@regelrecht/frontend-shared` holds the frontend code that more than one Vue app needs. It is not built or published on its own: the apps import its source files through the npm workspace, and Vite bundles whatever they import.

## Overview

- **Language**: JavaScript and Vue 3 single-file components
- **Location**: `packages/frontend-shared/`
- **Type**: npm workspace package (`private`, not published)
- **Tests**: Vitest with happy-dom, `npm test -w packages/frontend-shared`

## What it does

The editor is the reference for what lives here, and the other apps conform to it. Which app uses what:

| App | Imports |
|-----|---------|
| Editor (`frontend/`) | API calls, sign-in state, the GitHub link status, the color scheme, value helpers, the Gherkin runner |
| Demo (`frontend-demo/`) | The color scheme, value helpers, the Gherkin runner |
| The two static PoCs (`frontend-poc-*`) | The Gherkin runner, saved state, browser variants, the line diff, the reload-on-stale-bundle handler, the shared components |
| Lawmaking (`frontend-lawmaking/`) | Lists the package as a dependency, imports nothing from it today |

## Architecture

The package root (`src/index.js`) exports only the small primitives. Everything else has its own subpath in the `exports` map of `package.json`, so an app pulls in only what it imports: `./gherkin`, `./lib/*`, `./components/*` and the individual `./*.js` modules.

### Exported from the root

| Module | Purpose |
|--------|---------|
| `apiFetch.js` | A thin wrapper around `fetch`. A non-ok response throws an `ApiError` carrying the status, the body text and the content type. It deliberately has no retries, timeouts or auth handling; `apiFetch` returns the raw `Response` so callers keep the headers the ETag/If-Match handling relies on |
| `useAuth.js` | Sign-in state from `/auth/status`, the realm roles of the current user, and `hasRole` / `hasAnyRole` for showing role-specific UI. The backend still enforces every route |
| `useGithubAuth.js` | Whether the user has linked a GitHub account, from the editor API's `/auth/github/status`. When the endpoint is missing or fails, the controls are hidden |
| `useColorScheme.js` | The light/dark/auto picker. It sets `data-scheme` on `<html>`, which the design system keys its dark tokens on, and leaves it off for `auto`. Persistence is injected: the editor stores the choice server-side, other apps use `localStorage` |
| `values.js` | `isUnknown` and `missingFacts`: one definition of how the engine's Unknown value ([RFC-036](/rfcs/rfc-036)) arrives in JavaScript |

### The Gherkin runner (`./gherkin`)

A JavaScript runner for the canonical scenario language in `bdd/grammar.yaml`, run against the WASM build of the engine. The editor's scenario builder and the demo's scenario page both run on it.

| Module | Purpose |
|--------|---------|
| `parser.js` | Wraps `@cucumber/gherkin` into a small AST of scenarios, steps and data tables |
| `grammar.generated.js` | The step patterns, generated from `bdd/grammar.yaml` by `just bdd-codegen`. Never edit it by hand; CI regenerates it and fails when the committed file differs |
| `steps.js` | Turns the generated grammar into step definitions and matches step text against them |
| `actions.js` | The one dispatch that gives each grammar action its behavior, the JavaScript counterpart of the Rust BDD dispatch |
| `context.js` | `ExecutionContext`, the state of one scenario run |

The runner supports only the `core` tier. A step from another tier throws instead of doing nothing, so a scenario that needs more than the WASM engine offers fails visibly.

### Modules for the PoCs

These came in with the proof-of-concepts behind the [PoC Portal](./poc-portal). Their comments and identifiers are partly Dutch, like the apps that use them.

| Module | Purpose |
|--------|---------|
| `useBewaardeStand.js` | Keeps choices and settings across a page reload in `localStorage`, under a prefix per case. Two PoCs share one origin behind the portal, and without the prefix they would read each other's state. Edited law YAML is deliberately not stored here |
| `browserVarianten.js` | Variants a user saves in their own browser, in the same shape as the variants checked into the case. Each file keeps a fingerprint of the law text it started from, so the app can say when the corpus has changed underneath it |
| `reloadOnStaleBundle.js` | Reloads the page when a lazily loaded view no longer exists after a deploy, at most once per ten seconds |
| `lib/diff.js` | A line diff (longest common subsequence) for showing a change to a law's YAML, with no dependency |
| `components/` | `Paneel.vue` (a collapsible section), `KolommenMenu.vue` (a menu to pick variants as columns), `AssistentMeldingen.vue` (notices from the policy assistant) and `OptimalisatiepadChart.vue` (the path the assistant takes toward a target, which needs the optional `echarts` peer dependency) |

## Further reading

- [Editor](./frontend) - the main consumer, and the reference for this package
- [Demo](./demo) - runs scenarios in the browser with the Gherkin runner
- [PoC Portal](./poc-portal) - the PoCs that use the saved-state and variant modules
- [Scenarios](/concepts/scenarios) - the scenario language the runner executes

# RegelRecht UI Prototype

Static HTML/CSS/JS prototype for the RegelRecht user interface.

## Prerequisites

- Node.js 18+

## Setup

```bash
cd frontend
npm install
```

## Development

```bash
# Start development server with hot reload
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
```

## Testing

### Mocked component + e2e suite (`e2e/`)

`npm run test:e2e` runs the Playwright specs in `e2e/` against a local Vite dev
server with the backend **mocked** (see `playwright.config.js`). Fast, hermetic,
no auth. Selector helpers live in `e2e/helpers.js`.

### Deployed-preview smoke suite (`e2e-smoke/`)

A **token-free, deterministic** smoke test that drives the core editor flow
against a **deployed** PR-preview (or production) through the full stack:
real Keycloak login, editor-api, DB and WASM. It is the SpecFlow/Selenium-style
counterpart to the mocked suite — no LLM tokens, repeatable on every change.

```bash
# From the repo root — pass the target positionally:
just smoke-preview 886                                  # editor-pr886 preview
just smoke-preview https://editor.regelrecht.rijks.app  # any deployed URL

# Or as a just variable (must precede the recipe name):
just PR=886 smoke-preview
just URL=https://editor.regelrecht.rijks.app smoke-preview
```

The recipe sets `SMOKE_BASE_URL` and injects the shared test-user credentials
(`SMOKE_USER` / `SMOKE_PASS`) from the local cred file — credentials are **never**
committed and only ever read from the environment. `global-setup.js` logs in once
and stores the session in `.auth/state.json` (git-ignored); each spec reuses it.

Layout:

- `playwright.smoke.config.js` — no `webServer`; `baseURL` from `SMOKE_BASE_URL`.
- `e2e-smoke/global-setup.js` — deterministic login + `/auth/status` check.
- `e2e-smoke/smoke.spec.js` — the core flow (library → trajecten → create traject →
  editor → edit → save → reload → persist), cleaning up every traject it creates.
- `e2e-smoke/smoke-helpers.js` — preview-only helpers (login, traject CRUD). Selector
  helpers are **reused** from `e2e/helpers.js`, not copied.

**This smoke suite is the living contract for the editor.** When you add or change
an editor feature, **extend `e2e-smoke/smoke.spec.js`** so the deployed preview keeps
smoking the full, growing set of core behaviour — not just the frontend in isolation.

## Browser Support

This prototype uses modern CSS features including:
- CSS `:has()` selector (Chrome 105+, Safari 15.4+, Firefox 121+)
- CSS custom properties (variables)
- CSS Grid and Flexbox

## Project Structure

```
frontend/
├── assets/
│   ├── icons/          # SVG icons
│   └── rijkswapen.svg  # National emblem
├── css/
│   ├── components/     # Component-specific styles
│   ├── layout.css      # Page layout styles
│   ├── main.css        # CSS entry point
│   ├── reset.css       # CSS reset
│   └── variables.css   # Design tokens
├── fonts/              # Rijksoverheid fonts
├── index.html          # Single-page app entry point
└── src/
    ├── main.js         # Vue app bootstrap + router
    ├── router.js       # Vue Router (Library + Editor routes)
    ├── LibraryApp.vue  # Library view (/library)
    └── EditorApp.vue   # Editor view (/editor/:lawId?)
```

## Components

### From @nldd/design-system
- `<rvo-button>` - Buttons
- `<rvo-navbar>` - Navigation bar
- `<rvo-toggle-button>` - Toggle buttons

### Custom CSS Components
- Lists with collapsible items
- Tab navigation
- Split pane layouts

---
title: "Frontend"
description: "The Vue 3 law editor and library browser for working with machine-readable law."
---

The frontend is a law editor and library browser built with Vue 3 and Vite.

## Overview

- **Framework**: Vue 3 (Composition API)
- **Build tool**: Vite 8
- **Design system**: [@nldd/design-system](https://www.npmjs.com/package/@nldd/design-system) web components
- **Location**: `frontend/`

## Pages

### Library

Browse the corpus of Dutch laws in a 3-pane layout:

```
┌──────────────┬──────────────┬──────────────┐
│   Law List   │   Articles   │   Detail     │
│              │              │              │
│  Search &    │  Filtered    │  Text /      │
│  filter laws │  articles    │  Machine /   │
│              │              │  YAML tabs   │
└──────────────┴──────────────┴──────────────┘
```

- Pane 1: All laws from corpus, searchable
- Pane 2: Articles of selected law with favorites and filtering
- Pane 3: Article detail with tabs for text, machine-readable view, and raw YAML

### Editor

Edit law articles with a split-pane interface:

```
┌────────────────────────────────────────────┐
│  Document Tab Bar (open articles)          │
├──────────────────────┬─────────────────────┤
│   Legal Text         │  Machine-Readable   │
│                      │  or YAML View       │
│   With formatting    │                     │
│   toolbar            │  Segmented control  │
│   (bold, italic,     │  to switch view     │
│    lists, hr)        │                     │
└──────────────────────┴─────────────────────┘
```

- Floating action sheet for editing operations and conditions
- Law loaded from URL parameter (`?law=...`)

## Design System

The frontend is built almost entirely from `@nldd/design-system` web components (custom-element prefix `nldd-`), imported in the JS entry point (`src/main.js` imports `@nldd/design-system` and its styles). A representative slice of the components in use:

| Category | Components used |
|----------|----------------|
| **Layout** | `nldd-page`, `nldd-side-by-side-split-view`, `nldd-navigation-split-view`, `nldd-toolbar`, `nldd-spacer`, `nldd-container` |
| **Navigation** | `nldd-top-title-bar`, `nldd-tab-bar`, `nldd-document-tab-bar`, `nldd-menu` |
| **Inputs** | `nldd-search-field`, `nldd-text-field`, `nldd-multi-line-text-field`, `nldd-dropdown`, `nldd-combo-box`, `nldd-segmented-control` |
| **Lists & cells** | `nldd-list`, `nldd-list-item`, `nldd-text-cell`, `nldd-icon-cell`, `nldd-collection` |
| **Actions & overlays** | `nldd-button`, `nldd-icon-button`, `nldd-button-group`, `nldd-inline-dialog`, `nldd-modal-dialog`, `nldd-sheet` |
| **Content** | `nldd-rich-text`, `nldd-code-viewer`, `nldd-code-editor`, `nldd-title`, `nldd-tag` |

The Rijksoverheid brand color (`#154273`) and typography come from the design system; the only app-level stylesheet is `frontend/css/main.css`, which holds a handful of resets and one `--color-primary` token (the design system provides everything else through its own styles).

## Vue components

The app is split into two top-level shells, `LibraryApp.vue` (the law browser) and `EditorApp.vue` (the split-pane editor), plus ~30 components under `src/components/`. Notable ones:

| Component | Purpose |
|-----------|---------|
| `ArticleText.vue` / `ArticleTextEditor.vue` | Render and edit article text |
| `MachineReadable.vue` | Machine-readable visualization |
| `YamlView.vue` | Raw YAML view |
| `EditSheet.vue` / `ActionSheet.vue` / `OperationSettings.vue` | Editing operations and conditions |
| `LawGraphView.vue` (+ `graph/`) | Cross-law dependency graph |
| `ExecutionTraceView.vue` | Execution trace tree |
| `ScenarioBuilder/Form/Gherkin/Visual/Panel.vue` | BDD scenario authoring |
| `AnnotatedText.vue` / `NoteCreator.vue` | Stand-off notes (RFC-018) |
| `TrajectMenu.vue` / `TrajectMembersDialog.vue` | Traject collaboration |

Shared logic lives in composables (`useLaw.js` for loading and article selection, plus others for settings and corpus URLs).

## Data Loading

Law content comes from the **editor-api** over HTTP, not from static files. `useLaw.js` fetches and saves laws via `lawUrl(...)` (a `GET` to load, a `PUT` to save), with traject-scoped variants for private-repo collaboration. A `scripts/copy-laws.js` prebuild step still exists, but the runtime data flow goes through the API, so the editor can read and write the corpus.

## Development

```bash
cd frontend
npm install
npm run dev          # Start dev server on :3000
```

Or use the full dev stack:

```bash
just dev             # Starts everything with hot reload
```

## Deployment

The editor ships as a single Docker image (`regelrecht-editor`) that bundles the built Vue frontend together with the [editor-api](./editor-api) Rust binary, which serves the static assets and the REST API. It is deployed to RIG/ZAD:

- **Production**: `editor.regelrecht.rijks.app`
- **PR previews**: automatically deployed for each pull request

## Admin Dashboard

A separate admin UI exists at `packages/admin/` for pipeline management:

- **Backend**: Rust (Axum) with PostgreSQL
- **Frontend**: Vue 3 + Vite + `@nldd/design-system` (same stack as the editor)
- **Features**: Law status overview, job management, harvest/enrich triggers
- **Auth**: Optional OIDC integration

See the admin API endpoints:
- `GET /api/law_entries` - query law processing status
- `GET /api/jobs` - query job queue
- `POST /api/harvest-jobs` - create harvest job
- `POST /api/enrich-jobs` - create enrich jobs
- `GET /api/jobs/{id}` - job detail with progress

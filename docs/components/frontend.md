# Frontend

The frontend is a law editor and library browser built with Vue 3 and Vite.

## Overview

- **Framework**: Vue 3 (Composition API)
- **Build tool**: Vite 8
- **Design system**: [@minbzk/storybook](https://github.com/minbzk/storybook) web components
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

The frontend uses the **RegelRecht Design System** (`@minbzk/storybook`), which provides Lit-based web components:

| Category | Components Used |
|----------|----------------|
| **Layout** | `rr-page`, `rr-side-by-side-split-view`, `rr-toolbar`, `rr-box`, `rr-spacer` |
| **Navigation** | `rr-top-navigation-bar`, `rr-tab-bar`, `rr-document-tab-bar` |
| **Inputs** | `rr-search-field`, `rr-text-field`, `rr-drop-down-field`, `rr-segmented-control` |
| **Lists** | `rr-list`, `rr-list-item`, `rr-text-cell`, `rr-label-cell` |
| **Actions** | `rr-button`, `rr-icon-button`, `rr-button-bar` |

Components are registered as custom elements (prefix `rr-`) and work in any HTML context.

### Design Tokens

Design tokens extracted from Figma in `css/variables.css`:

- **Brand color**: `#154273` (Dutch Government blue)
- **Font**: Rijksoverheid Sans (official Dutch government typeface)
- **Spacing**: 8px base unit system
- **Shadows**: Two levels (sm, md)
- **Border radius**: 4px / 6px / 8px

## CSS Architecture

```
css/
├── main.css          # Entry point (imports all)
├── reset.css         # Modern CSS reset
├── variables.css     # Design tokens from Figma
├── layout.css        # Page layout and navigation
└── components/
    ├── list.css      # Lists with selections and badges
    ├── tabs.css      # Tab navigation (CSS-only + rr-toggle-button)
    ├── collapsible.css  # Native <details> accordions
    ├── panes.css     # Split-pane layout, YAML display
    └── editor.css    # Editor-specific components
```

Uses BEM-inspired naming, all colors via CSS variables, modern CSS features (`:has()`, Grid, custom properties).

## Vue Components

| Component | Purpose |
|-----------|---------|
| `LibraryApp.vue` | Library page - 3-pane law browser |
| `EditorApp.vue` | Editor page - split-pane law editor |
| `ArticleText.vue` | Formatted article text rendering |
| `MachineReadable.vue` | Machine-readable visualization |
| `YamlView.vue` | Raw YAML syntax display |
| `ActionSheet.vue` | Modal panel for editing operations |
| `OperationSettings.vue` | Operation parameter configuration |

Shared logic via `useLaw.js` composable (loads YAML, manages article selection).

## Data Loading

Laws are served as **static YAML files** - no backend API for law content:
- `scripts/copy-laws.js` copies laws from `corpus/regulation/` to `public/data/`
- Index at `/data/index.json` (generated from corpus metadata)
- Individual laws at `/data/wet/{law_id}/{date}.yaml`

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

Deployed as a static site via Docker (nginx) to RIG:

- **Production**: https://editor-regelrecht-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl
- **PR Previews**: Automatically deployed for each pull request

## Admin Dashboard

A separate admin UI exists at `packages/admin/` for pipeline management:

- **Backend**: Rust (Axum) with PostgreSQL
- **Frontend**: Vanilla JS + Storybook web components
- **Features**: Law status overview, job management, harvest/enrich triggers
- **Auth**: Optional OIDC integration

See the admin API endpoints:
- `GET /api/law_entries` - query law processing status
- `GET /api/jobs` - query job queue
- `POST /api/harvest-jobs` - create harvest job
- `POST /api/enrich-jobs` - create enrich jobs
- `GET /api/jobs/{id}` - job detail with progress

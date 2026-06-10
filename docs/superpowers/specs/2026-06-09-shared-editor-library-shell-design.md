# Shared editor/library shell — design

**Date:** 2026-06-09
**Status:** Implemented (chrome-store mechanism; see "Injection mechanism")
**Component:** `frontend/` (Vue editor SPA)

## Problem

The editor and the library (bibliotheek) are already part of a single Vue SPA
(one Vite entry, one `vue-router` with `createWebHistory()`), and switching
between them via the `nldd-tab-bar` already navigates client-side — verified
empirically: a `window` marker survives the switch, so there is **no document
reload**.

However, it *feels* like a full page refresh. The cause: `App.vue` is just
`<router-view />` with **no shared layout and no `<keep-alive>`**. `/editor`
maps to `EditorApp.vue` and `/library` to `LibraryApp.vue` — two independent
top-level components. Each switch **unmounts one whole component tree and mounts
the other**, re-running every `onMounted` hook: data refetch, loading
skeletons, scroll reset, and a visible rebuild of the chrome (toolbar, tab bar,
settings menu). That visual rebuild reads as a refresh.

## Goal

Eliminate the chrome rebuild on editor↔library switches by introducing a
**persistent shared shell** that holds the common chrome and stays mounted
across the switch. Only the main body content swaps. No visual change — the UI
must look identical; only the switch behaviour improves.

Non-goal: preserving each view's in-memory body state across switches
(keep-alive). View state is URL-driven (traject ref / law / article in the
route), so a body remount reloads from the URL anyway. Out of scope for this
change.

## Current structure (as-is)

Both `EditorApp.vue` (~2012 lines) and `LibraryApp.vue` (~883 lines) each own
the full chrome skeleton:

```
nldd-app-view
└── nldd-bar-split-view
    ├── slot="primary-bar-md"   nldd-toolbar  (md only)
    ├── slot="primary-bar-lg"   nldd-toolbar  (lg+)
    ├── slot="mobile-bar"       nldd-toolbar  (sm only)
    ├── slot="document-tabs"    (editor only — open document tabs)
    └── slot="main"             body content
```

Each of the three toolbars (md/lg/sm — required by the design system's
breakpoint slots) currently repeats the same chrome inline:

- **Shared across both apps** (~identical markup today): the section
  `nldd-tab-bar` (Bibliotheek/Editor), the search trigger (a `nldd-search-field`
  in the toolbar `slot="center"` at lg, a "Zoeken" button at md/sm),
  `TrajectMenu`, the settings `nldd-menu` (feature flags / colour scheme / auth
  person / login-logout), and the `TrajectDocuments` sheet.
- **Editor-specific**: `nldd-document-tab-bar` in `slot="document-tabs"`, and a
  federated write-back "PR #N" indicator button in the toolbar `slot="end"`.

> **Note (impl):** the original "as-is" snapshot listed the search field as
> library-only. Since PR #746 (full-corpus search) the editor carries the same
> search field, so search is treated as shared chrome. The settings menu also
> differed — the editor listed 7 feature-flag toggles, the library only 4; the
> shell now lists the full editor set (the extra flags only affect editor panes,
> so exposing them from the library is harmless).

Shared state already lives in module-level composables: `useAuth`,
`useColorScheme`, `useFeatureFlags`, `useTrajects`, `useLastVisitedRoute`. The
traject-aware tab targets are computed via `sectionTarget(...)` /
`lastLibraryPath`.

## Design (to-be)

### Component split

1. **`AppShell.vue`** (new) — owns `nldd-app-view` → `nldd-bar-split-view` plus
   the three responsive `primary-bar` toolbars containing the **shared chrome**:
   - section `nldd-tab-bar` (Bibliotheek/Editor) with `selected` derived from
     the current route, and `@click.prevent="router.push(...)"` targets
     computed from `sectionTarget` (unchanged behaviour, just relocated);
   - `TrajectMenu`;
   - the settings `nldd-menu` (flags / theme / auth).

   The shell owns the shared composables for this chrome (`useAuth`,
   `useColorScheme`, `useFeatureFlags`, `useTrajects`, `useLastVisitedRoute`).

   The shell renders a nested `<router-view />` into `slot="main"`, and renders
   the per-view toolbar extras itself, reading them from a shared chrome store
   (see "Injection mechanism" below).

2. **`EditorView.vue`** (renamed from `EditorApp.vue`) — loses its
   `nldd-app-view` / `bar-split-view` / `primary-bar` toolbars. Keeps its body
   (rendered through the shell's `<router-view>` into `slot="main"`). Its
   editor-specific toolbar/pane bits — the `nldd-document-tab-bar` and the
   "PR #N" write-back indicator — are published to the shell through the chrome
   store. Editor tab state (`openTabs` / `selectTab` / `closeTab` / `activeTab`)
   stays inside this component; only references (the open-tabs list, the active
   tab, and the four tab callbacks) are handed to the store.

3. **`LibraryView.vue`** (renamed from `LibraryApp.vue`) — same treatment; it has
   no editor-specific extras, so it only registers its `SearchPopover` with the
   store so the shell's search control can open it.

### Routing

Nested routes under the shell:

```
/ → AppShell
   ├── /library/:trajectRef?/:lawId?/:articleNumber?  → LibraryView  (default)
   └── /editor/:trajectRef?/:lawId?/:articleNumber?   → EditorView
```

Because both views are children of one `AppShell` route record, navigating
between them swaps only the nested `<router-view>`. The shell instance is
reused (not unmounted), so its chrome and shared state persist — no rebuild
flash.

The existing route paths, params, and redirects are preserved exactly, so all
deep links and the `sectionTarget`/`lastLibraryPath` logic keep working.

### Injection mechanism: shared chrome store (chosen)

The shell renders **all** toolbar items itself; the active view publishes its
per-view extras into a module-level reactive store (`useAppChrome.js`, matching
the existing `useColorScheme` / `useFeatureFlags` singletons):

- search trigger — each view registers its `SearchPopover` ref
  (`registerSearchPopover`); the shell's search button/field calls the store's
  `openSearch` / `onBarSearchKeydown`, which forward to the registered popover
  (the popover, with its differing `@select-law` handling, stays in each view).
- `EditorView` publishes the federated "PR #N" indicator (`lastSavedPr`) and the
  open document tabs + their callbacks (`setEditorChrome` / `registerTabActions`)
  and clears them on unmount (`clearEditorChrome`), so the library never shows a
  stale PR badge or tab bar. The shell renders the `document-tabs` pane only
  while the store reports open tabs.

**Why not `<Teleport>` (the earlier choice):** `nldd-toolbar` only projects
slotted items that are its *direct* children, and a teleport target placed
inside the toolbar breaks that projection; teleporting into the `nldd-toolbar`
host instead appends the items *after* the shell's own `slot="end"` items,
reordering the bar (search/PR would land after the settings menu). The store
lets the shell render every item in the original order, guaranteeing visual
parity, at the cost of handing a few editor references to the store — a small,
contained extraction rather than the "zero extraction" teleport promised.

### Data flow & state

- Shared chrome composables move to `AppShell`. They are module-level
  singletons today, so views that still reference them keep working.
- Section tab `selected` state and `libraryTabTarget` / `editorTabTarget` are
  computed in the shell from the current route (replacing the per-app copies).
- Body data fetching stays in each view (unchanged); a body remount on switch
  is expected and acceptable (URL-driven state).

### No additional CSS

The change reuses the exact existing `nldd-*` structure and markup; no new CSS
or design-system overrides are introduced. (If implementation reveals any
unavoidable custom CSS, it must be reported per project policy.)

## Testing

- **Regression smoke test (Playwright):** navigate to `/library`, set a
  `window` marker, click the Editor tab, assert the marker survives and the
  document was not reloaded (the same probe used during diagnosis), now with
  the shell mounted. Add the reverse direction.
- **Visual parity:** confirm both views render correctly at sm / md / lg
  breakpoints and the shared chrome (tab bar, settings menu, traject menu) is
  present and functional in both.
- Existing unit tests (`vitest`) for `apiAuthGuard`, composables, etc. must
  still pass.

## Out of scope / follow-ups

- `<keep-alive>` / body state preservation across switches.
- Any redesign of the toolbar or its contents.
- Deduplicating the three breakpoint toolbar variants further than what the
  shell consolidation already achieves (the breakpoint split is a design-system
  requirement).

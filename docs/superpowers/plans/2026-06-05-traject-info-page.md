# Traject-info Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only "Traject-info" sheet, opened from the traject dropdown, showing the active traject's create-screen fields plus its repo location (a new-tab link to the traject branch on GitHub) and branch details.

**Architecture:** A small `useTrajectDetail` composable fetches `GET /api/trajects/:id` (which already returns `sources`). A new `TrajectInfoDialog.vue` (cloned from the `TrajectMembersDialog.vue` sheet shell) renders the detail read-only, deriving the repo/branch from the `is_writable_own` source via two exported pure helpers. `TrajectMenu.vue` gets a `Traject-info…` menu item shown only when a traject is active.

**Tech Stack:** Vue 3 (`<script setup>`), Vite, Vitest + @vue/test-utils (happy-dom env), `@nldd/design-system` web components (`nldd-*`, treated as custom elements via `vite.config.js`).

**Working directory:** All paths below are relative to the worktree `/workspace/regelrecht/.worktrees/traject-info`. The worktree was created from `origin/main` on branch `feat/traject-info`. Run all `vitest`/`npx` commands with `--prefix frontend` semantics, i.e. from inside `frontend/` use `npx vitest run <path-relative-to-frontend>`.

**Backend:** No changes — `GET /api/trajects/:id` (`packages/editor-api/src/trajects.rs:645`) already returns `TrajectDetail` with `members`, `pending_invites`, and `sources` (each source carrying `gh_owner`, `gh_repo`, `gh_branch`, `gh_base_branch`, `gh_path`, `is_writable_own`).

---

## File Structure

- **Create:** `frontend/src/composables/useTrajectDetail.js` — fetches one traject's full detail by UUID id; exports the composable plus two pure helpers (`writableSource`, `branchTreeUrl`).
- **Create:** `frontend/src/composables/useTrajectDetail.test.js` — unit tests for the composable + helpers.
- **Create:** `frontend/src/components/TrajectInfoDialog.vue` — the read-only info sheet.
- **Create:** `frontend/src/components/TrajectInfoDialog.test.js` — component tests driving the sheet via mocked fetch.
- **Modify:** `frontend/src/components/TrajectMenu.vue` — add the `Traject-info…` menu item + open/close state for the info dialog.

---

## Task 1: `useTrajectDetail` composable + pure helpers

**Files:**
- Create: `frontend/src/composables/useTrajectDetail.js`
- Test: `frontend/src/composables/useTrajectDetail.test.js`

Mirrors `useTrajectMembers.js` (same `/api/trajects/:id` endpoint, same fresh-state-per-call shape) but keeps the whole `TrajectDetail` object and adds two pure helpers the dialog needs. The id passed in is the traject **UUID** (`activeTraject.value.id`), exactly like `TrajectMenu.openMembersForActive` already passes to the members dialog.

- [ ] **Step 1: Write the failing test**

Create `frontend/src/composables/useTrajectDetail.test.js`:

```js
import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  useTrajectDetail,
  writableSource,
  branchTreeUrl,
} from './useTrajectDetail.js';

// Minimal Response-like stub (mirrors useTrajectDocuments.test.js).
function res({ ok = true, status = 200, json = null }) {
  return {
    ok,
    status,
    async json() {
      return json;
    },
    async text() {
      return json ? JSON.stringify(json) : '';
    },
  };
}

const OWN_SOURCE = {
  source_id: 'own',
  name: 'eigen',
  source_type: 'github',
  gh_owner: 'MinBZK',
  gh_repo: 'regelrecht-corpus',
  gh_branch: 'traject/tariefswijziging-2026',
  gh_base_branch: 'development',
  gh_path: 'regulation/nl',
  gh_ref: null,
  local_path: null,
  priority: 0,
  auth_ref: null,
  is_writable_own: true,
};
const READONLY_SOURCE = { ...OWN_SOURCE, source_id: 'ro', is_writable_own: false };

const DETAIL = {
  id: '11111111-2222-3333-4444-555566667777',
  name: 'Tariefswijziging 2026',
  description: 'Waarom',
  scope: 'zorgtoeslag',
  status: 'bezig',
  role: 'owner',
  ref: 'tariefswijziging-2026-11111111',
  members: [],
  pending_invites: [],
  sources: [READONLY_SOURCE, OWN_SOURCE],
};

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('writableSource', () => {
  it('returns the source flagged is_writable_own', () => {
    expect(writableSource(DETAIL)?.source_id).toBe('own');
  });

  it('returns null when there is no detail or no writable source', () => {
    expect(writableSource(null)).toBe(null);
    expect(writableSource({ sources: [READONLY_SOURCE] })).toBe(null);
    expect(writableSource({})).toBe(null);
  });
});

describe('branchTreeUrl', () => {
  it('builds a github tree URL for the branch, slashes preserved', () => {
    expect(branchTreeUrl(OWN_SOURCE)).toBe(
      'https://github.com/MinBZK/regelrecht-corpus/tree/traject/tariefswijziging-2026',
    );
  });

  it('returns null for a non-github or incomplete source', () => {
    expect(branchTreeUrl(null)).toBe(null);
    expect(branchTreeUrl({ gh_owner: 'x', gh_repo: null, gh_branch: 'b' })).toBe(null);
    expect(branchTreeUrl({ gh_owner: 'x', gh_repo: 'y', gh_branch: null })).toBe(null);
  });
});

describe('useTrajectDetail', () => {
  it('fetches the detail by id and exposes it reactively', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(res({ json: DETAIL }));
    globalThis.fetch = fetchSpy;

    const { detail, loading, error, load } = useTrajectDetail();
    expect(loading.value).toBe(false);
    await load(DETAIL.id);

    expect(fetchSpy).toHaveBeenCalledWith(
      '/api/trajects/11111111-2222-3333-4444-555566667777',
    );
    expect(detail.value.name).toBe('Tariefswijziging 2026');
    expect(error.value).toBe(null);
    expect(loading.value).toBe(false);
  });

  it('resets state before loading so a reopen cannot flash stale data', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res({ json: DETAIL }));
    const { detail, load } = useTrajectDetail();
    await load(DETAIL.id);
    expect(detail.value).not.toBe(null);

    // Second load against a slow/failed fetch must have cleared detail first.
    let resolveFetch;
    globalThis.fetch = vi.fn().mockReturnValue(
      new Promise((r) => {
        resolveFetch = r;
      }),
    );
    const p = load(DETAIL.id);
    expect(detail.value).toBe(null); // cleared synchronously before await
    resolveFetch(res({ json: DETAIL }));
    await p;
  });

  it('records an error on a non-ok response and leaves detail null', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res({ ok: false, status: 404 }));
    const { detail, error, load } = useTrajectDetail();
    await load(DETAIL.id);
    expect(detail.value).toBe(null);
    expect(error.value).toBeInstanceOf(Error);
    expect(error.value.message).toMatch(/404/);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd frontend && npx vitest run src/composables/useTrajectDetail.test.js`
Expected: FAIL — `Failed to resolve import './useTrajectDetail.js'` (module does not exist yet).

- [ ] **Step 3: Write the implementation**

Create `frontend/src/composables/useTrajectDetail.js`:

```js
/**
 * useTrajectDetail — fetch one traject's full detail (`GET /api/trajects/:id`),
 * including its `sources`, for the read-only Traject-info sheet.
 *
 * Same endpoint and fresh-state-per-call shape as `useTrajectMembers`, but
 * keeps the whole `TrajectDetail` object (that composable discards `sources`).
 * The `id` argument is the traject **UUID** — the same value
 * `TrajectMenu.openMembersForActive` passes for member management — not the
 * URL `ref` form.
 */
import { ref } from 'vue';

/**
 * Pick the writable-own source from a `TrajectDetail`. That is the source the
 * traject pushes edits to, so its repo/branch fields are what the info sheet
 * shows. Returns `null` when there is no detail or no writable source (a
 * defensive shape the backend shouldn't produce, but the UI must not crash on).
 */
export function writableSource(detail) {
  if (!detail || !Array.isArray(detail.sources)) return null;
  return detail.sources.find((s) => s.is_writable_own) ?? null;
}

/**
 * Build a GitHub tree URL pointing at the traject branch:
 * `https://github.com/{owner}/{repo}/tree/{branch}`. Slashes inside the branch
 * name are left intact (GitHub serves `tree/feature/x` directly; percent-
 * encoding the `/` would break it). Returns `null` when the source is missing
 * owner/repo/branch, so the caller can render plain text instead of a dead link.
 */
export function branchTreeUrl(source) {
  if (!source) return null;
  const { gh_owner, gh_repo, gh_branch } = source;
  if (!gh_owner || !gh_repo || !gh_branch) return null;
  return `https://github.com/${gh_owner}/${gh_repo}/tree/${gh_branch}`;
}

export function useTrajectDetail() {
  const detail = ref(null);
  const loading = ref(false);
  const error = ref(null);

  async function load(trajectId) {
    // Reset before the await so a reopen against a different traject can't
    // briefly flash the previous traject's data (mirrors useTrajectMembers).
    loading.value = true;
    error.value = null;
    detail.value = null;
    try {
      const resp = await fetch(`/api/trajects/${trajectId}`);
      if (!resp.ok) {
        throw new Error(`Kon traject niet laden: ${resp.status}`);
      }
      detail.value = await resp.json();
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  return { detail, loading, error, load };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd frontend && npx vitest run src/composables/useTrajectDetail.test.js`
Expected: PASS — all 7 tests green.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/composables/useTrajectDetail.js frontend/src/composables/useTrajectDetail.test.js
git commit -m "feat(frontend): useTrajectDetail composable for traject-info sheet"
```

---

## Task 2: `TrajectInfoDialog.vue` read-only sheet

**Files:**
- Create: `frontend/src/components/TrajectInfoDialog.vue`
- Test: `frontend/src/components/TrajectInfoDialog.test.js`

Clones the sheet shell of `TrajectMembersDialog.vue` (Teleport → `nldd-sheet` right/520px/full-height → `nldd-page` → `nldd-top-title-bar`). Props `modelValue`/`trajectId`/`trajectName`, same `watch(modelValue)` → `load()` + `show()/hide()` pattern. Body shows the create-screen fields read-only plus repo/branch.

- [ ] **Step 1: Write the failing test**

Create `frontend/src/components/TrajectInfoDialog.test.js`:

```js
import { describe, it, expect, beforeAll, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import TrajectInfoDialog from './TrajectInfoDialog.vue';

// nldd-* tags compile to raw HTML (vite.config.js isCustomElement), so the
// `nldd-sheet` template ref is a real HTMLElement with no show()/hide().
// Register a no-op stub so the modelValue watcher doesn't throw on mount.
// (Same approach as EditSheet.test.js.)
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('nldd-sheet')) {
    class NddSheetTestStub extends HTMLElement {
      show() {}
      hide() {}
    }
    customElements.define('nldd-sheet', NddSheetTestStub);
  }
});

const OWN_SOURCE = {
  source_id: 'own',
  source_type: 'github',
  gh_owner: 'MinBZK',
  gh_repo: 'regelrecht-corpus',
  gh_branch: 'traject/tariefswijziging-2026',
  gh_base_branch: 'development',
  gh_path: 'regulation/nl',
  is_writable_own: true,
};

const DETAIL = {
  id: 'abc',
  name: 'Tariefswijziging 2026',
  description: 'Waarom dit traject',
  scope: 'zorgtoeslag',
  status: 'bezig',
  role: 'owner',
  members: [],
  pending_invites: [],
  sources: [OWN_SOURCE],
};

function res(json, ok = true, status = 200) {
  return { ok, status, async json() { return json; }, async text() { return ''; } };
}

beforeEach(() => {
  vi.restoreAllMocks();
});

function mountDialog() {
  return mount(TrajectInfoDialog, {
    attachTo: document.body,
    props: { modelValue: false, trajectId: 'abc', trajectName: 'Tariefswijziging 2026' },
  });
}

describe('TrajectInfoDialog', () => {
  it('loads detail when opened and renders the create-screen fields', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(DETAIL));
    const wrapper = mountDialog();

    await wrapper.setProps({ modelValue: true });
    await nextTick();
    await nextTick(); // let load() settle

    expect(globalThis.fetch).toHaveBeenCalledWith('/api/trajects/abc');
    const text = wrapper.text();
    expect(text).toContain('Tariefswijziging 2026');
    expect(text).toContain('Waarom dit traject');
    expect(text).toContain('zorgtoeslag');
    expect(text).toContain('bezig');
    expect(text).toContain('owner');
  });

  it('renders the repo as a new-tab nldd-link to the traject branch', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(DETAIL));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await nextTick();
    await nextTick();

    // nldd-link compiles to a raw custom element in the test env (vite
    // isCustomElement), so assert on its attributes — the underlying <a>,
    // slot text, and auto-rel only exist once the real Lit component
    // upgrades in the browser. We bind href/target/text/rel explicitly so
    // they are present as attributes here.
    const link = wrapper.get('nldd-link.traject-info-repo-link');
    expect(link.attributes('href')).toBe(
      'https://github.com/MinBZK/regelrecht-corpus/tree/traject/tariefswijziging-2026',
    );
    expect(link.attributes('target')).toBe('_blank');
    expect(link.attributes('rel')).toContain('noopener');
    expect(link.attributes('text')).toBe('MinBZK/regelrecht-corpus');
  });

  it('shows the branch, base branch and subpath', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(DETAIL));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await nextTick();
    await nextTick();

    const text = wrapper.text();
    expect(text).toContain('traject/tariefswijziging-2026'); // branch
    expect(text).toContain('development'); // base branch
    expect(text).toContain('regulation/nl'); // subpath
  });

  it('falls back to "repo-root" when the subpath is empty', async () => {
    const detail = { ...DETAIL, sources: [{ ...OWN_SOURCE, gh_path: '' }] };
    globalThis.fetch = vi.fn().mockResolvedValue(res(detail));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await nextTick();
    await nextTick();

    expect(wrapper.text()).toContain('repo-root');
  });

  it('shows an error message when the load fails', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(null, false, 404));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await nextTick();
    await nextTick();

    expect(wrapper.text()).toMatch(/niet laden|404/i);
  });

  it('emits update:modelValue=false when dismissed', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(DETAIL));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await nextTick();

    wrapper.vm.close();
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd frontend && npx vitest run src/components/TrajectInfoDialog.test.js`
Expected: FAIL — `Failed to resolve import './TrajectInfoDialog.vue'`.

- [ ] **Step 3: Write the implementation**

Create `frontend/src/components/TrajectInfoDialog.vue`:

```vue
<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import {
  useTrajectDetail,
  writableSource,
  branchTreeUrl,
} from '../composables/useTrajectDetail.js';

const props = defineProps({
  /** Whether the sheet is currently open. */
  modelValue: { type: Boolean, default: false },
  /** Traject to show (UUID id, same value the members dialog takes). */
  trajectId: { type: String, default: null },
  /** Traject display name, for the sheet header. */
  trajectName: { type: String, default: '' },
});

const emit = defineEmits(['update:modelValue']);

const sheetEl = ref(null);
const { detail, loading, error: loadError, load } = useTrajectDetail();

// Repo/branch come from the writable-own source; null-safe so an unexpected
// shape renders "onbekend" instead of crashing.
const source = computed(() => writableSource(detail.value));
const repoLabel = computed(() =>
  source.value ? `${source.value.gh_owner}/${source.value.gh_repo}` : null,
);
const repoUrl = computed(() => branchTreeUrl(source.value));
const subpath = computed(() => {
  const p = source.value?.gh_path;
  return p && p.trim() ? p : 'repo-root';
});

// dash for empty optional text fields.
function orDash(v) {
  return v && String(v).trim() ? v : '—';
}

watch(
  () => props.modelValue,
  async (v) => {
    await nextTick();
    if (v) {
      if (props.trajectId) await load(props.trajectId);
      sheetEl.value?.show();
    } else {
      sheetEl.value?.hide();
    }
  },
);

function close() {
  emit('update:modelValue', false);
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet
      ref="sheetEl"
      placement="right"
      width="520px"
      full-height
      @close="close"
    >
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar
          slot="header"
          :text="`Traject-info — ${trajectName}`"
          dismiss-text="Sluit"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section v-if="loading">
          <p class="traject-info-status">Laden…</p>
        </nldd-simple-section>

        <nldd-simple-section v-else-if="loadError">
          <p class="traject-info-error">
            {{ loadError.message || 'Fout bij laden' }}
          </p>
        </nldd-simple-section>

        <template v-else-if="detail">
          <nldd-simple-section heading="Gegevens">
            <nldd-list variant="box" class="traject-info-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ detail.name }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Beschrijving" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ orDash(detail.description) }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Scope" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ orDash(detail.scope) }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Status" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ detail.status }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Jouw rol" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ detail.role }}</span></nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-simple-section>

          <nldd-simple-section heading="Repository">
            <nldd-list variant="box" class="traject-info-list">
              <nldd-list-item size="md">
                <nldd-text-cell
                  text="Repo"
                  supporting-text="Opent de traject-branch op GitHub in een nieuw tabblad."
                  max-width="180px"
                ></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <!-- nldd-link is the design-system link component. It
                       auto-sets rel='noopener noreferrer' for target='_blank',
                       but we also pass rel explicitly so it is present even
                       before the Lit component upgrades (and is unit-testable).
                       end-icon hints the link leaves the app. -->
                  <nldd-link
                    v-if="repoUrl"
                    class="traject-info-repo-link"
                    size="md"
                    :href="repoUrl"
                    target="_blank"
                    rel="noopener noreferrer"
                    end-icon="external-link"
                    :text="repoLabel"
                  ></nldd-link>
                  <span v-else class="traject-info-value">{{ repoLabel || 'onbekend' }}</span>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Branch" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ source?.gh_branch || 'onbekend' }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Base branch" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ source?.gh_base_branch || 'onbekend' }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Subpath" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ subpath }}</span></nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-simple-section>
        </template>

        <nldd-container slot="footer" padding="16">
          <nldd-button
            variant="ghost"
            size="md"
            full-width
            text="Sluiten"
            @click="close"
          ></nldd-button>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
.traject-info-list nldd-cell {
  flex: 1;
  min-width: 0;
}
.traject-info-value {
  font-size: 14px;
  word-break: break-word;
}
.traject-info-status {
  font-size: 13px;
  color: var(--semantics-content-secondary-color, #555);
  margin: 8px 0;
}
.traject-info-error {
  color: var(--nldd-color-text-error, #c62828);
  font-size: 13px;
  margin-top: 8px;
}
.traject-info-repo-link {
  word-break: break-word;
}
</style>
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd frontend && npx vitest run src/components/TrajectInfoDialog.test.js`
Expected: PASS — all 6 tests green.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/TrajectInfoDialog.vue frontend/src/components/TrajectInfoDialog.test.js
git commit -m "feat(frontend): read-only TrajectInfoDialog sheet"
```

---

## Task 3: Wire `Traject-info…` into the dropdown

**Files:**
- Modify: `frontend/src/components/TrajectMenu.vue`

Add an `info` menu item (visible only with an active traject) above `Beheer leden…`, plus the open/close state and the `<TrajectInfoDialog>` instance. This mirrors the existing members wiring exactly.

- [ ] **Step 1: Add the import**

In `frontend/src/components/TrajectMenu.vue`, after the existing `TrajectMembersDialog` import (line 5), add:

```js
import TrajectInfoDialog from './TrajectInfoDialog.vue';
```

So the two import lines read:

```js
import TrajectMembersDialog from './TrajectMembersDialog.vue';
import TrajectInfoDialog from './TrajectInfoDialog.vue';
```

- [ ] **Step 2: Add info-dialog state + opener**

In the same file, immediately after the members-dialog state block (the `openMembersForActive` function ends at line 121, just before `function closeCreate()`), insert:

```js
// --- Info dialog state ---
const showInfo = ref(false);
const infoTrajectId = ref(null);
const infoTrajectName = ref('');

function openInfoForActive() {
  if (!activeTraject.value) return;
  infoTrajectId.value = activeTraject.value.id;
  infoTrajectName.value = activeTraject.value.name;
  showInfo.value = true;
}
```

- [ ] **Step 3: Add the menu item**

In the template, inside `<nldd-menu>`, add the info item directly before the existing `Beheer leden…` item (currently at lines 244-249). The block becomes:

```html
    <nldd-menu-divider></nldd-menu-divider>
    <nldd-menu-item
      v-if="activeTraject"
      text="Traject-info…"
      start-icon="info"
      @click="openInfoForActive"
    ></nldd-menu-item>
    <nldd-menu-item
      v-if="activeTraject"
      text="Beheer leden…"
      start-icon="users"
      @click="openMembersForActive"
    ></nldd-menu-item>
```

- [ ] **Step 4: Render the dialog**

Directly after the existing `<TrajectMembersDialog ... />` element (lines 257-261), add:

```html
  <TrajectInfoDialog
    v-model="showInfo"
    :traject-id="infoTrajectId"
    :traject-name="infoTrajectName"
  />
```

- [ ] **Step 5: Run the full frontend test suite to verify nothing broke**

Run: `cd frontend && npx vitest run`
Expected: PASS — the existing suite stays green and the two new test files (Task 1 + Task 2) pass. No test imports `TrajectMenu.vue` directly, so this step is a regression guard.

- [ ] **Step 6: Build to verify the SFC compiles**

Run: `cd frontend && npx vite build`
Expected: build completes with no Vue compiler errors referencing `TrajectMenu.vue` or `TrajectInfoDialog.vue`.

- [ ] **Step 7: Commit**

```bash
git add frontend/src/components/TrajectMenu.vue
git commit -m "feat(frontend): open Traject-info sheet from the traject dropdown"
```

---

## Manual verification (after Task 3)

Run the editor locally (or against a preview) and confirm:

1. With no active traject, the dropdown shows **no** `Traject-info…` item.
2. Select a traject → open the dropdown → `Traject-info…` appears above `Beheer leden…`.
3. Click it → a right-side sheet (same look as "Nieuw traject") opens with Naam/Beschrijving/Scope/Status/Jouw rol and a Repository section.
4. The **Repo** value is a link; clicking it opens `github.com/<owner>/<repo>/tree/<branch>` in a **new tab**.
5. Branch / Base branch / Subpath show the traject's source values (Subpath shows `repo-root` when empty).
6. "Sluit"/"Sluiten" closes the sheet.

---

## Self-review notes

- **Spec coverage:** create-screen fields (Naam/Beschrijving/Scope) → Task 2 "Gegevens" section; repo location + traject branch → Task 2 "Repository" section; repo link new tab to traject branch → `branchTreeUrl` (Task 1) + `<a target="_blank">` (Task 2); dropdown link, active traject only → Task 3 `v-if="activeTraject"`; same sheet component → cloned shell in Task 2. Status + Jouw rol included per approved design.
- **Type consistency:** `writableSource`/`branchTreeUrl`/`useTrajectDetail`/`load` names match across Tasks 1–2. The dialog props `modelValue`/`trajectId`/`trajectName` match the members-dialog contract Task 3 wires to.
- **Design-system compliance:** the repo link uses the design-system `nldd-link` component (`href`/`target="_blank"`/`rel`/`text`/`end-icon="external-link"`), not a raw `<a>` — `external-link` and `info` are confirmed-valid NDD icon names already used elsewhere in the app. No design-system shortcut taken.
- **Custom CSS to report to the user:** `.traject-info-value`, `.traject-info-repo-link`, `.traject-info-status`, `.traject-info-error` (font-size/word-break/colour for the read-only value cells; `flex:1;min-width:0` on `nldd-cell` to let values fill the row). These reuse the same NDD CSS variables and the exact `.traject-form-list nldd-cell` pattern already in `TrajectMenu.vue`; no new layout hacks. Report this list to the user at PR time per the CSS-reporting rule.

/**
 * useFeatureFlags — singleton feature flag store with API sync.
 *
 * Fetches flags from /api/feature-flags on first use, falls back to
 * hardcoded defaults when the API is unavailable (e.g. no database).
 */
import { ref, readonly } from 'vue';
import { apiFetch, apiFetchJson } from '../lib/apiFetch.js';

const DEFAULTS = {
  'panel.article_text': true,
  'panel.scenario_form': true,
  'panel.yaml_editor': true,
  'panel.machine_readable': true,
  // "Tekst viewer + notities" pane (RFC-005/RFC-018): a read-only article-text
  // view with resolved note highlights + inline note authoring, separate from
  // the editable Tekst editor. Off by default until the feature is past the
  // display-only MVP and the corpus has notes for more than one law.
  'panel.notes': false,
};

// Local overrides survive refresh when the backend has no persistence (dev).
const STORAGE_KEY = 'regelrecht-feature-flags';

function loadLocal() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved ? JSON.parse(saved) : {};
  } catch { return {}; }
}

function saveLocal(flagMap) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(flagMap)); } catch { /* ignore */ }
}

const flags = ref({ ...DEFAULTS, ...loadLocal() });
const loaded = ref(false);

let fetchPromise = null;

async function loadFlags() {
  if (fetchPromise) return fetchPromise;
  fetchPromise = (async () => {
    try {
      const server = await apiFetchJson('/api/feature-flags', {
        errorMessage: (status) => `HTTP ${status}`,
      });
      // Server values are the base; local overrides win so user toggles
      // survive refreshes when the backend can't persist (503 path below).
      flags.value = { ...DEFAULTS, ...server, ...loadLocal() };
    } catch (e) {
      console.warn('Failed to load feature flags, using defaults:', e.message);
      flags.value = { ...DEFAULTS, ...loadLocal() };
    } finally {
      loaded.value = true;
    }
  })();
  return fetchPromise;
}

async function toggle(key) {
  const current = flags.value[key] ?? DEFAULTS[key] ?? true;
  const newValue = !current;

  // Optimistic update, persisted locally so the toggle survives refreshes
  // whenever the backend can't accept the write (503 no-DB, or a failed write
  // handled in the catch below).
  flags.value = { ...flags.value, [key]: newValue };

  try {
    const res = await apiFetch(`/api/feature-flags/${encodeURIComponent(key)}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: newValue }),
      allowStatuses: [503],
      errorMessage: (status) => `HTTP ${status}`,
    });
    if (res.status === 503) {
      // Backend has no DB; keep the change local so it survives refresh.
      saveLocal({ ...loadLocal(), [key]: newValue });
      return;
    }
    // Clear any stale local override for this key — server is now authoritative.
    const local = loadLocal();
    if (key in local) {
      delete local[key];
      saveLocal(local);
    }
    const updated = await res.json();
    flags.value = { ...DEFAULTS, ...updated, ...loadLocal() };
  } catch (e) {
    // The write was rejected or unreachable — e.g. a 401 in OIDC-off dev (flag
    // writes need auth, the dev session has none), offline, or a 5xx. Keep the
    // toggle locally instead of reverting, so panes stay switchable client-side.
    // Same localStorage override the 503 path uses; a later successful write
    // clears it and the server becomes authoritative again.
    console.warn('Feature flag write failed; keeping change locally:', e.message);
    saveLocal({ ...loadLocal(), [key]: newValue });
  }
}

export function useFeatureFlags() {
  if (!loaded.value && !fetchPromise) {
    loadFlags();
  }
  return {
    flags: readonly(flags),
    loaded: readonly(loaded),
    isEnabled: (key) => flags.value[key] ?? DEFAULTS[key] ?? true,
    toggle,
  };
}

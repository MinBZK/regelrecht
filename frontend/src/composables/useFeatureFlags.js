/**
 * useFeatureFlags — singleton feature flag store with API sync.
 *
 * Fetches flags from /api/feature-flags on first use, falls back to
 * hardcoded defaults when the API is unavailable (e.g. no database).
 */
import { ref, readonly } from 'vue';

const DEFAULTS = {
  'panel.article_text': true,
  'panel.scenario_form': true,
  'panel.yaml_editor': true,
  'panel.machine_readable': true,
  'panel.law_graph': false,
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
      const res = await fetch('/api/feature-flags');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      // Server values are the base; local overrides win so user toggles
      // survive refreshes when the backend can't persist (503 path below).
      flags.value = { ...DEFAULTS, ...(await res.json()), ...loadLocal() };
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

  // Optimistic update. Persist locally too so the toggle survives refreshes
  // when the backend explicitly can't persist (503, no DB configured in dev).
  flags.value = { ...flags.value, [key]: newValue };

  try {
    const res = await fetch(`/api/feature-flags/${encodeURIComponent(key)}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: newValue }),
    });
    if (res.status === 503) {
      // Backend has no DB; keep the change local so it survives refresh.
      saveLocal({ ...loadLocal(), [key]: newValue });
      return;
    }
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    // Clear any stale local override for this key — server is now authoritative.
    const local = loadLocal();
    if (key in local) {
      delete local[key];
      saveLocal(local);
    }
    const updated = await res.json();
    flags.value = { ...DEFAULTS, ...updated, ...loadLocal() };
  } catch (e) {
    console.warn('Failed to update feature flag, reverting:', e.message);
    flags.value = { ...flags.value, [key]: current };
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

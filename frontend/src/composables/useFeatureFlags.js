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
  'panel.execution_trace': true,
  'panel.machine_readable': true,
};

const flags = ref({ ...DEFAULTS });
const loaded = ref(false);

let fetchPromise = null;

async function loadFlags() {
  if (fetchPromise) return fetchPromise;
  fetchPromise = (async () => {
    try {
      const res = await fetch('/api/feature-flags');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      flags.value = { ...DEFAULTS, ...(await res.json()) };
    } catch (e) {
      console.warn('Failed to load feature flags, using defaults:', e.message);
      flags.value = { ...DEFAULTS };
    } finally {
      loaded.value = true;
    }
  })();
  return fetchPromise;
}

async function toggle(key) {
  const current = flags.value[key] ?? DEFAULTS[key] ?? true;
  const newValue = !current;

  // Optimistic update
  flags.value = { ...flags.value, [key]: newValue };

  try {
    const res = await fetch(`/api/feature-flags/${encodeURIComponent(key)}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: newValue }),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const updated = await res.json();
    flags.value = { ...DEFAULTS, ...updated };
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

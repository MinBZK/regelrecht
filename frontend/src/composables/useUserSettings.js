/**
 * useUserSettings — singleton per-user settings store with API sync.
 *
 * Fetches settings from /api/user/settings on first use and merges them on
 * top of client-side defaults. Falls back to defaults on any failure (401,
 * network, missing DB) so a brand-new or anonymous user always sees a
 * usable editor.
 *
 * Theme is applied to <html> as `data-scheme`, which is the attribute
 * `@minbzk/storybook` (NDD design system) keys its dark tokens on —
 * it also sets `color-scheme` from that selector, so UA form controls
 * follow automatically.
 */
import { ref, readonly, computed, watchEffect } from 'vue';

const systemDark = typeof window !== 'undefined'
  && window.matchMedia
  && window.matchMedia('(prefers-color-scheme: dark)').matches;

const DEFAULTS = {
  theme: systemDark ? 'dark' : 'light',
};

const settings = ref({ ...DEFAULTS });
const loaded = ref(false);

let fetchPromise = null;

async function loadSettings() {
  if (fetchPromise) return fetchPromise;
  fetchPromise = (async () => {
    try {
      const res = await fetch('/api/user/settings');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      settings.value = { ...DEFAULTS, ...data };
    } catch (e) {
      // 401 (auth off), 503 (no DB) and network errors all collapse to the
      // same outcome: use defaults and keep the editor loading.
      console.warn('Falling back to default user settings:', e.message);
      settings.value = { ...DEFAULTS };
    } finally {
      loaded.value = true;
    }
  })();
  return fetchPromise;
}

async function setSetting(key, value) {
  const prev = settings.value[key];
  settings.value = { ...settings.value, [key]: value };

  try {
    const res = await fetch(`/api/user/settings/${encodeURIComponent(key)}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value }),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
  } catch (e) {
    console.warn('Revert user setting after failed PUT:', e.message);
    settings.value = { ...settings.value, [key]: prev };
  }
}

// Apply theme to <html>. Runs before the fetch completes too, using DEFAULTS.
watchEffect(() => {
  if (typeof document === 'undefined') return;
  const t = settings.value.theme || DEFAULTS.theme;
  document.documentElement.setAttribute('data-scheme', t);
});

export function useUserSettings() {
  if (!loaded.value && !fetchPromise) {
    loadSettings();
  }
  return {
    settings: readonly(settings),
    loaded: readonly(loaded),
    theme: computed(() => settings.value.theme),
    setTheme: (v) => setSetting('theme', v),
    toggleTheme: () =>
      setSetting('theme', settings.value.theme === 'dark' ? 'light' : 'dark'),
  };
}

/**
 * useUserSettings — singleton per-user settings store with API sync.
 *
 * Fetches settings from /api/user/settings on first use and merges them on
 * top of client-side defaults. Falls back to defaults on any failure (401,
 * network, missing DB) so a brand-new or anonymous user always sees a
 * usable editor.
 *
 * Theme values: 'auto' | 'light' | 'dark'. Applied to <html> as `data-scheme`
 * (the attribute `@nldd/design-system` keys its dark tokens on); 'auto' is
 * encoded as the *absence* of the attribute so OS-level prefers-color-scheme
 * takes over.
 */
import { ref, readonly, computed, watchEffect } from 'vue';

const VALID_THEMES = ['auto', 'light', 'dark'];

const DEFAULTS = {
  theme: 'auto',
};

// Cache the theme in localStorage so a returning user sees the right palette
// immediately on next page load — without this the page mounts with the
// prefers-color-scheme default and flips after the /api/user/settings fetch
// resolves, which is a visible whole-page flicker. The server remains the
// source of truth: a successful fetch always overwrites the cached value.
const THEME_STORAGE_KEY = 'rr-user-settings-theme';

function readCachedTheme() {
  try {
    const v = window.localStorage?.getItem(THEME_STORAGE_KEY);
    return VALID_THEMES.includes(v) ? v : null;
  } catch {
    return null;
  }
}

function writeCachedTheme(value) {
  try {
    window.localStorage?.setItem(THEME_STORAGE_KEY, value);
  } catch {
    // Ignore storage errors (private mode, quota, disabled) — flicker on
    // next load is the only consequence.
  }
}

const cachedTheme = typeof window !== 'undefined' ? readCachedTheme() : null;

const settings = ref({
  ...DEFAULTS,
  ...(cachedTheme ? { theme: cachedTheme } : {}),
});
const loaded = ref(false);

// Keys the user has touched locally before `loadSettings` resolved. The
// initial fetch must NOT overwrite these — otherwise a toggle clicked
// during the fetch latency window would briefly flip back to the stale
// server value while the PUT is still in flight.
const dirtyKeys = new Set();

let fetchPromise = null;

async function loadSettings() {
  if (fetchPromise) return fetchPromise;
  fetchPromise = (async () => {
    try {
      const res = await fetch('/api/user/settings');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      // Precedence: server > current settings (cached + user toggles) >
      // DEFAULTS. Spreading settings.value before data prevents an empty
      // `{}` response from overwriting a cached theme on a returning user
      // whose server row was never written — same flicker the cache
      // exists to prevent.
      const merged = { ...DEFAULTS, ...settings.value, ...data };
      // Preserve values the user already set locally during this fetch.
      for (const k of dirtyKeys) merged[k] = settings.value[k];
      settings.value = merged;
      if (!dirtyKeys.has('theme') && VALID_THEMES.includes(data.theme)) {
        writeCachedTheme(data.theme);
      }
    } catch (e) {
      // 401 (auth off), 503 (no DB) and network errors all collapse to
      // the same outcome: keep the editor loading on whatever's already
      // in settings.value (cachedTheme + DEFAULTS + any user toggles).
      // We have nothing better to merge — and resetting to DEFAULTS would
      // re-introduce the flicker the localStorage cache exists to avoid.
      console.warn('Keeping cached/default user settings:', e.message);
    } finally {
      loaded.value = true;
    }
  })();
  return fetchPromise;
}

async function setSetting(key, value) {
  dirtyKeys.add(key);
  settings.value = { ...settings.value, [key]: value };
  if (key === 'theme') writeCachedTheme(value);

  try {
    const res = await fetch(`/api/user/settings/${encodeURIComponent(key)}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value }),
    });
    if (!res.ok) {
      // 401 (anonymous), 503 (no DB), 5xx all collapse to "local-only mode":
      // keep the value in settings.value + localStorage so the picker remains
      // functional. Do not revert — the documented contract is that anonymous
      // users still get a working picker via the localStorage cache.
      console.warn(`User setting PUT not persisted (HTTP ${res.status})`);
    }
  } catch (e) {
    console.warn('User setting PUT failed (network error):', e.message);
  } finally {
    // Once the PUT has settled, the key is no longer "in flight". Keeping
    // it in dirtyKeys would suppress the server value if loadSettings ever
    // became re-entrant.
    dirtyKeys.delete(key);
  }
}

// Apply theme to <html>. Runs before the fetch completes too, using DEFAULTS.
// 'auto' is encoded as the absence of the attribute so the design-system's
// `@media (prefers-color-scheme: dark)` selector takes over.
watchEffect(() => {
  if (typeof document === 'undefined') return;
  const t = settings.value.theme || DEFAULTS.theme;
  const root = document.documentElement;
  if (t === 'auto') root.removeAttribute('data-scheme');
  else root.setAttribute('data-scheme', t);
});

export function useUserSettings() {
  if (!loaded.value && !fetchPromise) {
    loadSettings();
  }
  return {
    settings: readonly(settings),
    loaded: readonly(loaded),
    theme: computed(() => settings.value.theme),
    setTheme: (v) => {
      if (VALID_THEMES.includes(v)) setSetting('theme', v);
    },
  };
}

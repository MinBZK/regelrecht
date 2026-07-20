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
import { apiFetch, apiFetchJson, ApiError } from '../lib/apiFetch.js';

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
      const data = await apiFetchJson('/api/user/settings', {
        errorMessage: (status) => `HTTP ${status}`,
      });
      // Drop server-supplied values that don't pass client-side validation
      // before they reach `settings.value`. The backend validates writes,
      // but a stale row predating tightened validation could still produce
      // an invalid `theme`, which the watchEffect would otherwise apply as
      // `data-scheme=<garbage>` until the next user action.
      const sanitized = { ...data };
      if (sanitized.theme !== undefined && !VALID_THEMES.includes(sanitized.theme)) {
        delete sanitized.theme;
      }
      // Precedence: server > current settings (cached + user toggles) >
      // DEFAULTS. Spreading settings.value before data prevents an empty
      // `{}` response from overwriting a cached theme on a returning user
      // whose server row was never written — same flicker the cache
      // exists to prevent.
      const merged = { ...DEFAULTS, ...settings.value, ...sanitized };
      // Preserve values the user already set locally during this fetch.
      for (const k of dirtyKeys) merged[k] = settings.value[k];
      settings.value = merged;
      if (!dirtyKeys.has('theme') && VALID_THEMES.includes(sanitized.theme)) {
        writeCachedTheme(sanitized.theme);
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
    await apiFetch(`/api/user/settings/${encodeURIComponent(key)}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value }),
    });
  } catch (e) {
    // 401 (anonymous), 503 (no DB), 5xx all collapse to "local-only mode":
    // keep the value in settings.value + localStorage so the picker remains
    // functional. Do not revert — the documented contract is that anonymous
    // users still get a working picker via the localStorage cache.
    if (e instanceof ApiError) {
      console.warn(`User setting PUT not persisted (HTTP ${e.status})`);
    } else {
      console.warn('User setting PUT failed (network error):', e.message);
    }
  } finally {
    // Hold the dirty guard until the initial GET has merged. If the PUT
    // resolves before the GET, dropping the guard here would let the GET's
    // (possibly pre-PUT) snapshot clobber the user's just-confirmed value
    // when `loadSettings` reads `dirtyKeys`. After initial load the await
    // is free — `fetchPromise` is already resolved.
    if (fetchPromise) {
      try { await fetchPromise; } catch { /* GET error already handled */ }
    }
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

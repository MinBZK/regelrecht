/**
 * useColorScheme — shared color-scheme picker for the regelrecht frontends.
 *
 * Owns the single source of truth for applying the chosen theme to
 * `<html data-scheme>` (the attribute `@nldd/design-system` keys its dark
 * tokens on) and for validating theme values. 'auto' is encoded as the
 * *absence* of the attribute so the OS-level `prefers-color-scheme` media
 * query takes over.
 *
 * Persistence is injected so each app can supply the right backend without
 * this module knowing about app-specific endpoints:
 *   - the editor injects its server-backed `useUserSettings` adapter
 *     (`{ theme, setTheme }`), so the picker syncs across devices;
 *   - admin and lawmaking have no settings endpoint, so they use the default
 *     localStorage adapter.
 *
 * A persistence adapter is `{ theme: Ref<string>, setTheme(value): void }`.
 */
import { ref, readonly, watch } from 'vue';

export const VALID_THEMES = ['auto', 'light', 'dark'];

/**
 * Apply a theme to `<html>`. 'auto' (and any invalid value) removes the
 * attribute so `@media (prefers-color-scheme: dark)` drives the palette.
 */
export function applyColorScheme(theme) {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  if (theme === 'auto' || !VALID_THEMES.includes(theme)) {
    root.removeAttribute('data-scheme');
  } else {
    root.setAttribute('data-scheme', theme);
  }
}

/**
 * localStorage-backed persistence adapter — the default for apps without a
 * server-side settings store. Reads synchronously on creation so the cached
 * palette applies on first paint without a flicker.
 */
export function createLocalStoragePersistence(storageKey = 'rr-theme') {
  function read() {
    try {
      const v = window.localStorage?.getItem(storageKey);
      return VALID_THEMES.includes(v) ? v : null;
    } catch {
      return null;
    }
  }
  function write(value) {
    try {
      window.localStorage?.setItem(storageKey, value);
    } catch {
      // Ignore storage errors (private mode, quota, disabled) — the only
      // consequence is the scheme resets to 'auto' on next load.
    }
  }
  const theme = ref((typeof window !== 'undefined' && read()) || 'auto');
  return {
    theme,
    setTheme(value) {
      if (!VALID_THEMES.includes(value)) return;
      theme.value = value;
      write(value);
    },
  };
}

// The default localStorage adapter is a module-level singleton so every
// `useColorScheme()` call within one app shares one theme ref.
let defaultPersistence = null;

// Track which persistence backends already have the data-scheme applier wired
// so repeated `useColorScheme` calls don't stack duplicate watchers.
const appliedBackends = new WeakSet();

/**
 * @param {{ theme: import('vue').Ref<string>, setTheme: (v: string) => void }} [persistence]
 *   Optional persistence adapter. Defaults to a shared localStorage adapter.
 * @returns {{ colorScheme: import('vue').Ref<string>, setColorScheme: (v: string) => void }}
 */
export function useColorScheme(persistence) {
  const backend =
    persistence ||
    (defaultPersistence ??= createLocalStoragePersistence());

  if (!appliedBackends.has(backend)) {
    appliedBackends.add(backend);
    // `immediate` applies the current (cached/default) theme now and on change.
    watch(backend.theme, applyColorScheme, { immediate: true });
  }

  return {
    // Read-only: callers change the scheme through setColorScheme so the
    // persistence backend stays the single writer.
    colorScheme: readonly(backend.theme),
    setColorScheme: backend.setTheme,
  };
}

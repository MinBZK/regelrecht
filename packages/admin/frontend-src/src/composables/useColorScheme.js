/**
 * useColorScheme — color scheme picker for the admin app.
 *
 * Values: 'auto' | 'light' | 'dark'. Applied to <html> as the `data-scheme`
 * attribute that @nldd/design-system keys its dark tokens on; 'auto' is
 * encoded as the *absence* of the attribute so the OS-level
 * `prefers-color-scheme` media query takes over.
 *
 * Unlike the editor's server-backed user settings, the admin has no user
 * settings endpoint, so the preference is persisted in localStorage only.
 */
import { ref, readonly, computed, watchEffect } from 'vue';

const VALID_THEMES = ['auto', 'light', 'dark'];
const STORAGE_KEY = 'rr-admin-theme';

function readCached() {
  try {
    const v = window.localStorage?.getItem(STORAGE_KEY);
    return VALID_THEMES.includes(v) ? v : null;
  } catch {
    return null;
  }
}

function writeCached(value) {
  try {
    window.localStorage?.setItem(STORAGE_KEY, value);
  } catch {
    // Ignore storage errors (private mode, quota, disabled) — the only
    // consequence is the scheme resets to 'auto' on next load.
  }
}

const theme = ref(
  (typeof window !== 'undefined' && readCached()) || 'auto',
);

// Apply to <html>. 'auto' removes the attribute so the design-system's
// `@media (prefers-color-scheme: dark)` selector drives the palette.
watchEffect(() => {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  if (theme.value === 'auto') root.removeAttribute('data-scheme');
  else root.setAttribute('data-scheme', theme.value);
});

function setColorScheme(value) {
  if (!VALID_THEMES.includes(value)) return;
  theme.value = value;
  writeCached(value);
}

export function useColorScheme() {
  return {
    colorScheme: readonly(theme),
    setColorScheme,
  };
}

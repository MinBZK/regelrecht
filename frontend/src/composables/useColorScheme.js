/**
 * Color scheme picker — automatic (follows OS), light, or dark.
 *
 * Storybook keys off the `data-scheme` attribute on `<html>`:
 * - omitted → OS preference via @media(prefers-color-scheme)
 * - "light" / "dark" → forced
 *
 * Selection is persisted in localStorage so it survives refreshes.
 */
import { ref, watch } from 'vue';

const STORAGE_KEY = 'regelrecht-color-scheme';
const VALID = ['auto', 'light', 'dark'];

function loadSaved() {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return VALID.includes(v) ? v : 'auto';
  } catch { return 'auto'; }
}

const colorScheme = ref(loadSaved());

function apply(value) {
  const root = document.documentElement;
  if (value === 'auto') root.removeAttribute('data-scheme');
  else root.setAttribute('data-scheme', value);
}

apply(colorScheme.value);

watch(colorScheme, (val) => {
  apply(val);
  try { localStorage.setItem(STORAGE_KEY, val); } catch { /* ignore */ }
});

export function useColorScheme() {
  return {
    colorScheme,
    setColorScheme: (val) => { if (VALID.includes(val)) colorScheme.value = val; },
  };
}

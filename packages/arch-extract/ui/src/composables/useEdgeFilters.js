/**
 * useEdgeFilters — per-edge-kind visibility toggles for the architecture
 * explorer.
 *
 * The model only produces three relationship kinds (`depends-on`, `impl`,
 * `uses`); `calls` is deliberately never emitted, so it is not offered as a
 * toggle. All three start enabled and the choice is persisted in localStorage
 * so it survives a refresh.
 *
 * The read/write-with-try/catch shape and the module-level singleton follow
 * `useColorScheme.js` in this same directory — the explorer is a standalone npm
 * project with no shared package, so the pattern is reproduced rather than
 * imported. Keep the two in step.
 */
import { ref } from 'vue';

// Order is the display order of the toggles in the toolbar.
export const FILTERABLE_KINDS = ['depends-on', 'impl', 'uses'];

const STORAGE_KEY = 'arch-explorer-edge-kinds';

function read() {
  try {
    const raw = window.localStorage?.getItem(STORAGE_KEY);
    if (!raw) return null;
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return null;
    // Drop anything that is no longer a known kind (defensive against an old
    // persisted value after the kind set changes).
    return arr.filter((k) => FILTERABLE_KINDS.includes(k));
  } catch {
    return null;
  }
}

function write(kinds) {
  try {
    window.localStorage?.setItem(STORAGE_KEY, JSON.stringify(kinds));
  } catch {
    // Ignore storage errors (private mode, quota); the filters just reset to
    // "all on" on the next load.
  }
}

// Module-level singleton so every caller shares one enabled-set ref. `read()`
// may legitimately return an empty array (user turned everything off), so we
// only fall back to "all on" when there is no stored value at all (null).
const stored = typeof window !== 'undefined' ? read() : null;
const enabled = ref(new Set(stored ?? FILTERABLE_KINDS));

export function useEdgeFilters() {
  function toggleKind(kind) {
    if (!FILTERABLE_KINDS.includes(kind)) return;
    const next = new Set(enabled.value);
    if (next.has(kind)) next.delete(kind);
    else next.add(kind);
    enabled.value = next;
    write([...next]);
  }

  function kindEnabled(kind) {
    return enabled.value.has(kind);
  }

  return { enabledKinds: enabled, toggleKind, kindEnabled, FILTERABLE_KINDS };
}

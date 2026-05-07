import { ref, computed } from 'vue';

// Track the last visited fullPath per route name so the top-level
// Bibliotheek/Editor tab switch returns the user to where they were
// instead of the section root.
//
// Backed by sessionStorage so a refresh keeps the state for the same
// tab session, but doesn't bleed across browser tabs/windows.

const STORAGE_KEY = 'regelrecht:last-visited';

function load() {
  try {
    return JSON.parse(sessionStorage.getItem(STORAGE_KEY) ?? '{}');
  } catch {
    return {};
  }
}

const _lastVisited = ref(load());

function save() {
  try {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(_lastVisited.value));
  } catch {
    /* storage may be disabled / quota exceeded — fall back to memory only */
  }
}

export function recordLastVisited(routeName, fullPath) {
  if (!routeName) return;
  // Mutate in place rather than re-spreading. The ref wraps a reactive
  // object so a property assignment still notifies the lastLibraryPath /
  // lastEditorPath computeds. Avoids GC churn if the section list grows.
  _lastVisited.value[routeName] = fullPath;
  save();
}

export const lastLibraryPath = computed(() => _lastVisited.value.library ?? '/library');
export const lastEditorPath = computed(() => _lastVisited.value.editor ?? '/editor');

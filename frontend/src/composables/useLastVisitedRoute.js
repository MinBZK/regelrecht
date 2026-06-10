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

// Normalise each section's traject / no-traject route names to a single
// storage key per section: `editor`/`editor-traject` → `editor`,
// `library`/`library-traject` → `library`. Either way it's "the editor
// tab" / "the bibliotheek tab" semantically; keeping them separate would
// mean a `??` fallback chain has to pick a winner regardless of which
// was visited more recently. With one shared key, the last write wins by
// definition.
function storageKeyFor(routeName) {
  if (routeName === 'editor-traject') return 'editor';
  if (routeName === 'library-traject') return 'library';
  return routeName;
}

export function recordLastVisited(routeName, fullPath) {
  if (!routeName) return;
  // Mutate in place rather than re-spreading. The ref wraps a reactive
  // object so a property assignment still notifies the lastLibraryPath /
  // lastEditorPath computeds. Avoids GC churn if the section list grows.
  _lastVisited.value[storageKeyFor(routeName)] = fullPath;
  save();
}

export const lastLibraryPath = computed(() => _lastVisited.value.library ?? '/library');

// `/editor` (no traject) is the read-only editor. The first visit
// lands there; subsequent visits restore the most-recently-seen
// editor URL — read-only OR traject-scoped, whichever the user was on
// last.
export const lastEditorPath = computed(
  () => _lastVisited.value.editor ?? '/editor',
);

// Build a router target for the *other* top-level section (Bibliotheek /
// Editor) from its last-visited path, but re-stamp the trajectRef to the
// CURRENTLY active traject. The stored path can carry a stale trajectRef
// (or none) from an earlier visit; the active scope must win, otherwise a
// tab-switch would either drop the traject (the bug this fixes) or
// teleport the user into whatever traject they last had open in that
// section. Returns a `{ name, params }` location for `router.push`.
export function sectionTarget(router, storedPath, activeRef) {
  const loc = router.resolve(storedPath);
  const params = { ...loc.params };
  let name = loc.name;
  if (activeRef) {
    params.trajectRef = activeRef;
    if (name === 'library') name = 'library-traject';
    else if (name === 'editor') name = 'editor-traject';
  } else {
    delete params.trajectRef;
    if (name === 'library-traject') name = 'library';
    else if (name === 'editor-traject') name = 'editor';
  }
  // Defensive: a corrupted or extremely stale sessionStorage path can
  // resolve to an unrecognised (or null) route name, which would make the
  // downstream router.push throw. Fall back to the section root, deriving
  // the section from the stored path's prefix so the correct tab is kept.
  const KNOWN = ['library', 'library-traject', 'editor', 'editor-traject'];
  if (!KNOWN.includes(name)) {
    const section = storedPath.startsWith('/editor') ? 'editor' : 'library';
    name = activeRef ? `${section}-traject` : section;
  }
  return { name, params };
}

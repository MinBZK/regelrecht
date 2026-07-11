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
  // Home section: the public landing ('home'), a public law ('corpus-juris')
  // and the traject bibliotheek ('library-traject') all share one key so the
  // last write wins regardless of which shape the user was last on.
  if (routeName === 'corpus-juris' || routeName === 'library-traject') return 'home';
  return routeName; // 'home', 'editor'
}

export function recordLastVisited(routeName, fullPath) {
  if (!routeName) return;
  // Mutate in place rather than re-spreading. The ref wraps a reactive
  // object so a property assignment still notifies the lastLibraryPath /
  // lastEditorPath computeds. Avoids GC churn if the section list grows.
  _lastVisited.value[storageKeyFor(routeName)] = fullPath;
  save();
}

// The Home section's last-visited path (the public landing, a public law, or
// the traject bibliotheek — all stored under the 'home' key). The export name
// is kept for now to limit this refactor's blast radius across importers.
export const lastLibraryPath = computed(() => _lastVisited.value.home ?? '/');

// `/editor` (no traject) is the read-only editor. The first visit
// lands there; subsequent visits restore the most-recently-seen
// editor URL — the traject chooser OR a traject-scoped editor,
// whichever the user was on last.
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
  const name = loc.name;

  // Editor section = /editor* paths (or an unresolved path under /editor);
  // everything else is the Home section (home / corpus-juris / library-traject).
  const isEditor = name === 'editor' || name === 'editor-traject'
    || (name == null && storedPath.startsWith('/editor'));

  if (isEditor) {
    if (activeRef) {
      params.trajectRef = activeRef;
      // A stored chooser path carries the intended law as query.
      if (loc.query?.law) params.lawId = loc.query.law;
      if (loc.query?.article) params.articleNumber = loc.query.article;
      return { name: 'editor-traject', params };
    }
    // The editor requires a traject: without one the Editor tab goes to the
    // chooser, carrying any law as query so it opens after a traject is picked.
    const query = { ...(loc.query ?? {}) };
    if (params.lawId) query.law = params.lawId;
    if (params.articleNumber) query.article = params.articleNumber;
    return { name: 'editor', params: {}, query };
  }

  // Home section — the active traject scope wins over whatever the stored path
  // carried (re-stamped, or dropped when browsing without a traject).
  if (activeRef) {
    params.trajectRef = activeRef;
    return { name: 'library-traject', params };
  }
  delete params.trajectRef;
  // Public Home: a law drills into corpus-juris; otherwise the bare landing.
  return { name: params.lawId ? 'corpus-juris' : 'home', params };
}

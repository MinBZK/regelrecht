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

// Home-section route names: the public landing + a public law, and the traject
// landing + its corpus. Kept in one place so every "am I on Home?" check and
// "build a Home target" stays in sync as routes are added.
export const HOME_ROUTE_NAMES = [
  'home',
  'corpus-juris',
  'traject-home',
  'library-traject',
  'werkdocumenten-traject',
];
export function isHomeSection(routeName) {
  return HOME_ROUTE_NAMES.includes(routeName);
}

// Build a Home-section route location. With a traject: the bare traject home, or
// its corpus when a law is open. Without one: the public bare landing, or a
// public law. Centralises the with/without-law + with/without-traject split used
// by every Home navigation (tab switch, traject switch, chooser, in-view nav).
export function homeTarget({ trajectRef, lawId, articleNumber } = {}) {
  if (trajectRef) {
    return lawId
      ? { name: 'library-traject', params: { trajectRef, lawId, articleNumber } }
      : { name: 'traject-home', params: { trajectRef } };
  }
  return lawId
    ? { name: 'corpus-juris', params: { lawId, articleNumber } }
    : { name: 'home', params: {} };
}

function storageKeyFor(routeName) {
  if (routeName === 'editor-traject') return 'editor';
  // The whole Home section shares one key so the last write wins regardless of
  // which shape (landing / law / traject) the user was last on.
  if (isHomeSection(routeName)) return 'home';
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

// The page to return to when leaving the harvester — the route you were on when
// you entered it (Home or Editor). Backed by sessionStorage so a refresh inside
// the harvester still remembers where to go back; falls back to the root.
const HARVESTER_RETURN_KEY = 'regelrecht:harvester-return';
export function rememberHarvesterOrigin(fullPath) {
  try {
    sessionStorage.setItem(HARVESTER_RETURN_KEY, fullPath);
  } catch {
    /* storage disabled / quota — the fallback path handles it */
  }
}
export function harvesterReturnPath() {
  try {
    return sessionStorage.getItem(HARVESTER_RETURN_KEY) || '/';
  } catch {
    return '/';
  }
}

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

  // Editor section = /editor* or /trajecten/{ref}/editor* (or an unresolved
  // path under /editor); everything else is the Home section (home /
  // corpus-juris / traject-home / library-traject).
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

  // Werkdocumenten sub-mode of Home: preserve it (re-stamp the active traject,
  // keep the open document) so a Home↔Editor tab switch returns to the open
  // werkdocument. Without a traject the werkdoc route doesn't apply, so it falls
  // through to the corpus shapes below.
  const isWerkdoc = name === 'werkdocumenten-traject'
    || (name == null && storedPath.includes('/werkdocumenten'));
  if (isWerkdoc && activeRef) {
    const target = { name: 'werkdocumenten-traject', params: { trajectRef: activeRef } };
    if (params.docPath) target.params.docPath = params.docPath;
    return target;
  }

  // Home section (corpus) — the active traject scope wins over whatever the
  // stored path carried (re-stamped, or dropped when browsing without a traject).
  return homeTarget({
    trajectRef: activeRef || undefined,
    lawId: params.lawId,
    articleNumber: params.articleNumber,
  });
}

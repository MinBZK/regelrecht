// Pure logic of the code guide: addresses, search and source ranges. No Vue and
// no DOM, so it is tested directly (guide.test.js).
//
// Data comes from `code-guide serve` (src/views.rs): the workspace, one crate,
// one type or one function at a time.

/** Lines of a module's own file shown before the reader asks for more. */
export const MODULE_PREVIEW_LINES = 80;
/** Lines added by one "show more" press. */
export const CONTEXT_STEP = 40;
/** Most search results listed; the rest are summarised. */
export const MAX_RESULTS = 60;

export const VIEWS = ['details', 'graph'];
/** What the graph can show, from the widest to the narrowest. */
export const SCOPES = ['workspace', 'crate', 'module', 'type'];

/**
 * A module path as one address segment. Rust's `::` becomes `.`, the crate root
 * (`""`) becomes `~`, and the `:` of a binary root (`bin:evaluate`) becomes `~`
 * too, so no segment needs percent-encoding. `~` cannot occur in a module name.
 */
export function moduleToSegment(path) {
  if (path === '') return '~';
  return path.replace(/^bin:/, 'bin~').replaceAll('::', '.');
}

export function segmentToModule(segment) {
  if (segment === '~') return '';
  return segment.replace(/^bin~/, 'bin:').replaceAll('.', '::');
}

/**
 * Addresses: `#/` (all crates), `#/engine` (a crate), `#/engine/service` (a
 * module), `#/engine/service/LawExecutionService` (a type or function) and
 * `…/evaluate_law` (a method, by its key). After `?`: `view=graph`,
 * `scope=workspace|crate|module|type` and `source=wide`, each left out when it
 * is the default.
 */
export function parseHash(hash) {
  const [path, query = ''] = String(hash || '').replace(/^#\/?/, '').split('?');
  const parts = path
    .split('/')
    .filter(Boolean)
    .map((p) => {
      try {
        return decodeURIComponent(p);
      } catch {
        return p;
      }
    });
  const [crate, module, item, method] = parts;
  const params = new URLSearchParams(query);
  const view = params.get('view');
  const scope = params.get('scope');
  return {
    crate: crate ?? null,
    module: module === undefined ? null : segmentToModule(module),
    item: item ?? null,
    method: method ?? null,
    view: VIEWS.includes(view) ? view : 'details',
    scope: SCOPES.includes(scope) ? scope : null,
    wide: params.get('source') === 'wide',
  };
}

/** The inverse of {@link parseHash}. */
export function formatHash({ crate, module, item, method, view, scope, wide }) {
  const parts = [];
  if (crate) {
    parts.push(crate);
    if (module !== null && module !== undefined) {
      parts.push(moduleToSegment(module));
      if (item) {
        parts.push(item);
        if (method) parts.push(method);
      }
    }
  }
  const path = '#/' + parts.map((p, i) => (i === 1 ? p : encodeURIComponent(p))).join('/');
  const params = new URLSearchParams();
  if (view === 'graph') {
    params.set('view', 'graph');
    if (scope && SCOPES.includes(scope)) params.set('scope', scope);
  }
  if (wide) params.set('source', 'wide');
  const q = params.toString();
  return q ? `${path}?${q}` : path;
}

/** How a module is named on screen: its path, or the crate root. */
export function moduleLabel(path, crate) {
  if (path === '' || path === null) return crate ? `${crate} (crate root)` : 'crate root';
  if (path.startsWith('bin:')) return `binary ${path.slice(4).split('::').join('::')}`;
  return path;
}

export function findModule(crateView, path) {
  return crateView?.modules.find((m) => m.path === path) ?? null;
}

/** Whether `name` in `module` is a type (otherwise a function). */
export function isType(module, name) {
  return !!module?.types.some((t) => t.name === name);
}

/**
 * Modules nested by their `::` path, the crate root first and binaries last. A
 * module whose parent is missing becomes a root rather than disappearing.
 */
export function moduleTree(modules) {
  const byName = new Map(modules.map((m) => [m.path, { path: m.path, module: m, children: [] }]));
  const roots = [];
  for (const node of byName.values()) {
    const p = node.path;
    const parent = p.includes('::') ? byName.get(p.slice(0, p.lastIndexOf('::'))) : null;
    (parent ? parent.children : roots).push(node);
  }
  const rank = (p) => (p === '' ? 0 : p.startsWith('bin:') ? 2 : 1);
  const sort = (nodes) => {
    nodes.sort((a, b) => rank(a.path) - rank(b.path) || a.path.localeCompare(b.path));
    nodes.forEach((n) => sort(n.children));
  };
  sort(roots);
  return roots;
}

const leaf = (path) => path.split('::').pop();
export { leaf as moduleLeaf };

/**
 * Search over a crate's modules, types, functions and methods (the server's
 * `search` list): names rank above docs, exact above prefix above substring.
 */
export function search(entries, query) {
  const q = String(query || '').trim().toLowerCase();
  if (!q || !entries) return { results: [], total: 0 };
  const hits = [];
  for (const e of entries) {
    const score = rank(q, e.name, e.doc);
    if (score > 0) hits.push({ ...e, score });
  }
  hits.sort((a, b) => b.score - a.score || a.name.localeCompare(b.name) || a.module.localeCompare(b.module));
  return { results: hits.slice(0, MAX_RESULTS), total: hits.length };
}

function rank(q, name, doc) {
  const n = name.toLowerCase();
  const tail = n.split('::').pop();
  if (n === q || tail === q) return 100;
  if (n.startsWith(q) || tail.startsWith(q)) return 80;
  if (n.includes(q)) return 60;
  if (doc && doc.toLowerCase().includes(q)) return 20;
  return 0;
}

/** The address a search hit, or a call reference, points at. */
export function targetOf(entry) {
  if (entry.kind === 'module') return { module: entry.module, item: null, method: null };
  if (entry.type) return { module: entry.module, item: entry.type, method: entry.key ?? null };
  return { module: entry.module, item: entry.key ?? entry.name, method: null };
}

/** What a call reference (`FnRef`) links to. */
export function refTarget(ref) {
  return ref.type
    ? { crate: ref.crate, module: ref.module, item: ref.type, method: ref.key }
    : { crate: ref.crate, module: ref.module, item: ref.key, method: null };
}

/**
 * What the source viewer shows: the file and the first and last line. A method
 * or function shows its extent (doc comment included), a type its extent, a
 * module the top of its file. `null` when there is nothing to show.
 */
export function sourceRange({ module, type, fn, method }) {
  const target = method ?? fn;
  if (target?.extent) return { path: target.place.file, from: target.extent[0], to: target.extent[1] };
  if (target?.place) return { path: target.place.file, from: target.place.line, to: target.place.line + 40 };
  if (type?.extent) return { path: type.place.file, from: type.extent[0], to: type.extent[1] };
  if (type?.place) return { path: type.place.file, from: type.place.line, to: type.place.line + 40 };
  if (module?.file) return { path: module.file, from: 1, to: MODULE_PREVIEW_LINES };
  return null;
}

/** A range grown by `step` lines either way, kept inside the file. */
export function widen(range, total, step = CONTEXT_STEP) {
  return { ...range, from: Math.max(1, range.from - step), to: Math.min(total, range.to + step) };
}

/** Items that are `pub`, unless all are asked for. */
export function visibleItems(items, includePrivate) {
  return items.filter((i) => includePrivate || i.vis === 'pub');
}

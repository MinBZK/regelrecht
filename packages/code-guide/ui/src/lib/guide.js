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

/**
 * The graph's options as they appear in the address, each with its allowed
 * values; the first is the default and is left out of the address. `level` and
 * `depth` have no fixed default (it depends on what is open), so `null` stands
 * for "the default here".
 */
export const GRAPH_OPTIONS = {
  level: [null, 'functions', 'modules', 'crates'],
  calls: ['both', 'in', 'out'],
  depth: [null, '0', '1', '2', '3', '4', '5', '6'],
  layout: ['right', 'down', 'force', 'rings'],
  spacing: ['normal', 'compact', 'roomy'],
};

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
 * `…/evaluate_law` (a method, by its key). After `?`: `view=graph` with the
 * graph's options ({@link GRAPH_OPTIONS}), and `source=wide`, each left out
 * when it is the default.
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
  const options = Object.fromEntries(
    Object.entries(GRAPH_OPTIONS).map(([k, allowed]) => {
      const v = params.get(k);
      return [k, allowed.includes(v) ? v : allowed[0]];
    }),
  );
  return {
    crate: crate ?? null,
    module: module === undefined ? null : segmentToModule(module),
    item: item ?? null,
    method: method ?? null,
    view: VIEWS.includes(view) ? view : 'details',
    ...options,
    wide: params.get('source') === 'wide',
  };
}

/** The inverse of {@link parseHash}. */
export function formatHash({ crate, module, item, method, view, wide, ...options }) {
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
    for (const [k, allowed] of Object.entries(GRAPH_OPTIONS)) {
      const v = options[k] === undefined || options[k] === null ? null : String(options[k]);
      if (v !== null && v !== allowed[0] && allowed.includes(v)) params.set(k, v);
    }
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

/**
 * The trail from all crates down to what is open, for the breadcrumbs: each
 * step `{ text, target }`, where `target` is the address to go to (for
 * `hrefFor`) and is `null` for the last step, the page itself. A nested
 * module gets a step per level (`annotation` › `resolver`); a level that is not
 * a module of its own (no file, no items: `crateView` does not list it) is
 * shown without a link rather than as one that leads nowhere.
 */
export function breadcrumbs(route, crateView) {
  const r = route;
  const steps = [{ text: 'All crates', target: { crate: null } }];
  if (!r.crate) return [{ text: 'All crates', target: null }];
  steps.push({ text: r.crate, target: { crate: r.crate } });
  if (r.module !== null && r.module !== undefined) {
    if (r.module === '') {
      steps.push({ text: 'crate root', target: { crate: r.crate, module: '' } });
    } else if (r.module.startsWith('bin:')) {
      steps.push({ text: `binary ${r.module.slice(4)}`, target: { crate: r.crate, module: r.module } });
    } else {
      const parts = r.module.split('::');
      parts.forEach((part, i) => {
        const path = parts.slice(0, i + 1).join('::');
        const known = !crateView || crateView.modules.some((m) => m.path === path);
        steps.push({ text: part, target: known || path === r.module ? { crate: r.crate, module: path } : undefined });
      });
    }
    if (r.item) steps.push({ text: r.item, target: { crate: r.crate, module: r.module, item: r.item } });
    if (r.item && r.method) {
      steps.push({ text: r.method, target: { crate: r.crate, module: r.module, item: r.item, method: r.method } });
    }
  }
  // The last step is where you are: no link. `undefined` marks a level that
  // is no page of its own; it has no link either.
  const last = steps.at(-1);
  last.target = null;
  return steps;
}

/** Longest cycle tag that still names its partner; a longer one says only that. */
const CYCLE_TAG_MAX = 22;

/**
 * The tag for a module or crate that calls in a cycle with `names`, for a list
 * row where the tag must stay short: a tag does not wrap, and a long one
 * squeezes the row's name and description into a narrow column. One short
 * partner is named ("cycle with context"); otherwise the tag says "in a cycle".
 * `label` always names them all, for a screen reader; the module's own page
 * shows the full list.
 */
export function cycleTag(names) {
  const label = `calls in a cycle with ${names.join(', ')}`;
  const named = `cycle with ${names[0]}`;
  return { text: names.length === 1 && named.length <= CYCLE_TAG_MAX ? named : 'in a cycle', label };
}

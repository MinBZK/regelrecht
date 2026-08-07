/**
 * archIndex — the shared, DOM-free view on the architecture model.
 *
 * Every rendering in the explorer (the current Vue Flow view and the three
 * schema prototypes) needs the same three things from `GET /api/model`:
 *
 *  1. a containment index (parent / children / roots, in a stable order),
 *  2. the set of nodes that make up one *detail level* (`container`,
 *     `component`, `code`),
 *  3. a way to lift a deep node to whatever is on screen at that level.
 *
 * This module owns (1) and (2); `archRollup.js` owns (3) plus the aggregation
 * of relations. Both are pure functions of the model, so the whole pipeline is
 * unit-testable without a DOM (and reusable by every prototype).
 */

/** The three detail levels, coarse → fine. Also their zoom order. */
export const LEVELS = Object.freeze(['container', 'component', 'code']);

const LEVEL_RANK = Object.freeze({ container: 0, component: 1, code: 2 });

/** Rank of a level, coarse = 0. Unknown levels are treated as the finest. */
export function levelRank(level) {
  const r = LEVEL_RANK[level];
  return r === undefined ? LEVEL_RANK.code : r;
}

/**
 * Sort order for children: coarse kinds first, then alphabetically. Keeps a
 * crate's modules above its loose fns, types above their methods, etc. The
 * roots (Rust `crate`s and JS `app`s) are grouped so the two tiers do not
 * interleave; below an app, directories sort above the files they group.
 */
export const KIND_RANK = Object.freeze({
  crate: 0,
  app: 1,
  binary: 2,
  dir: 3,
  module: 4,
  component: 5,
  composable: 6,
  trait: 7,
  struct: 8,
  enum: 9,
  fn: 10,
  method: 11,
});

export function sortChildren(byId) {
  return (a, b) => {
    const na = byId.get(a);
    const nb = byId.get(b);
    const ra = KIND_RANK[na?.kind] ?? 9;
    const rb = KIND_RANK[nb?.kind] ?? 9;
    if (ra !== rb) return ra - rb;
    return (na?.name || a).localeCompare(nb?.name || b);
  };
}

/**
 * Build the containment index.
 *
 * A node whose `parent` does not exist in the model (the extractor emits a
 * handful of those — e.g. an inherent `impl` on a type it never recorded) is
 * treated as a **root** rather than being dropped: the whole point of the
 * rollup is that nothing disappears silently, and an unreachable node would
 * take its relations with it.
 *
 * @returns {{
 *   byId: Map<string, object>,
 *   childrenMap: Map<string, string[]>,
 *   roots: string[],
 *   parentOf: (id: string) => string|undefined,
 *   isAncestor: (anc: string, desc: string) => boolean,
 *   depthOf: (id: string) => number,
 * }}
 */
export function buildIndex(model) {
  const byId = new Map(model.nodes.map((n) => [n.id, n]));

  // Resolve `parent` once so a dangling reference behaves like "no parent"
  // everywhere instead of at each call site.
  const parents = new Map();
  for (const n of model.nodes) {
    const p = n.parent && byId.has(n.parent) ? n.parent : undefined;
    parents.set(n.id, p);
  }
  const parentOf = (id) => parents.get(id);

  const childrenMap = new Map();
  const roots = [];
  for (const n of model.nodes) {
    const p = parents.get(n.id);
    if (!p) {
      roots.push(n.id);
      continue;
    }
    if (!childrenMap.has(p)) childrenMap.set(p, []);
    childrenMap.get(p).push(n.id);
  }
  const cmp = sortChildren(byId);
  for (const arr of childrenMap.values()) arr.sort(cmp);
  roots.sort(cmp);

  const isAncestor = (anc, desc) => {
    let cur = parentOf(desc);
    while (cur) {
      if (cur === anc) return true;
      cur = parentOf(cur);
    }
    return false;
  };

  const depths = new Map();
  const depthOf = (id) => {
    if (depths.has(id)) return depths.get(id);
    const p = parentOf(id);
    const d = p ? depthOf(p) + 1 : 0;
    depths.set(id, d);
    return d;
  };
  for (const n of model.nodes) depthOf(n.id);

  return { byId, childrenMap, roots, parentOf, isAncestor, depthOf };
}

/**
 * The nodes that make up one detail level — the "units" a prototype draws.
 *
 * A unit is every node at or above the requested level (`container` shows the
 * 20 crates/apps/binaries; `component` adds the 803 modules/types/components;
 * `code` adds the 1605 methods/fns). Deeper nodes are not dropped: they roll
 * up into their nearest unit ancestor (see `archRollup.js`).
 *
 * Coverage is total by construction. A node that is *deeper* than the level
 * and has **no** ancestor within the level (only possible for the handful of
 * orphaned nodes described in `buildIndex`) becomes a unit itself, so its
 * relations still have somewhere to land.
 *
 * Units come back in depth-first containment order, so a node is always
 * immediately followed by its own subtree. Radial and Matrix use that order
 * directly as their seed ordering, which is what makes sibling components sit
 * next to each other on the ring / diagonal.
 *
 * @returns {{ units: string[], unitSet: Set<string> }}
 */
export function unitsAtLevel(index, level) {
  const maxRank = levelRank(level);
  const units = [];

  const walk = (id, hasRep) => {
    const node = index.byId.get(id);
    const inBudget = levelRank(node?.level) <= maxRank;
    const emitSelf = inBudget || !hasRep;
    if (emitSelf) units.push(id);
    const childHasRep = hasRep || emitSelf;
    for (const child of index.childrenMap.get(id) || []) walk(child, childHasRep);
  };
  for (const root of index.roots) walk(root, false);

  return { units, unitSet: new Set(units) };
}

/**
 * The container (crate / app / binary) a node belongs to — its top-most
 * ancestor-or-self. Used everywhere as the grouping key: hull colour on the
 * Map, ring segment on the Radial, diagonal block on the Matrix.
 */
export function containerOf(index, id) {
  let cur = id;
  let last = id;
  while (cur) {
    last = cur;
    const p = index.parentOf(cur);
    if (!p) break;
    cur = p;
  }
  return last;
}

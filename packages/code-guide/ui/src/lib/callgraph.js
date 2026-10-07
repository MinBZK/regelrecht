// The call graph around what is open: pure logic, no Vue, no DOM, no Cytoscape,
// so it is tested directly (callgraph.test.js).
//
// The server sends every function and every call once (`/api/calls`). From
// that this builds the graph at one of three levels (functions, or the calls
// lifted to modules or crates), takes the part around the focus (callers,
// callees or both, to a depth), and works out what to highlight: the paths
// between two places, or everything upstream or downstream of one.

import { MODULE_PREVIEW_LINES } from './guide.js';

/**
 * The design system's category colours used per crate, in order; status
 * colours left out. The same names colour a crate's tag in the legend.
 * Neighbours differ in hue: crates are coloured in name order, and two blues
 * side by side would make two crates look like one.
 */
export const CATEGORIES = [
  'hemelblauw',
  'oranje',
  'mintgroen',
  'paars',
  'geel',
  'roze',
  'mosgroen',
  'violet',
  'bruin',
  'lichtblauw',
  'donkergroen',
  'donkergeel',
  'lintblauw',
  'robijnrood',
  'donkerbruin',
  'groen',
  'donkerblauw',
  'rood',
];

/** Most nodes drawn; beyond that the farthest are left out, and counted. */
export const MAX_NODES = 400;

export const LEVELS = ['functions', 'modules', 'crates'];

/** The levels a page offers, finest first; the first is its default. */
export function levelsFor(page) {
  if (page === 'workspace') return ['crates'];
  if (page === 'crate') return ['modules', 'crates'];
  return LEVELS;
}

/** The level shown: the address's choice if the page offers it, else its default. */
export function levelFor(page, chosen) {
  const offered = levelsFor(page);
  return offered.includes(chosen) ? chosen : offered[0];
}

/**
 * How far from the focus the graph reaches by default. At the level of the
 * page itself (all crates, a crate's modules) the focus is everything there
 * is, so 0; a single function gets two steps, enough to see where it sits.
 */
export function defaultDepth(level, page) {
  if (level === 'crates') return page === 'workspace' ? 0 : 1;
  if (level === 'modules') return page === 'crate' ? 0 : 1;
  return page === 'function' ? 2 : 1;
}

const fnId = (i) => `f${i}`;
/** The function index in a function node's id, or null for another node. */
const fnIndex = (id) => (/^f\d+$/.test(id) ? Number(id.slice(1)) : null);
/** `crate::module`, or the crate alone for its root: a node says which crate it is in. */
const where = (crate, module) => (module === '' ? crate : `${crate}::${module.replace(/^bin:/, 'bin ')}`);
const modId = (crate, module) => `m:${crate}|${module}`;
const crateId = (crate) => `c:${crate}`;
export const edgeId = (source, target) => `${source}>${target}`;

/**
 * The call graph at `level`: nodes by id, and for each node its callees (`out`)
 * and callers (`in`) with the number of call sites. At the module and crate
 * levels the calls are summed, and calls within one module or crate are left
 * out: they are not an edge at that level.
 */
export function lift(data, level) {
  const nodes = new Map();
  const out = new Map();
  const inn = new Map();
  const idOf = [];
  const fileOf = new Map((data.modules ?? []).map((m) => [`${m.crate}|${m.module}`, m.file]));
  const moduleSource = (crate, module) => {
    const path = fileOf.get(`${crate}|${module}`);
    return path ? { path, from: 1, to: MODULE_PREVIEW_LINES } : null;
  };
  data.nodes.forEach((f, i) => {
    // Where the code is: a method's `impl` may sit in another module than its
    // type, and calls between modules are counted by where they are written.
    const home = f.implModule ?? f.module;
    let node;
    if (level === 'functions') {
      const title = f.type ? `${f.type}::${f.name}` : f.key;
      const trait = f.key.includes('@') ? f.key.slice(f.key.indexOf('@') + 1) : null;
      node = {
        id: fnId(i),
        label: title,
        sub: [trait && `impl ${trait}`, where(f.crate, home)].filter(Boolean).join(' · '),
        crate: f.crate,
        home,
        source: f.place
          ? {
              path: f.place.file,
              from: f.extent?.[0] ?? f.place.line,
              to: f.extent?.[1] ?? f.place.line + 40,
            }
          : null,
        vis: f.vis,
        doc: f.doc,
        stale: f.stale,
        target: f.type
          ? { crate: f.crate, module: f.module, item: f.type, method: f.key }
          : { crate: f.crate, module: f.module, item: f.key, method: null },
      };
    } else if (level === 'modules') {
      node = {
        id: modId(f.crate, home),
        label: home === '' ? `${f.crate} (crate root)` : where(f.crate, home),
        sub: null,
        crate: f.crate,
        home,
        source: moduleSource(f.crate, home),
        target: { crate: f.crate, module: home, item: null, method: null },
      };
    } else {
      node = {
        id: crateId(f.crate),
        label: f.crate,
        sub: null,
        crate: f.crate,
        source: moduleSource(f.crate, ''),
        target: { crate: f.crate, module: null, item: null, method: null },
      };
    }
    idOf[i] = node.id;
    if (!nodes.has(node.id)) {
      nodes.set(node.id, node);
      out.set(node.id, new Map());
      inn.set(node.id, new Map());
    }
  });
  for (const [a, b, sites] of data.edges) {
    const s = idOf[a];
    const t = idOf[b];
    if (s === undefined || t === undefined) continue;
    if (s === t && level !== 'functions') continue;
    out.get(s).set(t, (out.get(s).get(t) ?? 0) + sites);
    inn.get(t).set(s, (inn.get(t).get(s) ?? 0) + sites);
  }
  // `idOf[i]`: the node function `i` belongs to, so a node can be opened up
  // into its functions again (see `expand`).
  return { level, nodes, out, in: inn, idOf };
}

/**
 * The part with single functions added to it (`picked`: module or crate node
 * id -> the function ids added from it). An added function is drawn inside its
 * node, which becomes a box, and the calls it makes or receives run through
 * it; the node's other calls stay on the node itself. A call between the added
 * function and the rest of its own node is not an edge here (it would run
 * from a box to what is inside it).
 *
 * Returns a part of the same shape (`dist`, `edges`, `omitted`) plus `parents`
 * (function id -> its box). A function gets its node's distance. At the
 * functions level, or with nothing added, the part is unchanged.
 */
export function expand(data, graph, part, picked) {
  const parents = new Map();
  if (graph.level === 'functions' || !picked?.size) return { ...part, parents };
  const unit = (i) => {
    const id = fnId(i);
    return picked.get(graph.idOf[i])?.has(id) ? id : graph.idOf[i];
  };
  const sums = new Map();
  for (const [a, b, sites] of data.edges) {
    const A = graph.idOf[a];
    const B = graph.idOf[b];
    if (!part.dist.has(A) || !part.dist.has(B)) continue;
    const ua = unit(a);
    const ub = unit(b);
    // Within one node, or a function calling itself; and inside one box only
    // between two added functions.
    if (ua === ub || (A === B && (ua === A || ub === B))) continue;
    const id = edgeId(ua, ub);
    const e = sums.get(id);
    if (e) e.sites += sites;
    else sums.set(id, { id, source: ua, target: ub, sites });
  }
  const dist = new Map(part.dist);
  for (const [agg, fns] of picked) {
    if (!part.dist.has(agg)) continue;
    for (const f of fns) {
      parents.set(f, agg);
      dist.set(f, part.dist.get(agg));
    }
  }
  return { dist, edges: [...sums.values()], omitted: part.omitted, parents };
}

/**
 * The functions of a module or crate node that make or receive the calls drawn
 * to and from it in `part`: what can be added to the graph from it. Each with
 * the call sites crossing the node either way, most first.
 */
export function boundaryOf(data, graph, part, nodeId) {
  const counts = new Map();
  const bump = (i, key, sites) => {
    if (!counts.has(i)) counts.set(i, { id: fnId(i), in: 0, out: 0 });
    counts.get(i)[key] += sites;
  };
  for (const [a, b, sites] of data.edges) {
    const A = graph.idOf[a];
    const B = graph.idOf[b];
    if (A === B || !part.dist.has(A) || !part.dist.has(B)) continue;
    if (A === nodeId) bump(a, 'out', sites);
    if (B === nodeId) bump(b, 'in', sites);
  }
  return [...counts.values()].sort((x, y) => y.in + y.out - (x.in + x.out) || (x.id < y.id ? -1 : 1));
}

/**
 * The ids that stand for `ids` in an expanded part: a node itself, and the
 * functions added inside it.
 */
export function unitsOf(part, ids) {
  const out = new Set();
  for (const id of ids) {
    if (part.dist.has(id)) out.add(id);
    for (const [u, p] of part.parents ?? []) if (p === id) out.add(u);
  }
  return out;
}

/**
 * The ids the graph is centred on, for what is open. Functions: the function,
 * the selected method, a type's methods, or a module's functions. Modules: the
 * module, or every module of the crate on a crate page. Crates: the crate, or
 * every crate.
 */
export function focusOf(graph, route, page) {
  const r = route;
  const ids = new Set();
  for (const n of graph.nodes.values()) {
    const t = n.target;
    let hit = false;
    if (graph.level === 'crates') hit = page === 'workspace' || n.crate === r.crate;
    else if (graph.level === 'modules') hit = n.crate === r.crate && (page === 'crate' || t.module === r.module);
    else if (t.crate === r.crate) {
      // A module's functions are the ones written in it; a type's methods are
      // addressed under the type's module, wherever their `impl` is.
      if (page === 'module') hit = n.home === r.module;
      else if (page === 'type') {
        hit = t.module === r.module && t.item === r.item && t.method !== null && (!r.method || t.method === r.method);
      } else if (page === 'function') hit = t.module === r.module && t.method === null && t.item === r.item;
    }
    if (hit) ids.add(n.id);
  }
  return ids;
}

/**
 * Whether an item address (`#/crate/module/Name`) is a type or a function,
 * read from the call graph itself, so the graph does not wait for the crate's
 * page data to know what it is centred on.
 */
export function itemKind(data, route) {
  const r = route;
  const isType = data.nodes.some((n) => n.crate === r.crate && n.module === r.module && n.type === r.item);
  return isType ? 'type' : 'function';
}

/** Breadth first along `adj` from `start`, at most `depth` steps: id -> steps. */
function reach(adj, start, depth = Infinity, within = null) {
  const dist = new Map([...start].map((id) => [id, 0]));
  let frontier = [...start];
  for (let d = 1; d <= depth && frontier.length; d++) {
    const next = [];
    for (const id of frontier) {
      for (const n of adj.get(id)?.keys() ?? []) {
        if (dist.has(n) || (within && !within.has(n))) continue;
        dist.set(n, d);
        next.push(n);
      }
    }
    frontier = next;
  }
  return dist;
}

/**
 * The part of `graph` around `focus`: callers (`in`), callees (`out`) or both,
 * up to `depth` steps. Each node gets its distance: 0 for the focus, negative
 * for callers, positive for callees (a node that is both keeps the nearer).
 * Edges are every call between two nodes that are in it. Past
 * {@link MAX_NODES} the farthest nodes are left out and counted in `omitted`.
 */
export function neighbourhood(graph, focus, { calls = 'both', depth = 1 } = {}) {
  const down = calls === 'in' ? new Map([...focus].map((id) => [id, 0])) : reach(graph.out, focus, depth);
  const up = calls === 'out' ? new Map([...focus].map((id) => [id, 0])) : reach(graph.in, focus, depth);
  const dist = new Map();
  for (const [id, d] of down) dist.set(id, d);
  for (const [id, d] of up) if (!dist.has(id) || d < dist.get(id)) dist.set(id, -d);
  let omitted = 0;
  if (dist.size > MAX_NODES) {
    const kept = [...dist].sort((a, b) => Math.abs(a[1]) - Math.abs(b[1]) || (a[0] < b[0] ? -1 : 1)).slice(0, MAX_NODES);
    omitted = dist.size - kept.length;
    dist.clear();
    for (const [id, d] of kept) dist.set(id, d);
  }
  const edges = [];
  for (const s of dist.keys()) {
    for (const [t, sites] of graph.out.get(s) ?? []) {
      if (dist.has(t)) edges.push({ id: edgeId(s, t), source: s, target: t, sites });
    }
  }
  return { dist, edges, omitted };
}

/** Adjacency of a part (as {@link neighbourhood} returns it), both ways. */
function adjacency(part) {
  const out = new Map();
  const inn = new Map();
  for (const id of part.dist.keys()) {
    out.set(id, new Map());
    inn.set(id, new Map());
  }
  for (const e of part.edges) {
    out.get(e.source).set(e.target, e.sites);
    inn.get(e.target).set(e.source, e.sites);
  }
  return { out, in: inn };
}

/**
 * Every node and edge on some path from a node in `from` to a node in `to`
 * within `part`: a node reachable from `from` that also reaches `to`, and an
 * edge from such a node to another.
 */
export function pathsBetween(part, from, to) {
  const adj = adjacency(part);
  const fwd = reach(adj.out, from);
  const back = reach(adj.in, to);
  const nodes = new Set([...fwd.keys()].filter((id) => back.has(id)));
  const edges = new Set(part.edges.filter((e) => nodes.has(e.source) && nodes.has(e.target)).map((e) => e.id));
  return { nodes, edges };
}

const union = (a, b) => ({ nodes: new Set([...a.nodes, ...b.nodes]), edges: new Set([...a.edges, ...b.edges]) });

/**
 * What to highlight for the selected nodes. `paths`: the paths between the
 * focus and one selected node, either way, or between two selected nodes.
 * `in`: everything that leads to the selection (its callers, theirs, …).
 * `out`: everything the selection leads to. `null` without a selection.
 * `connected` is false when the selection has no path of that kind.
 */
export function highlight(part, { mode = 'paths', selected = [], focus = new Set() }) {
  // A selected box stands for the functions shown in it.
  const sel = selected.map((id) => unitsOf(part, [id])).filter((s) => s.size);
  if (!sel.length) return null;
  const all = new Set(sel.flatMap((s) => [...s]));
  let hl;
  if (mode === 'in' || mode === 'out') {
    const adj = adjacency(part);
    const nodes = new Set(reach(mode === 'in' ? adj.in : adj.out, all).keys());
    const edges = new Set(part.edges.filter((e) => nodes.has(e.source) && nodes.has(e.target)).map((e) => e.id));
    hl = { nodes, edges };
  } else {
    const [a, b] = sel;
    const other = b ?? new Set([...unitsOf(part, focus)].filter((id) => !a.has(id)));
    if (other.size) {
      hl = union(pathsBetween(part, a, other), pathsBetween(part, other, a));
    } else {
      // What is open, alone: paths to itself say nothing, so its own callers
      // and callees in the graph.
      const edges = part.edges.filter((e) => a.has(e.source) || a.has(e.target));
      hl = { nodes: new Set(edges.flatMap((e) => [e.source, e.target])), edges: new Set(edges.map((e) => e.id)) };
    }
  }
  const connected = hl.edges.size > 0;
  for (const id of all) hl.nodes.add(id);
  return { ...hl, connected };
}

/** A node's width for its longest line, within limits (px). */
export function nodeWidth(lines) {
  const longest = Math.max(0, ...lines.filter(Boolean).map((l) => l.length));
  return Math.round(Math.min(340, Math.max(120, longest * 7.6 + 28)));
}

/**
 * The elements Cytoscape draws. A node carries its colour group (`ci`, one per
 * crate in `crates` order), its size, and for the rings layout how far inward
 * it goes (`ring`: the focus in the middle). An edge is thicker for more call
 * sites, and labelled with the count when there is more than one. Colour
 * groups repeat after `palette` crates.
 */
export function toElements(lookup, part, focus, crates, palette = CATEGORIES.length) {
  const maxDist = Math.max(0, ...[...part.dist.values()].map(Math.abs));
  const ci = new Map(crates.map((c, i) => [c, i]));
  const node = (id) => (typeof lookup === 'function' ? lookup(id) : lookup.nodes.get(id));
  const parents = part.parents ?? new Map();
  // A node with functions added to it is drawn as a box around them, with its
  // own label and its remaining calls.
  const boxIds = new Set(parents.values());
  const boxes = [...boxIds].map((id) => {
    const n = node(id);
    return {
      group: 'nodes',
      data: { id, display: n.label, ci: (ci.get(n.crate) ?? 0) % palette, dist: part.dist.get(id) ?? 0 },
      classes: ['box', focus.has(id) && 'focus'].filter(Boolean).join(' '),
    };
  });
  const nodes = [...part.dist].filter(([id]) => !boxIds.has(id)).map(([id, d]) => {
    const n = node(id);
    const classes = [focus.has(id) && 'focus', n.vis === 'private' && 'private', n.stale && 'stale'].filter(Boolean);
    // Inside a box the box names the module; the node keeps only its own name.
    const sub = parents.has(id) ? null : n.sub;
    return {
      group: 'nodes',
      data: {
        id,
        ...(parents.has(id) ? { parent: parents.get(id) } : {}),
        display: sub ? `${n.label}\n${sub}` : n.label,
        width: nodeWidth([n.label, sub]),
        height: sub ? 48 : 34,
        ci: (ci.get(n.crate) ?? 0) % palette,
        dist: d,
        ring: maxDist - Math.abs(d),
      },
      classes: classes.join(' '),
    };
  });
  const edges = part.edges.map((e) => ({
    group: 'edges',
    data: {
      id: e.id,
      source: e.source,
      target: e.target,
      sites: e.sites,
      count: e.sites > 1 ? `×${e.sites}` : '',
      width: Math.min(5, 1.2 + Math.log2(e.sites) * 0.8),
    },
  }));
  return [...boxes, ...nodes, ...edges];
}

export const LAYOUTS = {
  right: 'Layered, left to right',
  down: 'Layered, top to bottom',
  force: 'Force-directed',
  rings: 'Rings around what is open',
};
const SPACING = { compact: 0.6, normal: 1, roomy: 1.7 };

/**
 * Cytoscape layout options. Layered puts callers above (or left of) what they
 * call, so a path reads in one direction; rings put the focus in the middle and
 * each step out on the next ring.
 */
export function layoutOptions(name = 'right', spacing = 'normal') {
  const f = SPACING[spacing] ?? 1;
  // `id` lets a caller tell whether the options really changed: the object is
  // rebuilt on every navigation, and a layout run is not free.
  const common = { id: `${name}:${spacing}`, animate: false, fit: true, padding: 24 };
  if (name === 'force') {
    return {
      ...common,
      name: 'fcose',
      quality: 'default',
      randomize: true,
      nodeDimensionsIncludeLabels: true,
      idealEdgeLength: () => 90 * f,
      nodeRepulsion: () => 9000 * f,
      nodeSeparation: 60 * f,
    };
  }
  if (name === 'rings') {
    return {
      ...common,
      name: 'concentric',
      concentric: (node) => node.data('ring'),
      levelWidth: () => 1,
      minNodeSpacing: 18 * f,
      avoidOverlap: true,
    };
  }
  return {
    ...common,
    name: 'dagre',
    rankDir: name === 'right' ? 'LR' : 'TB',
    nodeSep: (name === 'right' ? 14 : 28) * f,
    rankSep: (name === 'right' ? 110 : 70) * f,
    edgeSep: 8 * f,
    ranker: 'network-simplex',
  };
}

/**
 * Wraps the ranks of a layered layout that are too long into several rows
 * (`axis: 'y'`, top to bottom) or columns (`axis: 'x'`, left to right), and
 * moves the ranks after them along to make room. Dagre puts every node of a
 * rank on one line, so an opened box or a function with many callers becomes
 * a line too long to read at any zoom that shows it whole.
 *
 * `items`: `{ id, x, y, w, h }` (centre and size). Returns id -> `{ x, y }`.
 * A rank keeps its order and its centre; its rows are filled evenly.
 */
export function wrapRanks(items, { axis = 'y', maxPer = 5, gap = 28 } = {}) {
  const along = axis === 'y' ? 'x' : 'y';
  const size = axis === 'y' ? 'w' : 'h';
  const across = axis === 'y' ? 'h' : 'w';
  const ranks = new Map();
  for (const it of items) {
    const key = Math.round(it[axis]);
    if (!ranks.has(key)) ranks.set(key, []);
    ranks.get(key).push(it);
  }
  const out = new Map();
  let offset = 0;
  for (const key of [...ranks.keys()].sort((a, b) => a - b)) {
    const rank = ranks.get(key).sort((a, b) => a[along] - b[along]);
    if (rank.length <= maxPer) {
      for (const it of rank) out.set(it.id, { [along]: it[along], [axis]: it[axis] + offset });
      continue;
    }
    const lines = Math.ceil(rank.length / maxPer);
    const per = Math.ceil(rank.length / lines);
    const step = Math.max(...rank.map((it) => it[across])) + gap;
    const lo = Math.min(...rank.map((it) => it[along] - it[size] / 2));
    const hi = Math.max(...rank.map((it) => it[along] + it[size] / 2));
    const centre = (lo + hi) / 2;
    for (let l = 0; l < lines; l++) {
      const line = rank.slice(l * per, (l + 1) * per);
      const total = line.reduce((t, it) => t + it[size], 0) + gap * (line.length - 1);
      let at = centre - total / 2;
      for (const it of line) {
        out.set(it.id, { [along]: at + it[size] / 2, [axis]: key + offset + l * step });
        at += it[size] + gap;
      }
    }
    offset += (lines - 1) * step;
  }
  clearBoxes(items, out, gap);
  return out;
}

/** Room a box takes around its functions: its padding, and its label above. */
const BOX_PAD = 16;
const BOX_LABEL = 22;

/**
 * Moves a node that is not in a box out of that box's area, to the right of
 * everything at its height. Wrapping can leave a node of a rank the box spans
 * inside it, where it reads as one of the box's functions. Items with a
 * `parent` are in that box; `pos` is updated in place.
 */
function clearBoxes(items, pos, gap) {
  const byParent = new Map();
  for (const it of items) {
    if (!it.parent) continue;
    if (!byParent.has(it.parent)) byParent.set(it.parent, []);
    byParent.get(it.parent).push(it);
  }
  const rect = (it) => {
    const p = pos.get(it.id);
    return { x1: p.x - it.w / 2, x2: p.x + it.w / 2, y1: p.y - it.h / 2, y2: p.y + it.h / 2 };
  };
  for (const [box, members] of byParent) {
    const rs = members.map(rect);
    const b = {
      x1: Math.min(...rs.map((r) => r.x1)) - BOX_PAD,
      x2: Math.max(...rs.map((r) => r.x2)) + BOX_PAD,
      y1: Math.min(...rs.map((r) => r.y1)) - BOX_PAD - BOX_LABEL,
      y2: Math.max(...rs.map((r) => r.y2)) + BOX_PAD,
    };
    for (const it of items) {
      if (it.parent === box) continue;
      const r = rect(it);
      if (r.x2 <= b.x1 || r.x1 >= b.x2 || r.y2 <= b.y1 || r.y1 >= b.y2) continue;
      // Right of the box and of anything already at this height.
      const right = Math.max(
        b.x2,
        ...items
          .filter((o) => o !== it && o.parent !== box)
          .map(rect)
          .filter((o) => o.y2 > r.y1 && o.y1 < r.y2 && o.x1 >= b.x1)
          .map((o) => o.x2),
      );
      pos.set(it.id, { ...pos.get(it.id), x: right + gap + it.w / 2 });
    }
  }
}

/**
 * The wrapping (see `wrapRanks`) at which the whole graph fits a canvas of
 * `width` by `height` largest: every row length from 3 up to the longest rank
 * is tried. Returns id -> `{ x, y }`.
 */
export function bestWrap(items, { axis = 'y', gap = 28, width, height, padding = 24 }) {
  const ranks = new Map();
  for (const it of items) {
    const key = Math.round(it[axis]);
    ranks.set(key, (ranks.get(key) ?? 0) + 1);
  }
  const longest = Math.max(0, ...ranks.values());
  let best = null;
  let bestZoom = -1;
  for (let maxPer = Math.min(3, longest); maxPer <= longest; maxPer++) {
    const pos = wrapRanks(items, { axis, maxPer, gap });
    const zoom = fitZoom(items, pos, width - 2 * padding, height - 2 * padding);
    if (zoom > bestZoom) {
      bestZoom = zoom;
      best = pos;
    }
  }
  return best ?? new Map(items.map((it) => [it.id, { x: it.x, y: it.y }]));
}

/** The zoom at which the placed items just fit a `width` by `height` box. */
function fitZoom(items, pos, width, height) {
  let x1 = Infinity;
  let y1 = Infinity;
  let x2 = -Infinity;
  let y2 = -Infinity;
  for (const it of items) {
    const p = pos.get(it.id);
    x1 = Math.min(x1, p.x - it.w / 2);
    x2 = Math.max(x2, p.x + it.w / 2);
    y1 = Math.min(y1, p.y - it.h / 2);
    y2 = Math.max(y2, p.y + it.h / 2);
  }
  return Math.min(width / Math.max(1, x2 - x1), height / Math.max(1, y2 - y1));
}

/**
 * The crates in a part, each with the category colour its nodes are drawn in,
 * by name: what the legend shows.
 */
export function legendOf(lookup, part, crates) {
  const order = new Map(crates.map((c, i) => [c, i]));
  const present = new Set();
  for (const id of part.dist.keys()) {
    const n = lookup(id);
    if (n) present.add(n.crate);
  }
  return [...present].sort().map((crate) => ({
    crate,
    color: CATEGORIES[(order.get(crate) ?? 0) % CATEGORIES.length],
  }));
}

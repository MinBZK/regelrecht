// Pure logic of the graph views: which nodes and edges a scope shows, and where
// they sit. No Vue and no DOM, so the layout is tested directly (graph.test.js)
// and is the same on every run: no randomness and no physics, only sorting.
//
// Every edge is a call, as rust-analyzer resolved it: `{source, target}` reads
// "source calls target". Layers come from the server (layer 0 calls nothing
// else in the graph) and are laid out as rows with the callees at the bottom,
// so every arrow points down.

import { moduleLabel, moduleToSegment } from './guide.js';

// A module's node id is its address segment, never its path: the crate root's
// path is the empty string, and Vue Flow treats an empty id as no node at all,
// silently dropping every edge to or from it.
const modId = (path) => moduleToSegment(path);

/** Narrowest node, in px: what a graph of short names gets. */
export const NODE_WIDTH = 180;
/** Widest node, in px; a longer name is cut off rather than spreading the graph. */
export const MAX_NODE_WIDTH = 360;
/** Rough width of one character of a node's bold label, in px, plus the card's padding. */
const CHAR_WIDTH = 9.5;
const CARD_PADDING = 32;
/** Space between two neighbouring nodes, in px. */
const NODE_SPACING = 20;

/**
 * How wide the nodes of a graph are: enough for its longest label, within
 * [NODE_WIDTH, MAX_NODE_WIDTH]. Method names run much longer than module names
 * (`evaluate_law_output_with_trace_builder`), and two of them that differ only
 * at the end are useless when both are cut off.
 */
export function nodeWidthFor(labels) {
  const longest = Math.max(0, ...labels.map((l) => l.length));
  return Math.round(Math.min(MAX_NODE_WIDTH, Math.max(NODE_WIDTH, longest * CHAR_WIDTH + CARD_PADDING)));
}
/** Distance between two nodes of a layer, in px. */
export const NODE_GAP = 200;
/** Distance between two layers, in px. */
export const LAYER_GAP = 84;
/** Distance between the rows a long layer wraps into, in px. */
export const WRAP_GAP = 52;
/**
 * Most nodes in one row. A longer layer wraps into several rows, so a module
 * with a dozen dependencies is a block of a few rows and not a line so wide that
 * fitting it to the canvas shrinks the text to nothing.
 */
export const MAX_PER_ROW = 6;
/**
 * Widest a row may get, in px. With long labels (method names) the nodes are
 * wide, and six of them make a row so wide that fitting it shrinks everything;
 * a row then holds fewer nodes, never fewer than three.
 */
export const ROW_WIDTH = 1300;

/** How many nodes of `nodeWidth` fit in one row. */
export function perRowFor(nodeWidth) {
  return Math.max(3, Math.min(MAX_PER_ROW, Math.floor(ROW_WIDTH / (nodeWidth + NODE_SPACING))));
}
/** Passes of the ordering sweep. A few are enough for graphs of this size. */
const SWEEPS = 6;

/**
 * Positions for `nodes`: a row per layer with layer 0 at the bottom, ordered
 * within a row by the average position of their neighbours so edges cross as
 * little as a few sweeps can manage, then centred horizontally. A layer longer
 * than `maxPerRow` wraps into evenly filled rows. Ties break on id, so the
 * result never depends on input order.
 *
 * @returns {Map<string, {x: number, y: number}>}
 */
export function layoutLayers(
  nodes,
  edges,
  { nodeGap = NODE_GAP, layerGap = LAYER_GAP, wrapGap = WRAP_GAP, maxPerRow = MAX_PER_ROW } = {},
) {
  const byLayer = new Map();
  for (const n of [...nodes].sort((a, b) => a.id.localeCompare(b.id))) {
    if (!byLayer.has(n.layer)) byLayer.set(n.layer, []);
    byLayer.get(n.layer).push(n.id);
  }
  const layers = [...byLayer.keys()].sort((a, b) => a - b);

  const neighbours = new Map(nodes.map((n) => [n.id, []]));
  for (const e of edges) {
    neighbours.get(e.source)?.push(e.target);
    neighbours.get(e.target)?.push(e.source);
  }

  // Place of each node as a fraction of its row, so rows of different lengths
  // can be compared.
  const fraction = () => {
    const f = new Map();
    for (const ids of byLayer.values()) ids.forEach((id, i) => f.set(id, ids.length > 1 ? i / (ids.length - 1) : 0.5));
    return f;
  };

  for (let pass = 0; pass < SWEEPS; pass += 1) {
    const order = pass % 2 === 0 ? layers : [...layers].reverse();
    for (const layer of order) {
      const f = fraction();
      const ids = byLayer.get(layer);
      const score = (id) => {
        const ns = neighbours.get(id);
        return ns.length ? ns.reduce((sum, n) => sum + (f.get(n) ?? 0.5), 0) / ns.length : f.get(id);
      };
      const scores = new Map(ids.map((id) => [id, score(id)]));
      ids.sort((a, b) => scores.get(a) - scores.get(b) || a.localeCompare(b));
    }
  }

  // Row lengths of each layer: as few rows as the limit allows, filled evenly
  // (13 nodes at 6 per row is 5, 4, 4, not 6, 6, 1).
  const rowSizes = new Map(
    layers.map((layer) => {
      const n = byLayer.get(layer).length;
      const rows = Math.ceil(n / maxPerRow);
      const base = Math.floor(n / rows);
      const extra = n % rows;
      return [layer, Array.from({ length: rows }, (_, r) => base + (r < extra ? 1 : 0))];
    }),
  );
  const widest = Math.max(1, ...[...rowSizes.values()].flat());

  const positions = new Map();
  // Highest layer first, so the cursor runs from the top of the picture down.
  let y = 0;
  for (const layer of [...layers].reverse()) {
    const ids = byLayer.get(layer);
    const sizes = rowSizes.get(layer);
    let next = 0;
    sizes.forEach((size, row) => {
      // Rows are centred on each other, and on the widest row of the picture.
      const left = ((widest - size) / 2) * nodeGap;
      for (let column = 0; column < size; column += 1) {
        positions.set(ids[next], { x: left + column * nodeGap, y: y + row * wrapGap });
        next += 1;
      }
    });
    y += (sizes.length - 1) * wrapGap + layerGap;
  }
  return positions;
}


const edgeId = (source, target) => `${source}->${target}`;

/** All crates, layered by which crate calls into which. */
export function workspaceGraph(ws) {
  if (!ws) return { nodes: [], edges: [] };
  return {
    nodes: ws.crates.map((c) => ({
      id: c.name,
      label: c.name,
      layer: c.layer,
      cycleWith: c.cycleWith,
      target: { crate: c.name, module: null },
    })),
    edges: ws.edges.map((e) => ({ id: edgeId(e.from, e.to), source: e.from, target: e.to, calls: e.calls })),
  };
}

/** One crate's modules, layered by the calls between them. */
export function crateGraph(crateView) {
  if (!crateView) return { nodes: [], edges: [] };
  const name = crateView.name;
  const paths = new Set(crateView.modules.map((m) => m.path));
  const nodes = crateView.modules.map((m) => ({
    id: modId(m.path),
    label: moduleLabel(m.path, name),
    layer: m.layer,
    vis: m.vis,
    cycleWith: m.cycleWith,
    target: { crate: name, module: m.path },
  }));
  const edges = [];
  for (const m of crateView.modules) {
    for (const l of m.callsInto) {
      if (l.crate === name && paths.has(l.module)) {
        const [a, b] = [modId(m.path), modId(l.module)];
        edges.push({ id: edgeId(a, b), source: a, target: b, calls: l.calls });
      }
    }
  }
  return { nodes, edges };
}

/**
 * The open module, what it calls (below) and what calls it (above), across
 * crates. Only the edges that touch the module are drawn: with the calls among
 * its neighbours as well, a module with many neighbours becomes a hairball.
 */
export function moduleGraph(crateView, path) {
  const m = crateView?.modules.find((x) => x.path === path);
  if (!m) return { nodes: [], edges: [] };
  const here = crateView.name;
  const id = (crate, module) => (crate === here ? modId(module) : `${crate}|${modId(module)}`);
  const label = (crate, module) => (crate === here ? moduleLabel(module, crate) : `${crate}::${moduleLabel(module, crate)}`);
  const callees = new Map(m.callsInto.map((l) => [id(l.crate, l.module), l]));
  const callers = new Map(m.calledFrom.map((l) => [id(l.crate, l.module), l]));
  const self = id(here, path);
  const nodes = [{ id: self, label: label(here, path), layer: 1, role: 'self', target: { crate: here, module: path } }];
  for (const key of new Set([...callees.keys(), ...callers.keys()])) {
    if (key === self) continue;
    const l = callees.get(key) ?? callers.get(key);
    const both = callees.has(key) && callers.has(key);
    const role = both ? 'both' : callees.has(key) ? 'callee' : 'caller';
    nodes.push({
      id: key,
      label: label(l.crate, l.module),
      layer: role === 'callee' ? 0 : role === 'caller' ? 2 : 1,
      role,
      target: { crate: l.crate, module: l.module },
    });
  }
  nodes.sort((a, b) => a.id.localeCompare(b.id));
  const edges = [
    ...[...callees].map(([k, l]) => ({ id: edgeId(self, k), source: self, target: k, calls: l.calls })),
    ...[...callers].map(([k, l]) => ({ id: edgeId(k, self), source: k, target: self, calls: l.calls })),
  ];
  return { nodes, edges };
}

/**
 * A type's public methods and the calls among them (private helpers folded on
 * the server, kept in `via`), with the modules outside the type that call its
 * public methods on a row above. Methods with no call in or out are returned as
 * `unconnected` rather than drawn as loose boxes.
 */
export function typeGraph(typeView) {
  if (!typeView) return { nodes: [], edges: [], unconnected: [] };
  const g = typeView.graph;
  const layerOf = new Map();
  g.layers.forEach((layer, i) => layer.flat().forEach((k) => layerOf.set(k, i)));
  const top = Math.max(0, ...layerOf.values()) + 1;
  const called = new Set(g.external.flatMap((e) => Object.keys(e.targets)));
  const connected = new Set([...g.edges.flatMap((e) => [e.from, e.to]), ...called]);
  // Every method callable from outside the type; only private ones are helpers.
  const publicMethods = typeView.methods.filter((m) => m.vis !== 'private');
  const loc = { crate: typeView.crate, module: typeView.module, item: typeView.name };
  const nodes = publicMethods
    .filter((m) => connected.has(m.key))
    .map((m) => ({ id: m.key, label: m.key, layer: layerOf.get(m.key) ?? 0, target: { ...loc, method: m.key } }));
  const edges = g.edges.map((e) => ({ id: edgeId(e.from, e.to), source: e.from, target: e.to, via: e.via }));
  for (const e of g.external) {
    const id = `${e.crate}|${modId(e.module)}`;
    nodes.push({
      id,
      label: e.crate === typeView.crate ? moduleLabel(e.module, e.crate) : `${e.crate}::${moduleLabel(e.module, e.crate)}`,
      layer: top,
      external: true,
      target: { crate: e.crate, module: e.module },
    });
    for (const [key, calls] of Object.entries(e.targets)) {
      edges.push({ id: edgeId(id, key), source: id, target: key, calls });
    }
  }
  return {
    nodes,
    edges,
    unconnected: publicMethods
      .filter((m) => !connected.has(m.key))
      .map((m) => m.key)
      .sort(),
  };
}

/** The scope a page shows when the address names none: its own level. */
export function defaultScope(route, isTypePage) {
  if (isTypePage) return 'type';
  if (route.module !== null && route.module !== undefined) return 'module';
  if (route.crate) return 'crate';
  return 'workspace';
}

/**
 * Everything the graph component needs: nodes with positions and links, edges,
 * and the node width. `hrefFor(target)` turns a node's target into an address,
 * so the caller decides what a click keeps (the view, the scope).
 */
export function buildGraph({ scope, workspace, crateView, typeView, route, hrefFor }) {
  let graph;
  if (scope === 'workspace') graph = workspaceGraph(workspace);
  else if (scope === 'crate') graph = crateGraph(crateView);
  else if (scope === 'module') graph = moduleGraph(crateView, route.module);
  else graph = typeGraph(typeView);

  const nodeWidth = nodeWidthFor(graph.nodes.map((n) => n.label));
  const positions = layoutLayers(graph.nodes, graph.edges, {
    nodeGap: nodeWidth + NODE_SPACING,
    maxPerRow: perRowFor(nodeWidth),
  });
  const selected =
    scope === 'workspace'
      ? route.crate
      : scope === 'type'
        ? route.method
        : scope === 'crate' && route.module !== null && route.module !== undefined
          ? modId(route.module)
          : null;
  const selfId = scope === 'module' ? graph.nodes.find((n) => n.role === 'self')?.id : null;
  return {
    nodeWidth,
    unconnected: graph.unconnected ?? [],
    nodes: graph.nodes.map((n) => ({
      ...n,
      position: positions.get(n.id),
      href: hrefFor(n.target),
      selected: n.id === selected || n.id === selfId,
    })),
    edges: graph.edges,
  };
}

/**
 * mapLayout — prototype 1 of 3: "Map".
 *
 * Blocks and lines, but placed by a real layout engine instead of the grid the
 * current view uses. dagre ranks the rolled-up graph left→right, so a node's
 * column *is* its position in the dependency order and the engine — not the
 * folder tree — decides who ends up next to whom. That is the whole point: the
 * current view places by containment and therefore cannot show you that two
 * heavily-coupled types are neighbours.
 *
 * **Two stages below the container level.** A single flat dagre run over 823
 * components produces a correct but useless picture: the dependency graph is
 * shallow, so nearly everything lands in a handful of ranks and the drawing
 * becomes a ribbon hundreds of times taller than it is wide. So each container
 * is laid out on its own (a *district*), and the districts are then laid out
 * against each other by their aggregated dependencies. That is also what makes
 * it fast — many small runs instead of one quadratic-ish one.
 *
 * Choice of engine: dagre over ELK. ELK's edge is exactly this nested case, but
 * doing the nesting by hand is two dagre calls, and dagre is synchronous — no
 * worker, no async layout in the middle of a scroll.
 *
 * Pure function of (model, level): no DOM, no canvas, no Vue.
 */
import dagre from '@dagrejs/dagre';
import { buildScene } from './scene.js';
import { WORLD_SIZE, boundsOf, fitToWorld } from '../lib/normalize.js';

/** Box size grows with the rolled-up degree, so hubs are literally bigger. */
const BOX_MIN_W = 54;
const BOX_MIN_H = 20;
const BOX_GROWTH = 11; // px per √relation

export function nodeBoxSize(degree) {
  const g = Math.sqrt(Math.max(0, degree)) * BOX_GROWTH;
  return { w: BOX_MIN_W + g * 1.6, h: BOX_MIN_H + g * 0.55 };
}

/** How far apart dagre keeps ranks / siblings, inside a district. */
const INNER_SPACING = { ranksep: 70, nodesep: 18, edgesep: 6 };
/** …and between the districts themselves. */
const OUTER_SPACING = { ranksep: 260, nodesep: 140, edgesep: 40 };
/** Breathing room around a district's contents. */
const DISTRICT_PAD = 34;

/**
 * @param {object} model
 * @param {string} level  'container' | 'component' | 'code'
 * @param {object} [opts] `{ index, enabledKinds }` — see buildScene
 * @returns {{
 *   kind: 'map', level: string,
 *   nodes: Array<{id,x,y,w,h,degree,kind,name,container,internal}>,
 *   edges: Array<{id,from,to,kind,weight,points:Array<{x,y}>}>,
 *   groups: Array<{id,name,minX,minY,maxX,maxY,count}>,
 *   bounds: object, stats: object,
 * }}
 */
export function layoutMap(model, level, opts = {}) {
  const scene = buildScene(model, level, opts);
  const placed =
    level === 'container' ? layoutFlat(scene) : layoutDistricts(scene);

  const { nodes, edges } = placed;

  // One world box for every prototype and every level (see lib/normalize.js).
  // Node centres *and* routed line points go in, so a wide detour cannot push
  // the drawing outside the box the other prototypes also live in.
  const { scale } = fitToWorld([...nodes, ...edges.flatMap((e) => e.points)]);

  // Box sizes shrink with the same factor, but never below what one node's
  // share of the world box can carry — otherwise a zero-degree unit collapses
  // into an invisible sliver and "everything is here" stops being true. The
  // upper bound keeps a tiny graph from producing blocks that burst out of the
  // world box.
  const pitch = WORLD_SIZE / Math.sqrt(Math.max(1, nodes.length));
  const minW = Math.min(pitch * 0.42, WORLD_SIZE * 0.08);
  const minH = Math.min(pitch * 0.12, WORLD_SIZE * 0.022);
  const maxW = WORLD_SIZE * 0.16;
  const maxH = WORLD_SIZE * 0.06;
  for (const n of nodes) {
    n.w = Math.min(maxW, Math.max(n.w * scale, minW));
    n.h = Math.min(maxH, Math.max(n.h * scale, minH));
  }

  return {
    kind: 'map',
    level,
    nodes,
    edges,
    groups: groupHulls(nodes, scene),
    // Bounds include the boxes and the routed lines, not just the node centres,
    // so fit-to-view does not clip the outermost blocks or a wide detour.
    bounds: boundsOf([
      ...nodes.flatMap((n) => [
        { x: n.x - n.w / 2, y: n.y - n.h / 2 },
        { x: n.x + n.w / 2, y: n.y + n.h / 2 },
      ]),
      ...edges.flatMap((e) => e.points),
    ]),
    stats: scene.stats,
  };
}

function toNode(u, laid) {
  return {
    id: u.id,
    name: u.name,
    kind: u.kind,
    level: u.level,
    container: u.container,
    degree: u.degree,
    internal: u.internal,
    x: laid.x,
    y: laid.y,
    w: laid.width,
    h: laid.height,
  };
}

/** Run dagre over a set of units and the links between them. */
function runDagre(units, links, spacing, ranker) {
  const g = new dagre.graphlib.Graph({ multigraph: true, compound: false });
  g.setGraph({ rankdir: 'LR', ranker, ...spacing, marginx: 20, marginy: 20 });
  g.setDefaultEdgeLabel(() => ({}));
  for (const u of units) {
    const size = u.size || nodeBoxSize(u.degree);
    g.setNode(u.id, { width: size.w, height: size.h });
  }
  for (const link of links) {
    // dagre cannot rank a self-loop and would drop it; the rollup never
    // produces one (equal endpoints become an internal counter), but guard
    // anyway so a model change cannot silently lose a line.
    if (link.from === link.to) continue;
    g.setEdge(link.from, link.to, { weight: link.weight }, link.id);
  }
  dagre.layout(g);
  return g;
}

/** One dagre run over everything. Only used for the 20-odd containers. */
function layoutFlat(scene) {
  const g = runDagre(scene.units, scene.links, OUTER_SPACING, 'network-simplex');
  const nodes = scene.units.map((u) =>
    toNode(u, g.node(u.id) || { x: 0, y: 0, width: BOX_MIN_W, height: BOX_MIN_H }),
  );
  const nodeById = new Map(nodes.map((n) => [n.id, n]));
  const edges = scene.links.map((link) => {
    const laid = link.from === link.to ? null : g.edge({ v: link.from, w: link.to, name: link.id });
    return {
      id: link.id,
      from: link.from,
      to: link.to,
      kind: link.kind,
      weight: link.weight,
      points: laid?.points?.length >= 2 ? laid.points.map((p) => ({ x: p.x, y: p.y })) : chord(nodeById, link),
    };
  });
  return { nodes, edges };
}

/** A straight line between two node centres, for a relation dagre did not route. */
function chord(nodeById, link) {
  const a = nodeById.get(link.from);
  const b = nodeById.get(link.to);
  return [
    { x: a?.x ?? 0, y: a?.y ?? 0 },
    { x: b?.x ?? 0, y: b?.y ?? 0 },
  ];
}

/**
 * District layout: lay out each container's own units, then lay out the
 * containers against each other and translate everything into place.
 */
function layoutDistricts(scene) {
  const byContainer = new Map();
  for (const u of scene.units) {
    if (!byContainer.has(u.container)) byContainer.set(u.container, []);
    byContainer.get(u.container).push(u);
  }

  const inner = new Map(); // container -> { units, links }
  for (const [container, units] of byContainer) inner.set(container, { units, links: [] });
  const crossLinks = [];
  const unitContainer = new Map(scene.units.map((u) => [u.id, u.container]));
  for (const link of scene.links) {
    const ca = unitContainer.get(link.from);
    const cb = unitContainer.get(link.to);
    if (ca && ca === cb) inner.get(ca).links.push(link);
    else crossLinks.push(link);
  }

  // Stage 1: each district on its own. `longest-path` for the big ones — it is
  // O(V+E) where network-simplex is not, and at this density the tighter ranks
  // it gives up are invisible.
  const local = new Map(); // unit id -> { x, y, width, height }
  const districts = [];
  for (const [container, { units, links }] of inner) {
    const g = runDagre(units, links, INNER_SPACING, units.length > 120 ? 'longest-path' : 'network-simplex');
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const u of units) {
      const laid = g.node(u.id) || { x: 0, y: 0, width: BOX_MIN_W, height: BOX_MIN_H };
      local.set(u.id, laid);
      minX = Math.min(minX, laid.x - laid.width / 2);
      minY = Math.min(minY, laid.y - laid.height / 2);
      maxX = Math.max(maxX, laid.x + laid.width / 2);
      maxY = Math.max(maxY, laid.y + laid.height / 2);
    }
    districts.push({
      id: container,
      degree: 0,
      size: { w: maxX - minX + DISTRICT_PAD * 2, h: maxY - minY + DISTRICT_PAD * 2 },
      originX: minX - DISTRICT_PAD,
      originY: minY - DISTRICT_PAD,
      units,
    });
  }

  // Stage 2: the districts against each other, on their aggregated relations.
  const districtLinks = new Map();
  for (const link of crossLinks) {
    const key = `${unitContainer.get(link.from)}->${unitContainer.get(link.to)}`;
    const agg = districtLinks.get(key);
    if (agg) agg.weight += link.weight;
    else
      districtLinks.set(key, {
        id: key,
        from: unitContainer.get(link.from),
        to: unitContainer.get(link.to),
        weight: link.weight,
      });
  }
  const outer = runDagre(districts, [...districtLinks.values()], OUTER_SPACING, 'network-simplex');

  const nodes = [];
  for (const d of districts) {
    const box = outer.node(d.id) || { x: 0, y: 0, width: d.size.w, height: d.size.h };
    const dx = box.x - box.width / 2 - d.originX;
    const dy = box.y - box.height / 2 - d.originY;
    for (const u of d.units) {
      const laid = local.get(u.id);
      nodes.push(toNode(u, { x: laid.x + dx, y: laid.y + dy, width: laid.width, height: laid.height }));
    }
  }

  // Relations: dagre's own routing inside a district, straight chords between
  // districts. A straight chord across the map is honest about what it is —
  // the interesting part of a cross-district relation is *that* it crosses.
  const nodeById = new Map(nodes.map((n) => [n.id, n]));
  const edges = scene.links.map((link) => ({
    id: link.id,
    from: link.from,
    to: link.to,
    kind: link.kind,
    weight: link.weight,
    points: chord(nodeById, link),
  }));

  return { nodes, edges };
}

/**
 * A labelled bounding box per container, drawn behind its members. This is the
 * only place the folder tree still speaks in the Map — and the gap between a
 * hull's spread and its members' positions is exactly what tells you whether
 * the code agrees with the folders.
 */
function groupHulls(nodes, scene) {
  const boxes = new Map();
  for (const n of nodes) {
    let b = boxes.get(n.container);
    if (!b) {
      b = { id: n.container, minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity, count: 0 };
      boxes.set(n.container, b);
    }
    b.minX = Math.min(b.minX, n.x - n.w / 2);
    b.minY = Math.min(b.minY, n.y - n.h / 2);
    b.maxX = Math.max(b.maxX, n.x + n.w / 2);
    b.maxY = Math.max(b.maxY, n.y + n.h / 2);
    b.count += 1;
  }
  return [...boxes.values()].map((b) => ({
    ...b,
    name: scene.index.byId.get(b.id)?.name || b.id,
  }));
}

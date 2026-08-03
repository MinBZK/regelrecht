/**
 * radialLayout — prototype 2 of 3: "Radiaal".
 *
 * Every unit sits on one ring, ordered by the containment tree, so a container
 * owns a contiguous arc and its members are neighbours by construction. The
 * relations are then drawn as **hierarchical edge bundling** (Holten 2006):
 * instead of a straight chord, a relation follows the tree — up from its
 * source to the lowest common ancestor and back down to its target — and the
 * resulting control polygon is straightened by a factor β. Relations that share
 * a route bundle into a visible rope.
 *
 * What that buys, and what it costs, is the thing this prototype is here to
 * show: bundles make traffic *between subsystems* legible as a few thick ropes,
 * at the price of no longer being able to follow one individual line.
 *
 * Pure function of (model, level): no DOM, no canvas, no Vue.
 */
import { buildScene } from './scene.js';
import { WORLD_SIZE } from '../lib/normalize.js';

/** Ring radius, in the shared world box (which runs −500…+500). */
const RING_R = WORLD_SIZE * 0.42;

/**
 * Bundling strength. β = 1 follows the tree exactly (maximum bundling, every
 * line hugs the hierarchy); β = 0 is a straight chord (no bundling). 0.85 is
 * Holten's default and reads as "bundled but still directed".
 */
export const BUNDLE_BETA = 0.85;

/** Samples per bundled curve. Enough to look smooth, few enough to stay cheap. */
const CURVE_SAMPLES = 24;

/**
 * Dot radius grows with the rolled-up degree, so hubs stand out on the ring —
 * but it is capped by the slot pitch, otherwise the 2432 dots of the `code`
 * level merge into one fat sausage and nothing stands out at all.
 */
export function dotRadius(degree, pitch = Infinity) {
  const base = Math.min(1.6, pitch * 0.45);
  const growth = Math.min(1.5, pitch * 0.35);
  return base + Math.sqrt(Math.max(0, degree)) * growth;
}

/**
 * @param {object} model
 * @param {string} level
 * @param {object} [opts] `{ index, enabledKinds }`
 * @returns {{
 *   kind:'radial', level:string,
 *   nodes: Array<{id,x,y,angle,r,degree,kind,name,container,internal}>,
 *   edges: Array<{id,from,to,kind,weight,points:Array<{x,y}>}>,
 *   groups: Array<{id,name,startAngle,endAngle,count}>,
 *   bounds: object, stats: object,
 * }}
 */
export function layoutRadial(model, level, opts = {}) {
  const scene = buildScene(model, level, opts);
  const n = Math.max(1, scene.units.length);

  // 1. One slot per unit, in containment order — a container's members are
  //    therefore one contiguous arc.
  const slot = new Map();
  // Arc length one unit gets on the ring, which is what caps the dot size.
  const pitch = (2 * Math.PI * RING_R) / n;
  const nodes = scene.units.map((u, i) => {
    // Shift by half a slot so the first unit is not clipped by the seam.
    const angle = ((i + 0.5) / n) * Math.PI * 2 - Math.PI / 2;
    const pos = { x: Math.cos(angle) * RING_R, y: Math.sin(angle) * RING_R };
    slot.set(u.id, pos);
    return {
      id: u.id,
      name: u.name,
      kind: u.kind,
      level: u.level,
      container: u.container,
      degree: u.degree,
      internal: u.internal,
      angle,
      r: dotRadius(u.degree, pitch),
      x: pos.x,
      y: pos.y,
    };
  });

  // 2. Control points for the *internal* tree nodes the bundles route through:
  //    the mean angle of the subtree, at a radius set by depth. Shallow
  //    ancestors sit near the centre, so a cross-container relation dives deep
  //    and a within-container one stays near the rim.
  const maxDepth = Math.max(1, ...scene.units.map((u) => scene.index.depthOf(u.id)));
  const control = new Map(slot);
  const ancestorsOf = (id) => {
    const chain = [];
    let cur = scene.index.parentOf(id);
    while (cur) {
      chain.push(cur);
      cur = scene.index.parentOf(cur);
    }
    return chain;
  };
  const acc = new Map(); // internal node -> { sx, sy, count }
  for (const u of scene.units) {
    for (const anc of ancestorsOf(u.id)) {
      let a = acc.get(anc);
      if (!a) {
        a = { sx: 0, sy: 0, count: 0 };
        acc.set(anc, a);
      }
      const p = slot.get(u.id);
      a.sx += p.x;
      a.sy += p.y;
      a.count += 1;
    }
  }
  for (const [id, a] of acc) {
    if (control.has(id)) continue; // also a unit: keep it on the ring
    const mx = a.sx / a.count;
    const my = a.sy / a.count;
    const len = Math.hypot(mx, my) || 1;
    const depth = scene.index.depthOf(id);
    const radius = RING_R * (depth / (maxDepth + 1));
    control.set(id, { x: (mx / len) * radius, y: (my / len) * radius });
  }
  const centre = { x: 0, y: 0 };

  // 3. Route each relation through the tree and straighten it by β.
  const edges = scene.links.map((link) => ({
    id: link.id,
    from: link.from,
    to: link.to,
    kind: link.kind,
    weight: link.weight,
    points: bundledPath(link.from, link.to, control, centre, ancestorsOf),
  }));

  return {
    kind: 'radial',
    level,
    nodes,
    edges,
    groups: arcSegments(nodes, scene, n),
    bounds: { minX: -RING_R * 1.05, minY: -RING_R * 1.05, maxX: RING_R * 1.05, maxY: RING_R * 1.05 },
    stats: scene.stats,
  };
}

/**
 * The tree route source → lowest common ancestor → target, as control points.
 *
 * Both endpoints are searched *including themselves*: a relation from a unit to
 * its own container meets the tree at the container, so it is the shortest
 * route there is. Treating only the strict ancestors as candidates would leave
 * it without a meeting point and send it through the centre of the ring —
 * drawing the most local relation in the model exactly like the most global
 * one, and breaking the reading rule the whole prototype rests on ("a rope
 * diving toward the centre crosses a container boundary").
 */
function treeRoute(from, to, ancestorsOf) {
  const upChain = [from, ...ancestorsOf(from)]; // source → root
  const downChain = [to, ...ancestorsOf(to)]; // target → root
  const upSet = new Set(upChain);
  const lcaIndex = downChain.findIndex((id) => upSet.has(id));
  const lca = lcaIndex >= 0 ? downChain[lcaIndex] : null;

  const head = [];
  for (const id of upChain) {
    head.push(id);
    if (id === lca) break;
  }
  // Back down the target's chain, from just below the meeting point to the
  // target itself.
  const tail = [];
  for (let i = (lcaIndex >= 0 ? lcaIndex : downChain.length) - 1; i >= 0; i -= 1) {
    tail.push(downChain[i]);
  }
  // Two different roots (no common ancestor): route through the centre so the
  // curve still dips inward instead of cutting straight across the ring.
  return { path: [...head, ...tail], throughCentre: lca === null };
}

function bundledPath(from, to, control, centre, ancestorsOf) {
  const { path, throughCentre } = treeRoute(from, to, ancestorsOf);
  const pts = path.map((id) => control.get(id) || centre);
  if (throughCentre) pts.splice(Math.floor(pts.length / 2), 0, centre);
  if (pts.length < 2) return [pts[0] || centre, pts[0] || centre];

  // β-straightening: pull each control point toward the straight chord.
  const first = pts[0];
  const last = pts[pts.length - 1];
  const m = pts.length - 1;
  const ctrl = pts.map((p, i) => {
    const s = i / m;
    return {
      x: BUNDLE_BETA * p.x + (1 - BUNDLE_BETA) * (first.x + s * (last.x - first.x)),
      y: BUNDLE_BETA * p.y + (1 - BUNDLE_BETA) * (first.y + s * (last.y - first.y)),
    };
  });
  return sampleBSpline(ctrl, CURVE_SAMPLES);
}

/**
 * Uniform cubic B-spline through the control polygon, clamped at both ends so
 * the curve actually starts on the source dot and ends on the target dot.
 */
function sampleBSpline(ctrl, samples) {
  if (ctrl.length === 2) return [ctrl[0], ctrl[1]];
  const p = [ctrl[0], ctrl[0], ...ctrl, ctrl[ctrl.length - 1], ctrl[ctrl.length - 1]];
  const segs = p.length - 3;
  const out = [];
  const per = Math.max(2, Math.round(samples / segs));
  for (let s = 0; s < segs; s += 1) {
    const [p0, p1, p2, p3] = [p[s], p[s + 1], p[s + 2], p[s + 3]];
    for (let i = 0; i < per; i += 1) {
      const t = i / per;
      const t2 = t * t;
      const t3 = t2 * t;
      const b0 = (-t3 + 3 * t2 - 3 * t + 1) / 6;
      const b1 = (3 * t3 - 6 * t2 + 4) / 6;
      const b2 = (-3 * t3 + 3 * t2 + 3 * t + 1) / 6;
      const b3 = t3 / 6;
      out.push({
        x: b0 * p0.x + b1 * p1.x + b2 * p2.x + b3 * p3.x,
        y: b0 * p0.y + b1 * p1.y + b2 * p2.y + b3 * p3.y,
      });
    }
  }
  out.push(ctrl[ctrl.length - 1]);
  return out;
}

/**
 * The arc each container occupies on the ring, for the outer band + labels.
 *
 * Spans are taken from *slot boundaries*, not from the member dots: a container
 * with a single member would otherwise get a zero-length arc and disappear from
 * the band exactly when it most needs naming.
 */
function arcSegments(nodes, scene, n) {
  const slotAngle = (i) => (i / n) * Math.PI * 2 - Math.PI / 2;
  const segs = new Map();
  nodes.forEach((node, i) => {
    let s = segs.get(node.container);
    if (!s) {
      s = { id: node.container, first: i, last: i, count: 0 };
      segs.set(node.container, s);
    }
    s.first = Math.min(s.first, i);
    s.last = Math.max(s.last, i);
    s.count += 1;
  });
  return [...segs.values()].map((s) => ({
    id: s.id,
    count: s.count,
    startAngle: slotAngle(s.first),
    endAngle: slotAngle(s.last + 1),
    name: scene.index.byId.get(s.id)?.name || s.id,
  }));
}

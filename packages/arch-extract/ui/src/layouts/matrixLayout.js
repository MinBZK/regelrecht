/**
 * matrixLayout — prototype 3 of 3: "Matrix" (design structure matrix).
 *
 * Rows and columns are the units of the level; a cell (r, c) is filled when row
 * r has a relation *to* column c. The matrix never overlaps and never crosses,
 * so density is readable at any size — the entire question is whether the
 * **ordering** makes the structure visible.
 *
 * Ordering here is two-stage:
 *
 *  1. seed with containment order (`unitsAtLevel` already returns depth-first
 *     order), so the folder structure is the starting hypothesis;
 *  2. refine globally with iterated barycentre sorting — repeatedly move each
 *     unit to the mean position of everything it is connected to. Units that
 *     talk to each other drift together and settle into blocks on the diagonal.
 *
 * Stage 2 is deliberately *not* constrained to stay inside a container: a block
 * that straddles two containers is exactly the finding this prototype exists
 * to surface. The container boundaries are drawn as tick marks on top, so you
 * can see where connectivity and folders disagree.
 *
 * A full clustering algorithm (Louvain, spectral) would order marginally
 * better; the ticket asks for a simple ordering and barycentre is O(iterations
 * × E log V), which keeps the code level instant.
 *
 * Pure function of (model, level): no DOM, no canvas, no Vue.
 */
import { buildScene } from './scene.js';
import { WORLD_SIZE } from '../lib/normalize.js';

/** Barycentre passes. More than ~8 stops changing the picture. */
export const BARYCENTRE_PASSES = 8;

/**
 * @param {object} model
 * @param {string} level
 * @param {object} [opts] `{ index, enabledKinds }`
 * @returns {{
 *   kind:'matrix', level:string,
 *   order: string[],
 *   cell: number,
 *   nodes: Array<{id,row,x,y,degree,kind,name,container,internal}>,
 *   cells: Array<{row,col,kind,weight,from,to}>,
 *   groups: Array<{id,name,start,end,count}>,
 *   bounds: object, stats: object,
 * }}
 */
export function layoutMatrix(model, level, opts = {}) {
  const scene = buildScene(model, level, opts);
  const order = orderUnits(scene);
  const n = Math.max(1, order.length);
  const rowOf = new Map(order.map((id, i) => [id, i]));

  const cell = WORLD_SIZE / n;
  const origin = -WORLD_SIZE / 2;
  const at = (i) => origin + (i + 0.5) * cell;

  const nodes = order.map((id, i) => {
    const u = scene.unitById.get(id);
    return {
      id,
      name: u.name,
      kind: u.kind,
      level: u.level,
      container: u.container,
      degree: u.degree,
      internal: u.internal,
      row: i,
      // Diagonal position: where the unit's own label and self-block sit.
      x: at(i),
      y: at(i),
    };
  });

  // Both ends of a link are units of this level — `buildScene` guarantees it —
  // so a missing row means that guarantee broke, not that the model has such a
  // case. Drawing the cell at (0, 0) anyway would put a wrong mark on the
  // diagonal and say nothing; it is counted as unplaced instead, which is where
  // the toolbar reads "nothing disappears quietly" from.
  let lost = 0;
  const cells = [];
  for (const link of scene.links) {
    const row = rowOf.get(link.from);
    const col = rowOf.get(link.to);
    if (row === undefined || col === undefined) {
      lost += link.weight;
      continue;
    }
    cells.push({ from: link.from, to: link.to, kind: link.kind, weight: link.weight, row, col });
  }

  return {
    kind: 'matrix',
    level,
    order,
    cell,
    nodes,
    cells,
    edges: cells, // alias, so every layout exposes its relations under one name
    groups: groupBands(nodes, scene),
    // The container strips sit just outside the matrix itself, so the framed
    // area is a little larger than the square.
    bounds: {
      minX: origin - WORLD_SIZE * 0.05,
      minY: origin - WORLD_SIZE * 0.05,
      maxX: -origin + WORLD_SIZE * 0.01,
      maxY: -origin + WORLD_SIZE * 0.01,
    },
    stats: lost
      ? { ...scene.stats, visible: scene.stats.visible - lost, unplaced: scene.stats.unplaced + lost }
      : scene.stats,
  };
}

/** Seed order (containment) refined by iterated barycentre sorting. */
export function orderUnits(scene) {
  const ids = scene.units.map((u) => u.id);
  const pos = new Map(ids.map((id, i) => [id, i]));

  // Undirected neighbour lists, weighted by the rolled-up relation count.
  const nbrs = new Map(ids.map((id) => [id, []]));
  for (const link of scene.links) {
    if (!nbrs.has(link.from) || !nbrs.has(link.to)) continue;
    nbrs.get(link.from).push({ id: link.to, w: link.weight });
    nbrs.get(link.to).push({ id: link.from, w: link.weight });
  }

  let current = ids.slice();
  for (let pass = 0; pass < BARYCENTRE_PASSES; pass += 1) {
    const score = new Map();
    for (const id of current) {
      const list = nbrs.get(id);
      if (!list.length) {
        // Isolated units keep their seed position instead of piling up at 0.
        score.set(id, pos.get(id));
        continue;
      }
      let sum = 0;
      let weight = 0;
      for (const { id: other, w } of list) {
        sum += pos.get(other) * w;
        weight += w;
      }
      // Blend with the unit's own position so the order converges instead of
      // oscillating between two mirrored arrangements.
      score.set(id, (sum / weight) * 0.7 + pos.get(id) * 0.3);
    }
    current = current
      .slice()
      .sort((a, b) => score.get(a) - score.get(b) || a.localeCompare(b));
    current.forEach((id, i) => pos.set(id, i));
  }
  return current;
}

/** Contiguous runs of one container along the diagonal, for the axis bands. */
function groupBands(nodes, scene) {
  const bands = [];
  let run = null;
  for (const node of nodes) {
    if (!run || run.id !== node.container) {
      run = { id: node.container, start: node.row, end: node.row, count: 0 };
      bands.push(run);
    }
    run.end = node.row;
    run.count += 1;
  }
  return bands.map((b) => ({ ...b, name: scene.index.byId.get(b.id)?.name || b.id }));
}

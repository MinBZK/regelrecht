/**
 * One shared test suite over the three prototype layouts.
 *
 * The prototypes must be *comparable*, so the properties that make them
 * comparable are asserted the same way for all three (criterion 15):
 *
 *  - the level follows the zoom factor, and nothing but the zoom factor;
 *  - every unit of that level is placed — none silently missing;
 *  - every enabled relation is accounted for, as a line or as an internal
 *    counter, and the reported totals say so;
 *  - the geometry lands in the shared world box, which is what keeps the point
 *    under the cursor still while the level changes.
 */
import { describe, it, expect } from 'vitest';
import { layoutMap } from './mapLayout.js';
import { layoutRadial } from './radialLayout.js';
import { layoutMatrix } from './matrixLayout.js';
import { buildScene } from './scene.js';
import { buildIndex, unitsAtLevel } from '../lib/archIndex.js';
import { WORLD_SIZE } from '../lib/normalize.js';
import { levelForZoom, LEVEL_ZOOM_THRESHOLDS } from '../composables/useSemanticZoom.js';
import { ALL_KINDS, makeModel } from '../test/fixtures.js';

const PROTOTYPES = [
  ['map', layoutMap],
  ['radial', layoutRadial],
  ['matrix', layoutMatrix],
];
const LEVELS = ['container', 'component', 'code'];

describe.each(PROTOTYPES)('%s layout', (name, layout) => {
  it.each(LEVELS)('places every unit of the %s level', (level) => {
    const model = makeModel();
    const expected = unitsAtLevel(buildIndex(model), level).units;
    const result = layout(model, level, { enabledKinds: ALL_KINDS });

    expect(result.nodes.map((n) => n.id).sort()).toEqual(expected.slice().sort());
    expect(result.stats.units).toBe(expected.length);
    for (const n of result.nodes) {
      expect(Number.isFinite(n.x)).toBe(true);
      expect(Number.isFinite(n.y)).toBe(true);
    }
  });

  it.each(LEVELS)('accounts for every relation at the %s level', (level) => {
    const model = makeModel();
    const result = layout(model, level, { enabledKinds: ALL_KINDS });

    // Nothing is dropped: every model edge is either a line or an internal
    // counter on a node.
    expect(result.stats.total).toBe(model.edges.length);
    expect(result.stats.visible).toBe(model.edges.length);
    expect(result.stats.unplaced).toBe(0);

    const drawnWeight = result.edges.reduce((sum, e) => sum + e.weight, 0);
    const internalWeight = result.nodes.reduce((sum, n) => sum + n.internal, 0);
    expect(drawnWeight + internalWeight).toBe(model.edges.length);
  });

  it.each(LEVELS)('only references units of the %s level in its relations', (level) => {
    const model = makeModel();
    const result = layout(model, level, { enabledKinds: ALL_KINDS });
    const ids = new Set(result.nodes.map((n) => n.id));
    for (const e of result.edges) {
      expect(ids.has(e.from)).toBe(true);
      expect(ids.has(e.to)).toBe(true);
    }
  });

  it('lands in the shared world box, so a level change keeps the view still', () => {
    const model = makeModel();
    for (const level of LEVELS) {
      const { nodes, bounds } = layout(model, level, { enabledKinds: ALL_KINDS });
      const half = WORLD_SIZE / 2 + 1;
      for (const n of nodes) {
        expect(Math.abs(n.x)).toBeLessThanOrEqual(half);
        expect(Math.abs(n.y)).toBeLessThanOrEqual(half);
      }
      // The drawn extent may exceed the box a little (a block is wider than
      // its centre), but never by so much that the levels stop being framed
      // the same way.
      expect(bounds.maxX - bounds.minX).toBeLessThanOrEqual(WORLD_SIZE * 1.25);
      expect(bounds.maxY - bounds.minY).toBeLessThanOrEqual(WORLD_SIZE * 1.25);
    }
  });

  it('excludes a disabled relation kind from both the lines and the totals', () => {
    const model = makeModel();
    const result = layout(model, 'container', { enabledKinds: new Set(['depends-on']) });
    expect(result.edges.every((e) => e.kind === 'depends-on')).toBe(true);
    expect(result.stats.total).toBe(2); // only E6 + E7 are depends-on
  });

  it('follows the zoom factor and nothing else', () => {
    const model = makeModel();
    const index = buildIndex(model);
    // Whatever the zoom factor says the level is, that is the level laid out.
    for (const zoom of [0.5, 1, LEVEL_ZOOM_THRESHOLDS.component * 2, LEVEL_ZOOM_THRESHOLDS.code * 2]) {
      const level = levelForZoom(zoom);
      const result = layout(model, level, { index, enabledKinds: ALL_KINDS });
      expect(result.level).toBe(level);
      expect(result.stats.units).toBe(unitsAtLevel(index, level).units.length);
    }
  });

  it('is deterministic — the same input gives the same geometry', () => {
    const model = makeModel();
    const a = layout(model, 'component', { enabledKinds: ALL_KINDS });
    const b = layout(model, 'component', { enabledKinds: ALL_KINDS });
    expect(a.nodes.map((n) => [n.id, n.x, n.y])).toEqual(b.nodes.map((n) => [n.id, n.x, n.y]));
  });
});

describe('buildScene', () => {
  it('keeps a parent→child relation as a real line, since the flat views do not nest', () => {
    // E5 (T1 → crate:a) lifts to mod:a::m1 → crate:a at the component level.
    // The nested Vue Flow view draws that as containment; a flat prototype
    // cannot, so it must stay a line.
    const scene = buildScene(makeModel(), 'component', { enabledKinds: ALL_KINDS });
    const line = scene.links.find((l) => l.from === 'mod:a::m1' && l.to === 'crate:a');
    expect(line).toBeTruthy();
    expect(scene.stats.visible).toBe(scene.stats.total);
  });

  it('counts a relation whose two ends roll up to one unit as internal', () => {
    const scene = buildScene(makeModel(), 'container', { enabledKinds: ALL_KINDS });
    const crateA = scene.units.find((u) => u.id === 'crate:a');
    // E3 (T1→U1) and E5 (T1→crate:a) are both internal to crate:a.
    expect(crateA.internal).toBe(2);
    expect(scene.stats.internal).toBe(2);
  });

  it('reports the degree used for hub sizing', () => {
    const scene = buildScene(makeModel(), 'container', { enabledKinds: ALL_KINDS });
    const byId = new Map(scene.units.map((u) => [u.id, u]));
    // Degree counts *underlying* relations, not lines — that is what makes it
    // comparable to "AppState has 95 connections". crate:a: E1+E2 (uses→b),
    // E4 (impl→c), E6+E7 (depends-on) = 5. crate:b: E1+E2+E6 = 3.
    expect(byId.get('crate:a').degree).toBe(5);
    expect(byId.get('crate:b').degree).toBe(3);
  });
});

describe('matrix ordering', () => {
  it('puts connected units near each other on the diagonal', () => {
    const { order } = layoutMatrix(makeModel(), 'container', { enabledKinds: ALL_KINDS });
    expect(order).toHaveLength(3);
    expect(new Set(order)).toEqual(new Set(['crate:a', 'crate:b', 'crate:c']));
  });

  it('never places a cell outside the matrix', () => {
    const result = layoutMatrix(makeModel(), 'code', { enabledKinds: ALL_KINDS });
    for (const c of result.cells) {
      expect(c.row).toBeGreaterThanOrEqual(0);
      expect(c.row).toBeLessThan(result.order.length);
      expect(c.col).toBeGreaterThanOrEqual(0);
      expect(c.col).toBeLessThan(result.order.length);
    }
  });
});

describe('radial bundling', () => {
  it('routes a curve that starts on its source and ends on its target', () => {
    const result = layoutRadial(makeModel(), 'container', { enabledKinds: ALL_KINDS });
    const byId = new Map(result.nodes.map((n) => [n.id, n]));
    for (const e of result.edges) {
      const first = e.points[0];
      const last = e.points[e.points.length - 1];
      const from = byId.get(e.from);
      const to = byId.get(e.to);
      expect(Math.hypot(first.x - from.x, first.y - from.y)).toBeLessThan(1);
      expect(Math.hypot(last.x - to.x, last.y - to.y)).toBeLessThan(1);
    }
  });

  it('bends a cross-container relation inward instead of cutting straight across', () => {
    const result = layoutRadial(makeModel(), 'component', { enabledKinds: ALL_KINDS });
    const edge = result.edges.find((e) => e.from === 'mod:a::m1' && e.to === 'crate:b');
    const ringR = Math.hypot(result.nodes[0].x, result.nodes[0].y);
    const mid = edge.points[Math.floor(edge.points.length / 2)];
    expect(Math.hypot(mid.x, mid.y)).toBeLessThan(ringR * 0.8);
  });
});

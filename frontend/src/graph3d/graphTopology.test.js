import { describe, it, expect } from 'vitest';
import { buildAdjacency } from './GraphScene.js';
import { buildEdgePositions, buildEdgeColors, needsPerEdgeColor } from './edgeLayer.js';
import { FrameStats } from './frameStats.js';
import {
  parseColor,
  mixColor,
  nodeColor,
  readPalette,
  srgbToLinearChannel,
  colorToLinearBytes,
} from './palette.js';

const graph = {
  nodeCount: 4,
  edgeCount: 4,
  positions: Float32Array.from([0, 0, 0, 1, 0, 0, 2, 0, 0, 3, 0, 0]),
  edgeSource: Uint32Array.from([0, 0, 0, 1]),
  edgeTarget: Uint32Array.from([1, 2, 3, 2]),
  edgeType: Uint8Array.from([0, 0, 0, 0]),
};

describe('buildAdjacency', () => {
  it('lists both directions for every edge', () => {
    const { offsets, neighbours } = buildAdjacency(graph);
    const of = (i) => Array.from(neighbours.slice(offsets[i], offsets[i + 1])).sort();
    expect(of(0)).toEqual([1, 2, 3]);
    expect(of(1)).toEqual([0, 2]);
    expect(of(2)).toEqual([0, 1]);
    expect(of(3)).toEqual([0]);
  });

  it('keeps the edge index alongside each neighbour', () => {
    const { offsets, edgeOf, neighbours } = buildAdjacency(graph);
    for (let i = 0; i < graph.nodeCount; i++) {
      for (let k = offsets[i]; k < offsets[i + 1]; k++) {
        const e = edgeOf[k];
        const other = neighbours[k];
        const pair = [graph.edgeSource[e], graph.edgeTarget[e]];
        expect(pair).toContain(i);
        expect(pair).toContain(other);
      }
    }
  });

  it('handles a star without losing the hub degree', () => {
    const n = 1000;
    const star = {
      nodeCount: n,
      edgeCount: n - 1,
      positions: new Float32Array(n * 3),
      edgeSource: new Uint32Array(n - 1),
      edgeTarget: Uint32Array.from({ length: n - 1 }, (_, i) => i + 1),
      edgeType: new Uint8Array(n - 1),
    };
    const { offsets } = buildAdjacency(star);
    expect(offsets[1] - offsets[0]).toBe(n - 1);
  });
});

describe('edge buffers', () => {
  it('writes two vertices per edge in source-target order', () => {
    const pos = buildEdgePositions(graph);
    expect(pos).toHaveLength(24);
    expect(Array.from(pos.slice(0, 6))).toEqual([0, 0, 0, 1, 0, 0]);
    expect(Array.from(pos.slice(18, 24))).toEqual([1, 0, 0, 2, 0, 0]);
  });

  it('only needs a colour attribute once a second edge type appears', () => {
    expect(needsPerEdgeColor(Uint8Array.from([0, 0, 0]))).toBe(false);
    expect(needsPerEdgeColor(Uint8Array.from([0, 2, 0]))).toBe(true);
    expect(needsPerEdgeColor(new Uint8Array(0))).toBe(false);
  });

  it('gives both endpoints of an edge the same colour', () => {
    const palette = readPalette(null);
    const colors = buildEdgeColors({ ...graph, edgeType: Uint8Array.from([2, 0, 0, 0]) }, palette);
    expect(Array.from(colors.slice(0, 3))).toEqual(Array.from(colors.slice(3, 6)));
    expect(Array.from(colors.slice(0, 3))).not.toEqual(Array.from(colors.slice(6, 9)));
  });
});

describe('FrameStats', () => {
  it('reports the tail, not just the average', () => {
    const s = new FrameStats(100);
    for (let i = 0; i < 99; i++) s.push(10);
    s.push(500);
    const out = s.summary();
    expect(out.p50).toBe(10);
    expect(out.max).toBe(500);
    expect(out.fps).toBeCloseTo(100, 5);
  });

  it('turns timestamps into deltas', () => {
    const s = new FrameStats(10);
    s.mark(1000);
    s.mark(1016);
    s.mark(1032);
    expect(s.summary().p50).toBe(16);
  });

  it('is empty before the first frame', () => {
    expect(new FrameStats().summary()).toEqual({ count: 0, p50: 0, p95: 0, max: 0, fps: 0 });
  });
});

describe('palette', () => {
  it('parses the colour notations the tokens can produce', () => {
    expect(parseColor('#ff8800')).toBe(0xff8800);
    expect(parseColor('#f80')).toBe(0xff8800);
    expect(parseColor('rgb(255, 136, 0)')).toBe(0xff8800);
    expect(parseColor('oklch(0.7 0.1 200)', 0x123456)).toBe(0x123456);
    expect(parseColor('', 0x111111)).toBe(0x111111);
  });

  it('mixes towards the target colour', () => {
    expect(mixColor(0x000000, 0xffffff, 0)).toBe(0x000000);
    expect(mixColor(0x000000, 0xffffff, 1)).toBe(0xffffff);
    expect(mixColor(0x000000, 0xffffff, 0.5)).toBe(0x808080);
  });

  it('converts to the linear working space for raw attributes', () => {
    // Black and white are fixed points; everything in between darkens, which
    // is what stops the renderer converting the same colour twice.
    expect(srgbToLinearChannel(0)).toBe(0);
    expect(srgbToLinearChannel(1)).toBeCloseTo(1, 6);
    expect(srgbToLinearChannel(0.5)).toBeLessThan(0.5);
    expect(colorToLinearBytes(0xffffff)).toEqual([255, 255, 255]);
    expect(colorToLinearBytes(0x000000)).toEqual([0, 0, 0]);
    expect(colorToLinearBytes(0x808080)[0]).toBeLessThan(0x80);
  });

  it('separates the three enrichment statuses within one hue', () => {
    const palette = readPalette(null);
    const harvested = nodeColor(palette, 0, 0);
    const enriched = nodeColor(palette, 0, 1);
    const validated = nodeColor(palette, 0, 2);
    expect(harvested).not.toBe(enriched);
    expect(enriched).not.toBe(validated);
    // Harvested washes out towards the background, so it is the lightest.
    const lum = (c) => ((c >> 16) & 0xff) + ((c >> 8) & 0xff) + (c & 0xff);
    expect(lum(harvested)).toBeGreaterThan(lum(enriched));
    expect(lum(enriched)).toBeGreaterThan(lum(validated));
  });
});

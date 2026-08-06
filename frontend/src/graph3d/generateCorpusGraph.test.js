import { describe, it, expect } from 'vitest';
import { generateCorpusGraph, mulberry32 } from './generateCorpusGraph.js';
import { KIND_IDS, EDGE_TYPE_IDS, STATUS_IDS } from './graphSchema.js';

describe('mulberry32', () => {
  it('is deterministic and stays in [0, 1)', () => {
    const a = mulberry32(42);
    const b = mulberry32(42);
    for (let i = 0; i < 100; i++) {
      const v = a();
      expect(v).toBe(b());
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
    }
  });
});

describe('generateCorpusGraph', () => {
  it('produces the promised shape and sizes', () => {
    const g = generateCorpusGraph({ nodeCount: 500, edgeCount: 2000 });
    expect(g.nodeCount).toBe(500);
    expect(g.edgeCount).toBe(2000);
    expect(g.positions).toHaveLength(1500);
    expect(g.kind).toHaveLength(500);
    expect(g.weight).toHaveLength(500);
    expect(g.labels).toHaveLength(500);
    expect(g.edgeSource).toHaveLength(2000);
    expect(g.edgeTarget).toHaveLength(2000);
    expect(g.edgeType).toHaveLength(2000);
  });

  it('is reproducible for a seed and differs between seeds', () => {
    const a = generateCorpusGraph({ nodeCount: 200, edgeCount: 400, seed: 3 });
    const b = generateCorpusGraph({ nodeCount: 200, edgeCount: 400, seed: 3 });
    const c = generateCorpusGraph({ nodeCount: 200, edgeCount: 400, seed: 4 });
    expect(Array.from(a.positions)).toEqual(Array.from(b.positions));
    expect(Array.from(a.edgeSource)).toEqual(Array.from(b.edgeSource));
    expect(a.labels[7]).toBe(b.labels[7]);
    expect(Array.from(a.positions)).not.toEqual(Array.from(c.positions));
  });

  it('keeps every field inside its declared domain', () => {
    const g = generateCorpusGraph({ nodeCount: 300, edgeCount: 900, seed: 11 });
    const maxKind = Math.max(...Object.values(KIND_IDS));
    const maxStatus = Math.max(...Object.values(STATUS_IDS));
    for (let i = 0; i < g.nodeCount; i++) {
      expect(g.kind[i]).toBeLessThanOrEqual(maxKind);
      expect(g.status[i]).toBeLessThanOrEqual(maxStatus);
      expect(Number.isFinite(g.positions[i * 3])).toBe(true);
      expect(g.weight[i]).toBeGreaterThan(0);
    }
    for (let e = 0; e < g.edgeCount; e++) {
      expect(g.edgeType[e]).toBe(EDGE_TYPE_IDS.citation);
      expect(g.edgeSource[e]).toBeLessThan(g.nodeCount);
      expect(g.edgeTarget[e]).toBeLessThan(g.nodeCount);
      expect(g.edgeSource[e]).not.toBe(g.edgeTarget[e]);
    }
  });

  it('gives the framework laws a star of edges', () => {
    const g = generateCorpusGraph({
      nodeCount: 2000,
      edgeCount: 20000,
      hubs: 1,
      hubShare: 0.2,
      seed: 5,
    });
    let hubDegree = 0;
    for (let e = 0; e < g.edgeCount; e++) {
      if (g.edgeSource[e] === 0 || g.edgeTarget[e] === 0) hubDegree++;
    }
    // Roughly a fifth of all edges start at the single hub, which is the
    // Awb-shaped star the renderer has to survive.
    expect(hubDegree).toBeGreaterThan(3000);
  });

  it('can skip the label strings for the very large sizes', () => {
    const g = generateCorpusGraph({ nodeCount: 1000, edgeCount: 10, labels: false });
    expect(g.labels).toBeNull();
  });

  it('rejects nonsense sizes instead of allocating them', () => {
    expect(() => generateCorpusGraph({ nodeCount: 0, edgeCount: 10 })).toThrow();
    expect(() => generateCorpusGraph({ nodeCount: 10, edgeCount: -1 })).toThrow();
  });
});

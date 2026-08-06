import { describe, it, expect } from 'vitest';
import { packGraph } from './packGraph.js';
import { KIND_IDS, STATUS_IDS, EDGE_TYPE_IDS } from './graphSchema.js';

const graph = {
  nodes: [
    { id: 'wet_a', label: 'Wet A', x: 1, y: 2, z: 3, kind: 'law', weight: 4 },
    { id: 'wet_b', label: 'Wet B', x: -1, y: 0, z: 5, kind: 'amvb', weight: 1 },
    { id: 'wet_c', label: 'Wet C', x: 0, y: 0, z: 0, kind: 'onbekend' },
  ],
  edges: [
    { source: 'wet_a', target: 'wet_b', type: 'citation' },
    { source: 'wet_b', target: 'wet_c', type: 'delegation' },
    { source: 'wet_a', target: 'wet_weg', type: 'citation' },
  ],
};

describe('packGraph', () => {
  it('packs the fixed node fields into typed arrays', () => {
    const p = packGraph(graph);
    expect(p.nodeCount).toBe(3);
    expect(Array.from(p.positions.slice(0, 3))).toEqual([1, 2, 3]);
    expect(p.kind[0]).toBe(KIND_IDS.law);
    expect(p.kind[1]).toBe(KIND_IDS.amvb);
    expect(p.labels[1]).toBe('Wet B');
    expect(p.ids[2]).toBe('wet_c');
    expect(p.weight[0]).toBe(4);
  });

  it('falls back instead of throwing on an unknown kind or status', () => {
    const p = packGraph(graph);
    expect(p.kind[2]).toBe(KIND_IDS.law);
    expect(p.status[2]).toBe(STATUS_IDS.harvested);
    expect(p.weight[2]).toBe(1);
  });

  it('resolves edges to indices and drops dangling ones', () => {
    const p = packGraph(graph);
    expect(p.edgeCount).toBe(2);
    expect(p.dropped).toBe(1);
    expect(Array.from(p.edgeSource)).toEqual([0, 1]);
    expect(Array.from(p.edgeTarget)).toEqual([1, 2]);
    expect(p.edgeType[1]).toBe(EDGE_TYPE_IDS.delegation);
  });

  it('survives an empty payload', () => {
    const p = packGraph({});
    expect(p.nodeCount).toBe(0);
    expect(p.edgeCount).toBe(0);
  });
});

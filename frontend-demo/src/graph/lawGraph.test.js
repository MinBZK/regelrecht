import { describe, expect, it } from 'vitest';
import { buildGraph, colourIndex, itemId, lawShape, lawSize, layout, neighbourhood, LAW_W } from './lawGraph.js';

const law = (id, { sources = [], inputs = [], outputs = [] } = {}) => ({
  id,
  name: id,
  service: 'X',
  doc: {
    articles: [
      {
        machine_readable: {
          execution: {
            input: [
              ...sources.map((name) => ({ name, source: {} })),
              ...inputs.map(([name, regulation, output]) => ({ name, source: { regulation, output } })),
            ],
            output: outputs.map((name) => ({ name })),
          },
        },
      },
    ],
  },
});

describe('lawShape', () => {
  it('splits register sources from cross-law inputs and dedupes by name', () => {
    const l = law('a', { sources: ['polis'], inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['toeslag'] });
    l.doc.articles.push({ machine_readable: { execution: { input: [{ name: 'polis', source: {} }], output: [{ name: 'toeslag' }] } } });
    const shape = lawShape(l);
    expect(shape.sources.map((s) => s.name)).toEqual(['polis']);
    expect(shape.inputs).toEqual([{ name: 'leeftijd', ref: { regulation: 'brp', output: 'leeftijd' } }]);
    expect(shape.outputs.map((o) => o.name)).toEqual(['toeslag']);
  });
});

describe('layout', () => {
  it('puts a law right of the laws it reads from', () => {
    const size = lawSize(lawShape(law('x', { outputs: ['o'] })));
    const positions = layout(
      new Map([
        ['consumer', { size, deps: ['supplier'] }],
        ['supplier', { size, deps: [] }],
      ]),
    );
    expect(positions.get('supplier').x).toBe(0);
    expect(positions.get('consumer').x).toBeGreaterThan(LAW_W);
  });
});

describe('buildGraph', () => {
  const brp = law('brp', { sources: ['geboortedatum'], outputs: ['leeftijd'] });
  const zt = law('zt', { inputs: [['leeftijd', 'brp', 'leeftijd'], ['inkomen', 'ib', 'inkomen']], outputs: ['hoogte'] });

  it('draws an edge from an input to the output that supplies it, and none to an absent law', () => {
    const { nodes, edges } = buildGraph([brp, zt]);
    expect(edges).toHaveLength(1);
    expect(edges[0].source).toBe(itemId('zt', 'in', 'leeftijd'));
    expect(edges[0].target).toBe(itemId('brp', 'out', 'leeftijd'));
    expect(nodes.filter((n) => n.type === 'law')).toHaveLength(2);
    expect(nodes.find((n) => n.id === itemId('brp', 'src', 'geboortedatum')).parentNode).toBe('brp::box::sources');
  });

  it('shows the persona values on the items and dims laws outside the focus', () => {
    const values = { outputs: { zt: { hoogte: '€ 1.654' } }, sources: { brp: { geboortedatum: '1989' } } };
    const ib = law('ib', { outputs: ['inkomen'] });
    const { nodes, edges } = buildGraph([brp, zt, ib], values, 'brp');
    expect(nodes.find((n) => n.id === itemId('zt', 'out', 'hoogte')).data.value).toBe('€ 1.654');
    expect(nodes.find((n) => n.id === 'ib').class).toMatch(/graph-dim/);
    expect(nodes.find((n) => n.id === 'zt').class).not.toMatch(/graph-dim/);
    expect(edges.find((e) => e.data.to === 'ib').style.opacity).toBeLessThan(0.5);
  });
});

describe('neighbourhood', () => {
  it('adds what a selected law reads from and what reads from it, but not further', () => {
    const brp = law('brp', { outputs: ['leeftijd'] });
    const zvw = law('zvw', { inputs: [['detentie', 'pbw', 'detentie']], outputs: ['verzekerd'] });
    const pbw = law('pbw', { outputs: ['detentie'] });
    const zt = law('zt', { inputs: [['leeftijd', 'brp', 'leeftijd'], ['verzekerd', 'zvw', 'verzekerd']], outputs: ['hoogte'] });
    const ww = law('ww', { inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['uitkering'] });
    const shown = neighbourhood(new Set(['zt']), [brp, zvw, pbw, zt, ww]);
    expect([...shown].sort()).toEqual(['brp', 'zt', 'zvw']);
  });
});

describe('colourIndex', () => {
  it('gives a service a stable colour and wraps after seven', () => {
    const idx = colourIndex(['A', 'B', 'A', 'C', 'D', 'E', 'F', 'G', 'H']);
    expect(idx('A')).toBe(0);
    expect(idx('B')).toBe(1);
    expect(idx('H')).toBe(0);
  });
});

describe('buildGraph with a visible set', () => {
  it('keeps every law in place and only hides what is outside the set', () => {
    const brp = law('brp', { outputs: ['leeftijd'] });
    const zt = law('zt', { inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['hoogte'] });
    const ww = law('ww', { inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['uitkering'] });
    const all = buildGraph([brp, zt, ww]);
    const some = buildGraph([brp, zt, ww], {}, null, new Set(['brp', 'zt']));
    expect(some.nodes.find((n) => n.id === 'zt').position).toEqual(all.nodes.find((n) => n.id === 'zt').position);
    expect(some.nodes.find((n) => n.id === 'ww').hidden).toBe(true);
    expect(some.nodes.find((n) => n.id === 'zt').hidden).toBe(false);
    expect(some.edges.find((e) => e.data.from === 'ww').hidden).toBe(true);
    expect(some.edges.find((e) => e.data.from === 'zt').hidden).toBe(false);
  });
});

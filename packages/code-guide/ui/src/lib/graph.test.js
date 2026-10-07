import { describe, expect, it } from 'vitest';
import {
  MAX_NODE_WIDTH,
  MAX_PER_ROW,
  NODE_WIDTH,
  buildGraph,
  crateGraph,
  defaultScope,
  layoutLayers,
  moduleGraph,
  nodeWidthFor,
  perRowFor,
  typeGraph,
  workspaceGraph,
} from './graph.js';

const link = (crate, module, calls = 1) => ({ crate, module, calls });
const mod = (path, layer, callsInto = [], calledFrom = [], extra = {}) => ({
  path,
  layer,
  vis: 'pub',
  cycleWith: [],
  callsInto,
  calledFrom,
  types: [],
  functions: [],
  ...extra,
});

// Crate `x`: service calls engine and types; engine calls types; wasm calls
// service; service also calls into another crate and is called from one.
const crate = {
  name: 'x',
  modules: [
    mod('types', 0, [], [link('x', 'engine'), link('x', 'service')]),
    mod('engine', 1, [link('x', 'types', 3)], [link('x', 'service')]),
    mod('service', 2, [link('x', 'engine', 5), link('x', 'types'), link('model', 'law', 7)], [link('x', 'wasm'), link('app', 'main', 2)]),
    mod('wasm', 3, [link('x', 'service')]),
  ],
};
const pairs = (edges) => edges.map((e) => `${e.source}>${e.target}`).sort();

describe('workspaceGraph', () => {
  it('has a node per crate and an edge per crate-to-crate call', () => {
    const g = workspaceGraph({
      crates: [{ name: 'a', layer: 1, cycleWith: [] }, { name: 'b', layer: 0, cycleWith: [] }],
      edges: [{ from: 'a', to: 'b', calls: 4 }],
    });
    expect(g.nodes.map((n) => [n.id, n.layer])).toEqual([['a', 1], ['b', 0]]);
    expect(g.edges).toEqual([{ id: 'a->b', source: 'a', target: 'b', calls: 4 }]);
    expect(g.nodes[0].target).toEqual({ crate: 'a', module: null });
  });
});

describe('crateGraph', () => {
  it('draws the crate\'s modules and only the calls between them', () => {
    const g = crateGraph(crate);
    expect(g.nodes.map((n) => n.id)).toEqual(['types', 'engine', 'service', 'wasm']);
    expect(pairs(g.edges)).toEqual(['engine>types', 'service>engine', 'service>types', 'wasm>service']);
  });
});

describe('the crate root and binaries', () => {
  // Vue Flow drops an edge whose end has an empty id, so a module's id must
  // never be its path: the crate root's path is "".
  const rooted = {
    name: 'r',
    modules: [
      mod('', 1, [link('r', 'util', 2)], [link('r', 'bin:tool')]),
      mod('util', 0, [], [link('r', '')]),
      mod('bin:tool', 2, [link('r', '')]),
    ],
  };

  it('gives every node a non-empty id and keeps the root\'s edges', () => {
    const g = crateGraph(rooted);
    expect(g.nodes.every((n) => n.id)).toBe(true);
    expect(pairs(g.edges)).toEqual(['bin~tool>~', '~>util']);
    expect(g.nodes.find((n) => n.id === '~').target).toEqual({ crate: 'r', module: '' });
  });

  it('keeps the root\'s edges around it too, and marks it open', () => {
    const g = buildGraph({ scope: 'module', crateView: rooted, route: { crate: 'r', module: '' }, hrefFor: () => '#' });
    expect(g.nodes.every((n) => n.id)).toBe(true);
    expect(g.edges).toHaveLength(2);
    expect(g.nodes.filter((n) => n.selected).map((n) => n.id)).toEqual(['~']);
    const c = buildGraph({ scope: 'crate', crateView: rooted, route: { crate: 'r', module: '' }, hrefFor: () => '#' });
    expect(c.nodes.filter((n) => n.selected).map((n) => n.id)).toEqual(['~']);
  });
});

describe('moduleGraph', () => {
  it('puts what the module calls below it and what calls it above, across crates', () => {
    const g = moduleGraph(crate, 'service');
    const byId = Object.fromEntries(g.nodes.map((n) => [n.id, n]));
    expect(byId.service.role).toBe('self');
    expect([byId.engine.layer, byId.service.layer, byId.wasm.layer]).toEqual([0, 1, 2]);
    expect(byId['model|law'].label).toBe('model::law');
    expect(byId['app|main'].target).toEqual({ crate: 'app', module: 'main' });
    expect(byId['app|main'].role).toBe('caller');
    expect(byId['model|law'].target).toEqual({ crate: 'model', module: 'law' });
  });

  it('keeps only the edges that touch the module', () => {
    expect(pairs(moduleGraph(crate, 'service').edges)).toEqual([
      'app|main>service',
      'service>engine',
      'service>model|law',
      'service>types',
      'wasm>service',
    ]);
  });

  it('is empty for a module that does not exist', () => {
    expect(moduleGraph(crate, 'nope')).toEqual({ nodes: [], edges: [] });
  });
});

describe('typeGraph', () => {
  const m = (key, vis = 'pub') => ({ key, vis });
  const type = {
    crate: 'x',
    module: 'service',
    name: 'S',
    methods: [m('run'), m('step'), m('load'), m('alone'), m('helper', 'private'), m('inner', 'restricted')],
    graph: {
      edges: [
        { from: 'run', to: 'step', via: [] },
        { from: 'step', to: 'load', via: ['prepare'] },
      ],
      layers: [[['alone'], ['inner'], ['load']], [['step']], [['run']]],
      external: [{ crate: 'app', module: 'main', targets: { run: 2, load: 1 } }],
    },
  };

  it('draws the connected public methods and the outside callers on a row above', () => {
    const g = typeGraph(type);
    const byId = Object.fromEntries(g.nodes.map((n) => [n.id, n]));
    expect(Object.keys(byId).sort()).toEqual(['app|main', 'load', 'run', 'step']);
    expect(byId['app|main'].layer).toBeGreaterThan(byId.run.layer);
    expect(byId['app|main'].external).toBe(true);
    expect(byId.run.target).toEqual({ crate: 'x', module: 'service', item: 'S', method: 'run' });
  });

  it('keeps the helpers a call goes through, and counts outside calls per method', () => {
    const g = typeGraph(type);
    expect(g.edges.find((e) => e.source === 'step').via).toEqual(['prepare']);
    expect(g.edges.filter((e) => e.source === 'app|main').map((e) => [e.target, e.calls]).sort()).toEqual([
      ['load', 1],
      ['run', 2],
    ]);
  });

  it('lists methods without calls apart, pub(crate) included, and never a private one', () => {
    expect(typeGraph(type).unconnected).toEqual(['alone', 'inner']);
  });
});

describe('defaultScope', () => {
  it('shows the level that is open', () => {
    expect(defaultScope({ crate: null, module: null })).toBe('workspace');
    expect(defaultScope({ crate: 'x', module: null })).toBe('crate');
    expect(defaultScope({ crate: 'x', module: '' })).toBe('module');
    expect(defaultScope({ crate: 'x', module: 'a' }, true)).toBe('type');
  });
});

describe('buildGraph', () => {
  const hrefFor = (t) => `#/${t.crate}/${t.module ?? ''}`;

  it('marks the open module in the crate scope and links every node', () => {
    const g = buildGraph({ scope: 'crate', crateView: crate, route: { crate: 'x', module: 'engine' }, hrefFor });
    expect(g.nodes.filter((n) => n.selected).map((n) => n.id)).toEqual(['engine']);
    expect(g.nodes.every((n) => n.href && n.position)).toBe(true);
  });

  it('marks the module itself in its neighbourhood', () => {
    const g = buildGraph({ scope: 'module', crateView: crate, route: { crate: 'x', module: 'service' }, hrefFor });
    expect(g.nodes.filter((n) => n.selected).map((n) => n.id)).toEqual(['service']);
  });

  it('is empty, not an error, while the data is loading', () => {
    for (const scope of ['workspace', 'crate', 'module', 'type']) {
      const g = buildGraph({ scope, route: { crate: 'x', module: 'a' }, hrefFor });
      expect(g.nodes).toEqual([]);
    }
  });
});

describe('node width and row length', () => {
  it('keeps short labels at the narrowest width and widens for long ones, capped', () => {
    expect(nodeWidthFor(['types', 'engine'])).toBe(NODE_WIDTH);
    expect(nodeWidthFor(['evaluate_law_output_with_trace'])).toBeGreaterThan(NODE_WIDTH);
    expect(nodeWidthFor(['x'.repeat(200)])).toBe(MAX_NODE_WIDTH);
    expect(nodeWidthFor([])).toBe(NODE_WIDTH);
  });

  it('fits fewer wide nodes in a row, never fewer than three', () => {
    expect(perRowFor(NODE_WIDTH)).toBe(MAX_PER_ROW);
    expect(perRowFor(MAX_NODE_WIDTH)).toBeLessThan(MAX_PER_ROW);
    expect(perRowFor(5000)).toBe(3);
  });
});

describe('layoutLayers', () => {
  const nodes = crateGraph(crate).nodes;
  const edges = crateGraph(crate).edges;

  it('puts each layer in its own row, callees at the bottom', () => {
    const pos = layoutLayers(nodes, edges);
    const y = (id) => pos.get(id).y;
    expect(y('types')).toBeGreaterThan(y('engine'));
    expect(y('engine')).toBeGreaterThan(y('service'));
    expect(y('service')).toBeGreaterThan(y('wasm'));
  });

  it('never overlaps two nodes and has no negative coordinates', () => {
    const pos = layoutLayers(nodes, edges);
    expect(new Set([...pos.values()].map((p) => `${p.x},${p.y}`)).size).toBe(pos.size);
    for (const { x, y } of pos.values()) expect(Math.min(x, y)).toBeGreaterThanOrEqual(0);
  });

  it('does not depend on input order', () => {
    const a = layoutLayers(nodes, edges);
    const b = layoutLayers([...nodes].reverse(), [...edges].reverse());
    expect([...a.entries()].sort()).toEqual([...b.entries()].sort());
  });

  it('wraps a long layer into evenly filled rows within the limit', () => {
    const wide = Array.from({ length: 13 }, (_, i) => ({ id: `d${String(i).padStart(2, '0')}`, layer: 0 }));
    const pos = layoutLayers([...wide, { id: 'top', layer: 1 }], wide.map((d) => ({ id: `t-${d.id}`, source: 'top', target: d.id })));
    const sizes = new Map();
    for (const d of wide) sizes.set(pos.get(d.id).y, (sizes.get(pos.get(d.id).y) ?? 0) + 1);
    expect([...sizes.values()].sort()).toEqual([4, 4, 5]);
    for (const d of wide) expect(pos.get('top').y).toBeLessThan(pos.get(d.id).y);
  });

  it('copes with an empty graph and with one node', () => {
    expect(layoutLayers([], []).size).toBe(0);
    expect([...layoutLayers([{ id: 'only', layer: 3 }], []).values()]).toEqual([{ x: 0, y: 0 }]);
  });
});

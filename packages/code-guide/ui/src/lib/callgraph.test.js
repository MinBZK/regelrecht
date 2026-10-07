import { describe, expect, it } from 'vitest';
import {
  MAX_NODES,
  defaultDepth,
  boundaryOf,
  edgeId,
  expand,
  focusOf,
  highlight,
  itemKind,
  layoutOptions,
  levelFor,
  levelsFor,
  lift,
  neighbourhood,
  nodeWidth,
  pathsBetween,
  toElements,
  unitsOf,
  bestWrap,
  CATEGORIES,
  legendOf,
  wrapRanks,
} from './callgraph.js';
import { createCy } from './cy.js';

const fn = (crate, module, key, type = null, extra = {}) => ({
  crate,
  module,
  type,
  key,
  name: key.split('@')[0].split('::').pop(),
  vis: 'pub',
  doc: null,
  stale: false,
  ...extra,
});

// Crate `app` calls into crate `eng`:
//   app::main            -> eng::service::Svc::run
//   eng::service::Svc::run -> eng::service::Svc::step (private) -> eng::rules::eval
//   eng::service::Svc::run -> eng::rules::eval   (twice)
//   eng::rules::eval      -> eng::rules::eval   (recursion)
//   eng::rules::parse     (nothing calls it, it calls nothing)
//   eng::service::Svc::fmt@Display -> eng::service::Svc::run
const data = {
  nodes: [
    fn('app', 'bin:app', 'main'), // 0
    fn('eng', 'service', 'run', 'Svc'), // 1
    fn('eng', 'service', 'step', 'Svc', { vis: 'private' }), // 2
    fn('eng', 'rules', 'eval'), // 3
    fn('eng', 'rules', 'parse'), // 4
    fn('eng', 'service', 'fmt@Display', 'Svc', { vis: 'trait' }), // 5
  ],
  edges: [
    [0, 1, 1],
    [1, 2, 1],
    [2, 3, 1],
    [1, 3, 2],
    [3, 3, 1],
    [5, 1, 1],
  ],
};
const ids = (set) => [...set].sort();
const route = (crate, module = null, item = null, method = null) => ({ crate, module, item, method });

describe('levels and depth', () => {
  it('offers finer levels further down, the finest as the default', () => {
    expect(levelsFor('workspace')).toEqual(['crates']);
    expect(levelsFor('crate')).toEqual(['modules', 'crates']);
    expect(levelsFor('type')).toEqual(['functions', 'modules', 'crates']);
    expect(levelFor('crate', 'functions')).toBe('modules');
    expect(levelFor('module', 'crates')).toBe('crates');
    expect(levelFor('module', null)).toBe('functions');
  });

  it('reaches further around a single function than around a page full', () => {
    expect(defaultDepth('functions', 'function')).toBe(2);
    expect(defaultDepth('functions', 'module')).toBe(1);
    expect(defaultDepth('modules', 'crate')).toBe(0);
    expect(defaultDepth('crates', 'workspace')).toBe(0);
  });
});

describe('lift', () => {
  it('has a node per function, labelled with its type and trait, linked to its page', () => {
    const g = lift(data, 'functions');
    expect(g.nodes.size).toBe(6);
    const fmt = g.nodes.get('f5');
    expect(fmt.label).toBe('Svc::fmt');
    expect(fmt.sub).toBe('impl Display · eng::service');
    expect(g.nodes.get('f0').sub).toBe('app::bin app');
    expect(fmt.target).toEqual({ crate: 'eng', module: 'service', item: 'Svc', method: 'fmt@Display' });
    expect(g.nodes.get('f3').target).toEqual({ crate: 'eng', module: 'rules', item: 'eval', method: null });
    expect(g.out.get('f1').get('f3')).toBe(2);
    expect(g.in.get('f3').get('f3')).toBe(1);
  });

  it('counts a method under the module its impl is written in, and addresses it under its type\'s', () => {
    const moved = {
      nodes: [fn('eng', 'service', 'run', 'Svc', { implModule: 'service::run' }), fn('eng', 'rules', 'eval')],
      edges: [[0, 1, 1]],
    };
    const f = lift(moved, 'functions').nodes.get('f0');
    expect(f.sub).toBe('eng::service::run');
    expect(f.target.module).toBe('service');
    expect([...lift(moved, 'modules').out.get('m:eng|service::run').keys()]).toEqual(['m:eng|rules']);
    const g = lift(moved, 'functions');
    expect(ids(focusOf(g, route('eng', 'service::run'), 'module'))).toEqual(['f0']);
    expect(ids(focusOf(g, route('eng', 'service'), 'module'))).toEqual([]);
    expect(ids(focusOf(g, route('eng', 'service', 'Svc'), 'type'))).toEqual(['f0']);
    expect(itemKind(moved, route('eng', 'service', 'Svc'))).toBe('type');
    expect(itemKind(moved, route('eng', 'rules', 'eval'))).toBe('function');
  });

  it('knows what source to show for a function, a module and a crate', () => {
    const withSource = {
      nodes: [
        fn('eng', 'rules', 'eval', null, { place: { file: 'eng/src/rules.rs', line: 12 }, extent: [10, 30] }),
        fn('eng', 'rules', 'parse', null, { place: { file: 'eng/src/rules.rs', line: 40 }, extent: null }),
      ],
      edges: [],
      modules: [
        { crate: 'eng', module: 'rules', file: 'eng/src/rules.rs' },
        { crate: 'eng', module: '', file: 'eng/src/lib.rs' },
      ],
    };
    const f = lift(withSource, 'functions').nodes;
    expect(f.get('f0').source).toEqual({ path: 'eng/src/rules.rs', from: 10, to: 30 });
    expect(f.get('f1').source).toEqual({ path: 'eng/src/rules.rs', from: 40, to: 80 });
    expect(lift(withSource, 'modules').nodes.get('m:eng|rules').source.path).toBe('eng/src/rules.rs');
    expect(lift(withSource, 'crates').nodes.get('c:eng').source.path).toBe('eng/src/lib.rs');
  });

  it('labels a binary\'s module the same way at every level', () => {
    const bin = { nodes: [fn('p', 'bin:api', 'main')], edges: [] };
    expect(lift(bin, 'modules').nodes.get('m:p|bin:api').label).toBe('p::bin api');
  });

  it('sums calls per module and per crate, and leaves out calls within one', () => {
    const m = lift(data, 'modules');
    expect(ids(m.nodes.keys())).toEqual(['m:app|bin:app', 'm:eng|rules', 'm:eng|service']);
    expect(m.out.get('m:eng|service').get('m:eng|rules')).toBe(3);
    expect(m.out.get('m:eng|rules').size).toBe(0);
    expect(m.nodes.get('m:eng|service').target).toEqual({ crate: 'eng', module: 'service', item: null, method: null });
    const c = lift(data, 'crates');
    expect([...c.out.get('c:app')]).toEqual([['c:eng', 1]]);
    expect(c.out.get('c:eng').size).toBe(0);
  });
});

describe('focusOf', () => {
  const g = lift(data, 'functions');
  it('is the function, the method, the type\'s methods or the module\'s functions', () => {
    expect(ids(focusOf(g, route('eng', 'rules', 'eval'), 'function'))).toEqual(['f3']);
    expect(ids(focusOf(g, route('eng', 'service', 'Svc', 'run'), 'type'))).toEqual(['f1']);
    expect(ids(focusOf(g, route('eng', 'service', 'Svc'), 'type'))).toEqual(['f1', 'f2', 'f5']);
    expect(ids(focusOf(g, route('eng', 'rules'), 'module'))).toEqual(['f3', 'f4']);
  });

  it('is the module or the crate\'s modules, the crate or every crate', () => {
    const m = lift(data, 'modules');
    expect(ids(focusOf(m, route('eng', 'rules'), 'module'))).toEqual(['m:eng|rules']);
    expect(ids(focusOf(m, route('eng'), 'crate'))).toEqual(['m:eng|rules', 'm:eng|service']);
    const c = lift(data, 'crates');
    expect(ids(focusOf(c, route('eng'), 'crate'))).toEqual(['c:eng']);
    expect(ids(focusOf(c, route(null), 'workspace'))).toEqual(['c:app', 'c:eng']);
  });
});

describe('neighbourhood', () => {
  const g = lift(data, 'functions');
  const run = new Set(['f1']);

  it('takes callers above (negative) and callees below (positive), to the depth', () => {
    const p = neighbourhood(g, run, { calls: 'both', depth: 1 });
    expect(Object.fromEntries(p.dist)).toEqual({ f1: 0, f2: 1, f3: 1, f0: -1, f5: -1 });
    expect(p.edges.map((e) => e.id).sort()).toEqual(
      ['f0>f1', 'f1>f2', 'f1>f3', 'f2>f3', 'f3>f3', 'f5>f1'].sort(),
    );
    expect(p.edges.find((e) => e.id === 'f1>f3').sites).toBe(2);
  });

  it('follows only callers or only callees on request', () => {
    expect(ids(neighbourhood(g, run, { calls: 'in', depth: 3 }).dist.keys())).toEqual(['f0', 'f1', 'f5']);
    expect(ids(neighbourhood(g, run, { calls: 'out', depth: 3 }).dist.keys())).toEqual(['f1', 'f2', 'f3']);
  });

  it('is only the focus at depth 0, with the calls among it', () => {
    const p = neighbourhood(g, new Set(['f1', 'f2']), { depth: 0 });
    expect(ids(p.dist.keys())).toEqual(['f1', 'f2']);
    expect(p.edges.map((e) => e.id)).toEqual(['f1>f2']);
  });

  it('keeps the nearest nodes past the limit and counts the rest', () => {
    const many = { nodes: [fn('x', 'm', 'hub')], edges: [] };
    for (let i = 1; i <= MAX_NODES + 50; i++) {
      many.nodes.push(fn('x', 'm', `leaf${i}`));
      many.edges.push([0, i, 1]);
    }
    const h = lift(many, 'functions');
    const p = neighbourhood(h, new Set(['f0']), { depth: 1 });
    expect(p.dist.size).toBe(MAX_NODES);
    expect(p.dist.has('f0')).toBe(true);
    expect(p.omitted).toBe(51);
    expect(p.edges.every((e) => p.dist.has(e.source) && p.dist.has(e.target))).toBe(true);
  });
});

describe('paths and highlighting', () => {
  const g = lift(data, 'functions');
  const part = neighbourhood(g, new Set(['f1']), { depth: 3 });

  it('finds every node and call on some path, through cycles', () => {
    const p = pathsBetween(part, new Set(['f0']), new Set(['f3']));
    expect(ids(p.nodes)).toEqual(['f0', 'f1', 'f2', 'f3']);
    expect(ids(p.edges)).toEqual(['f0>f1', 'f1>f2', 'f1>f3', 'f2>f3', 'f3>f3']);
  });

  it('highlights the paths between the focus and a selected node, whichever way they run', () => {
    const down = highlight(part, { selected: ['f3'], focus: new Set(['f1']) });
    expect(ids(down.nodes)).toEqual(['f1', 'f2', 'f3']);
    expect(down.connected).toBe(true);
    const up = highlight(part, { selected: ['f0'], focus: new Set(['f1']) });
    expect(ids(up.edges)).toEqual([edgeId('f0', 'f1')]);
  });

  it('highlights the paths between two selected nodes', () => {
    const h = highlight(part, { selected: ['f5', 'f2'], focus: new Set(['f1']) });
    expect(ids(h.nodes)).toEqual(['f1', 'f2', 'f5']);
    expect(ids(h.edges)).toEqual(['f1>f2', 'f5>f1']);
  });

  it('highlights the open function\'s own calls when it is the one selected', () => {
    const h = highlight(part, { selected: ['f1'], focus: new Set(['f1']) });
    expect(ids(h.edges)).toEqual(['f0>f1', 'f1>f2', 'f1>f3', 'f5>f1']);
    expect(h.connected).toBe(true);
  });

  it('says so when there is no path, and still marks the selection', () => {
    const h = highlight(part, { selected: ['f0', 'f5'], focus: new Set(['f1']) });
    expect(h.connected).toBe(false);
    expect(ids(h.nodes)).toEqual(['f0', 'f5']);
  });

  it('highlights everything leading to, or following from, the selection', () => {
    expect(ids(highlight(part, { mode: 'in', selected: ['f2'] }).nodes)).toEqual(['f0', 'f1', 'f2', 'f5']);
    const out = highlight(part, { mode: 'out', selected: ['f2'] });
    expect(ids(out.nodes)).toEqual(['f2', 'f3']);
    expect(ids(out.edges)).toEqual(['f2>f3', 'f3>f3']);
  });

  it('is nothing without a selection in the graph', () => {
    expect(highlight(part, { selected: [] })).toBeNull();
    expect(highlight(part, { selected: ['f4'] })).toBeNull();
  });
});

describe('adding single functions to a module or crate graph', () => {
  const m = lift(data, 'modules');
  const part = neighbourhood(m, new Set(['m:eng|service']), { depth: 1 });
  const edges = (p) => p.edges.map((e) => `${e.id}:${e.sites}`).sort();
  const pick = (entries) => new Map(entries.map(([k, v]) => [k, new Set(v)]));

  it('lists the functions behind a node\'s calls, busiest first', () => {
    // run: called from app (1), calls rules (2); step: calls rules (1). fmt only calls inside.
    expect(boundaryOf(data, m, part, 'm:eng|service')).toEqual([
      { id: 'f1', in: 1, out: 2 },
      { id: 'f2', in: 0, out: 1 },
    ]);
    expect(boundaryOf(data, m, part, 'm:eng|rules').map((x) => x.id)).toEqual(['f3']);
  });

  it('changes nothing with nothing added, or at the functions level', () => {
    expect(edges(expand(data, m, part, new Map()))).toEqual(edges(part));
    const f = lift(data, 'functions');
    const fp = neighbourhood(f, new Set(['f1']), { depth: 1 });
    expect(expand(data, f, fp, pick([['f1', ['f1']]])).parents.size).toBe(0);
  });

  it('runs only the added function\'s calls through it, the rest stays on its node', () => {
    const x = expand(data, m, part, pick([['m:eng|service', ['f2']]]));
    // step's call to rules goes through step; run's calls stay on the module.
    expect(edges(x)).toEqual(['f2>m:eng|rules:1', 'm:app|bin:app>m:eng|service:1', 'm:eng|service>m:eng|rules:2']);
    expect([...x.parents]).toEqual([['f2', 'm:eng|service']]);
    // The module stays a node: it is the box, and keeps its own edges.
    expect(x.dist.get('m:eng|service')).toBe(0);
    expect(x.dist.get('f2')).toBe(0);
  });

  it('draws a call between two added functions of one node, not one to the node itself', () => {
    const x = expand(data, m, part, pick([['m:eng|service', ['f1', 'f2']]]));
    expect(edges(x)).toEqual(['f1>f2:1', 'f1>m:eng|rules:2', 'f2>m:eng|rules:1', 'm:app|bin:app>f1:1']);
    const one = expand(data, m, part, pick([['m:eng|service', ['f1']]]));
    // run calls step, which is not added: that call is inside the box.
    expect(edges(one).some((e) => e.startsWith('f1>m:eng|service'))).toBe(false);
  });

  it('lets a box stand for itself and what was added in it, in a selection', () => {
    const x = expand(data, m, part, pick([['m:eng|service', ['f2']]]));
    expect(ids(unitsOf(x, ['m:eng|service']))).toEqual(['f2', 'm:eng|service']);
    const h = highlight(x, { selected: ['m:eng|rules'], focus: new Set(['m:eng|service']) });
    expect(ids(h.nodes)).toEqual(['f2', 'm:eng|rules', 'm:eng|service']);
  });

  it('draws the node as a box around what was added, which then leaves out its module', () => {
    const x = expand(data, m, part, pick([['m:eng|service', ['f1']]]));
    const f = lift(data, 'functions');
    const lookup = (id) => m.nodes.get(id) ?? f.nodes.get(id);
    const els = toElements(lookup, x, new Set(['m:eng|service']), ['app', 'eng']);
    const byId = Object.fromEntries(els.map((e) => [e.data.id, e]));
    expect(byId['m:eng|service'].classes).toBe('box focus');
    expect(byId['m:eng|service'].data.width).toBeUndefined();
    expect(byId.f1.data.parent).toBe('m:eng|service');
    expect(byId.f1.data.display).toBe('Svc::run');
    expect(els.filter((e) => e.data.id === 'm:eng|service')).toHaveLength(1);
  });

  // Force-directed (fcose) is left out here: headless, a box has no size and
  // fcose fails on it. In a browser it works; the browser suite checks it.
  it('lays out boxes in the layered and ring layouts', () => {
    const x = expand(data, m, part, pick([['m:eng|service', ['f1', 'f2']]]));
    const f = lift(data, 'functions');
    const els = toElements((id) => m.nodes.get(id) ?? f.nodes.get(id), x, new Set(), ['app', 'eng']);
    for (const name of ['down', 'right', 'rings']) {
      const cy = createCy({ elements: els });
      cy.layout({ ...layoutOptions(name), fit: false }).run();
      const box = cy.getElementById('m:eng|service');
      expect(box.isParent()).toBe(true);
      expect(box.children().length).toBe(2);
      cy.destroy();
    }
  });
});

describe('elements', () => {
  const g = lift(data, 'functions');
  const focus = new Set(['f1']);
  const part = neighbourhood(g, focus, { depth: 1 });
  const els = toElements(g, part, focus, ['app', 'eng']);
  const byId = Object.fromEntries(els.map((e) => [e.data.id, e]));

  it('marks the focus and private functions, and colours by crate', () => {
    expect(byId.f1.classes).toBe('focus');
    expect(byId.f2.classes).toBe('private');
    expect([byId.f0.data.ci, byId.f1.data.ci]).toEqual([0, 1]);
    expect(byId.f1.data.display).toBe('Svc::run\neng::service');
  });

  it('puts the focus on the innermost ring and labels repeated calls', () => {
    expect(byId.f1.data.ring).toBeGreaterThan(byId.f0.data.ring);
    expect(byId['f1>f3'].data.count).toBe('×2');
    expect(byId['f0>f1'].data.count).toBe('');
    expect(byId['f1>f3'].data.width).toBeGreaterThan(byId['f0>f1'].data.width);
  });

  it('sizes nodes to their text, within limits', () => {
    expect(nodeWidth(['a'])).toBe(120);
    expect(nodeWidth(['x'.repeat(200)])).toBe(340);
    expect(nodeWidth(['evaluate_law_output_with_trace', null])).toBeGreaterThan(120);
  });
});

describe('layouts', () => {
  const g = lift(data, 'functions');
  const focus = new Set(['f1']);
  const part = neighbourhood(g, focus, { depth: 2 });
  const elements = toElements(g, part, focus, ['app', 'eng']);
  const run = (name, spacing) => {
    const cy = createCy({ elements });
    cy.layout({ ...layoutOptions(name, spacing), fit: false }).run();
    const pos = Object.fromEntries(cy.nodes().map((n) => [n.id(), { ...n.position() }]));
    cy.destroy();
    return pos;
  };

  it('puts callers above what they call when layered top to bottom, left of it left to right', () => {
    const down = run('down');
    expect(down.f0.y).toBeLessThan(down.f1.y);
    expect(down.f1.y).toBeLessThan(down.f2.y);
    const right = run('right');
    expect(right.f0.x).toBeLessThan(right.f1.x);
  });

  it('spreads further apart with more spacing', () => {
    const span = (pos) => Math.max(...Object.values(pos).map((p) => p.y)) - Math.min(...Object.values(pos).map((p) => p.y));
    expect(span(run('down', 'roomy'))).toBeGreaterThan(span(run('down', 'compact')));
  });

  it('runs the force-directed and ring layouts without overlapping nodes', () => {
    for (const name of ['force', 'rings']) {
      const pos = run(name);
      expect(new Set(Object.values(pos).map((p) => `${Math.round(p.x)},${Math.round(p.y)}`)).size).toBe(Object.keys(pos).length);
    }
  });
});

describe('wrapping long ranks', () => {
  const box = (id, x, y, w = 100, h = 30) => ({ id, x, y, w, h });

  it('leaves short ranks as they are', () => {
    const pos = wrapRanks([box('a', 0, 0), box('b', 200, 0), box('c', 100, 100)], { maxPer: 3 });
    expect(pos.get('a')).toEqual({ x: 0, y: 0 });
    expect(pos.get('c')).toEqual({ x: 100, y: 100 });
  });

  it('wraps a long rank into even rows, in order, and moves later ranks down', () => {
    const rank = Array.from({ length: 7 }, (_, i) => box(`n${i}`, i * 130, 0));
    const pos = wrapRanks([...rank, box('below', 400, 100)], { maxPer: 3, gap: 30 });
    const rows = new Map();
    for (let i = 0; i < 7; i++) {
      const { y } = pos.get(`n${i}`);
      rows.set(y, [...(rows.get(y) ?? []), i]);
    }
    expect([...rows.values()]).toEqual([[0, 1, 2], [3, 4, 5], [6]]);
    // Three rows of 30 high with a gap of 30: the next rank moves down by two steps.
    expect(pos.get('below').y).toBe(100 + 2 * 60);
    // Within a row, left to right without overlap.
    expect(pos.get('n1').x - pos.get('n0').x).toBe(130);
  });

  it('moves a node that is not in a box out of the box\'s area', () => {
    // The box has functions in ranks 0 and 120; `other` is in rank 60, between them.
    const inBox = [0, 1, 2].flatMap((i) => [
      { ...box(`top${i}`, i * 130, 0), parent: 'B' },
      { ...box(`bottom${i}`, i * 130, 120), parent: 'B' },
    ]);
    const pos = wrapRanks([...inBox, box('other', 130, 60)], { maxPer: 3, gap: 30 });
    const members = inBox.map((m) => pos.get(m.id));
    const right = Math.max(...members.map((p) => p.x)) + 50;
    expect(pos.get('other').x - 50).toBeGreaterThanOrEqual(right + 16);
  });

  it('wraps a long column the same way when laid out left to right', () => {
    const column = Array.from({ length: 4 }, (_, i) => box(`n${i}`, 0, i * 50));
    const pos = wrapRanks([...column, box('right', 200, 0)], { axis: 'x', maxPer: 2, gap: 20 });
    expect(pos.get('n0').x).toBe(pos.get('n1').x);
    expect(pos.get('n2').x).toBeGreaterThan(pos.get('n0').x);
    expect(pos.get('right').x).toBe(200 + 120);
  });
});

describe('choosing how far to wrap', () => {
  const box = (id, x, y, w = 100, h = 30) => ({ id, x, y, w, h });
  const span = (pos, items) => {
    const xs = items.map((it) => pos.get(it.id).x);
    const ys = items.map((it) => pos.get(it.id).y);
    return { w: Math.max(...xs) - Math.min(...xs), h: Math.max(...ys) - Math.min(...ys) };
  };
  // One rank of 24 nodes: a single line 3000 wide and 30 high.
  const rank = Array.from({ length: 24 }, (_, i) => box(`n${i}`, i * 130, 0));

  it('wraps a long rank into a block for a square canvas', () => {
    const s = span(bestWrap(rank, { width: 800, height: 800 }), rank);
    expect(s.w).toBeLessThan(1000);
    expect(s.h).toBeGreaterThan(100);
  });

  it('keeps it a line for a wide, low canvas', () => {
    const s = span(bestWrap(rank, { width: 3200, height: 100 }), rank);
    expect(s.h).toBe(0);
  });

  it('leaves a graph without long ranks alone', () => {
    const items = [box('a', 0, 0), box('b', 0, 100)];
    expect([...bestWrap(items, { width: 500, height: 500 }).values()]).toEqual([
      { x: 0, y: 0 },
      { x: 0, y: 100 },
    ]);
  });
});

describe('the legend', () => {
  it('names each crate in the graph with the colour its nodes have', () => {
    const f = lift(data, 'functions');
    const part = neighbourhood(f, new Set(['f1']), { depth: 1 });
    const legend = legendOf((id) => f.nodes.get(id), part, ['app', 'eng', 'other']);
    expect(legend).toEqual([
      { crate: 'app', color: CATEGORIES[0] },
      { crate: 'eng', color: CATEGORIES[1] },
    ]);
    const els = toElements(f, part, new Set(['f1']), ['app', 'eng', 'other']);
    expect(els.find((e) => e.data.id === 'f0').data.ci).toBe(0);
    expect(els.find((e) => e.data.id === 'f1').data.ci).toBe(1);
  });
});

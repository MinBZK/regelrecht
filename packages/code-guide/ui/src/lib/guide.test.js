import { describe, expect, it } from 'vitest';
import {
  CONTEXT_STEP,
  MAX_RESULTS,
  MODULE_PREVIEW_LINES,
  findModule,
  formatHash,
  isType,
  moduleLabel,
  moduleToSegment,
  moduleTree,
  parseHash,
  refTarget,
  search,
  segmentToModule,
  sourceRange,
  targetOf,
  visibleItems,
  widen,
} from './guide.js';

const none = { crate: null, module: null, item: null, method: null, view: 'details', scope: null, wide: false };

describe('module segments', () => {
  it('round-trips nested modules, the crate root and binary roots', () => {
    for (const path of ['', 'service', 'annotation::resolver', 'bin:evaluate', 'bin:tool::cli']) {
      expect(segmentToModule(moduleToSegment(path))).toBe(path);
    }
  });

  it('needs no percent-encoding', () => {
    for (const path of ['', 'a::b', 'bin:x']) {
      const seg = moduleToSegment(path);
      expect(encodeURIComponent(seg)).toBe(seg);
    }
  });
});

describe('addresses', () => {
  it('reads every level', () => {
    expect(parseHash('#/')).toEqual(none);
    expect(parseHash('#/engine')).toEqual({ ...none, crate: 'engine' });
    expect(parseHash('#/engine/~')).toEqual({ ...none, crate: 'engine', module: '' });
    expect(parseHash('#/engine/annotation.resolver/Hint/new@From<String>')).toMatchObject({
      module: 'annotation::resolver',
      item: 'Hint',
      method: 'new@From<String>',
    });
  });

  it('round-trips a method key with a trait and generics through the address', () => {
    const sel = { ...none, crate: 'engine', module: 'service', item: 'S', method: 'from@From<String>' };
    expect(parseHash(formatHash(sel))).toEqual(sel);
  });

  it('keeps the crate root apart from no module at all', () => {
    expect(formatHash({ crate: 'engine', module: '' })).toBe('#/engine/~');
    expect(formatHash({ crate: 'engine', module: null })).toBe('#/engine');
  });

  it('carries view, scope and wide only when they are not the default', () => {
    const sel = { crate: 'engine', module: 'service' };
    expect(formatHash({ ...sel, view: 'details' })).toBe('#/engine/service');
    expect(formatHash({ ...sel, view: 'graph', scope: 'module' })).toBe('#/engine/service?view=graph&scope=module');
    expect(formatHash({ ...sel, scope: 'module' })).toBe('#/engine/service');
    expect(formatHash({ ...sel, wide: true })).toBe('#/engine/service?source=wide');
    expect(formatHash({ crate: null, view: 'graph' })).toBe('#/?view=graph');
  });

  it('falls back to the defaults for values it does not know', () => {
    expect(parseHash('#/x?view=3d&scope=galaxy&source=huge')).toMatchObject({ view: 'details', scope: null, wide: false });
  });

  it('survives a malformed escape', () => {
    expect(parseHash('#/x/m/%E0%A4%A').item).toBe('%E0%A4%A');
  });
});

describe('labels and lookups', () => {
  const crate = {
    name: 'x',
    modules: [
      { path: '', types: [], functions: [] },
      { path: 'a', types: [{ name: 'T' }], functions: [{ key: 'f' }] },
      { path: 'a::b', types: [], functions: [] },
      { path: 'bin:tool', types: [], functions: [] },
      { path: 'orphan::child', types: [], functions: [] },
    ],
  };

  it('names the crate root and binaries for people', () => {
    expect(moduleLabel('', 'engine')).toBe('engine (crate root)');
    expect(moduleLabel('bin:evaluate')).toBe('binary evaluate');
    expect(moduleLabel('a::b')).toBe('a::b');
  });

  it('tells a type from a function', () => {
    const a = findModule(crate, 'a');
    expect(isType(a, 'T')).toBe(true);
    expect(isType(a, 'f')).toBe(false);
  });

  it('nests modules, root first and binaries last, keeping orphans', () => {
    const tree = moduleTree(crate.modules);
    expect(tree.map((n) => n.path)).toEqual(['', 'a', 'orphan::child', 'bin:tool']);
    expect(tree[1].children.map((n) => n.path)).toEqual(['a::b']);
  });
});

describe('search', () => {
  const entries = [
    { kind: 'struct', module: 'm', name: 'Value', doc: null },
    { kind: 'struct', module: 'm', name: 'ValueKind', doc: null },
    { kind: 'fn', module: 'm', name: 'is_value', key: 'is_value', doc: null },
    { kind: 'method', module: 'm', type: 'S', key: 'run', name: 'S::run', doc: 'mentions value here' },
  ];

  it('ranks exact over prefix over substring over a doc hit', () => {
    expect(search(entries, 'value').results.map((r) => r.name)).toEqual(['Value', 'ValueKind', 'is_value', 'S::run']);
  });

  it('finds a method by its own name', () => {
    expect(search(entries, 'RUN').results[0].name).toBe('S::run');
  });

  it('returns nothing for an empty query or no entries', () => {
    expect(search(entries, '  ')).toEqual({ results: [], total: 0 });
    expect(search(null, 'x')).toEqual({ results: [], total: 0 });
  });

  it('caps the list and reports the true total', () => {
    const many = Array.from({ length: 100 }, (_, i) => ({ kind: 'fn', module: 'm', name: `thing${i}` }));
    const r = search(many, 'thing');
    expect(r.results).toHaveLength(MAX_RESULTS);
    expect(r.total).toBe(100);
  });
});

describe('targets', () => {
  it('turns a search hit into an address', () => {
    expect(targetOf({ kind: 'module', module: 'a' })).toEqual({ module: 'a', item: null, method: null });
    expect(targetOf({ kind: 'struct', module: 'a', name: 'T' })).toEqual({ module: 'a', item: 'T', method: null });
    expect(targetOf({ kind: 'method', module: 'a', type: 'T', key: 'run' })).toEqual({ module: 'a', item: 'T', method: 'run' });
    expect(targetOf({ kind: 'fn', module: 'a', key: 'f', name: 'f' })).toEqual({ module: 'a', item: 'f', method: null });
  });

  it('turns a call reference into an address in its own crate', () => {
    expect(refTarget({ crate: 'y', module: 'm', type: 'T', key: 'k' })).toEqual({ crate: 'y', module: 'm', item: 'T', method: 'k' });
    expect(refTarget({ crate: 'y', module: 'm', type: null, key: 'f' })).toEqual({ crate: 'y', module: 'm', item: 'f', method: null });
  });
});

describe('sourceRange', () => {
  const place = (line) => ({ file: 'x/src/a.rs', line });
  it('shows a method or function by its extent, doc comment included', () => {
    expect(sourceRange({ method: { place: place(5), extent: [3, 9] } })).toEqual({ path: 'x/src/a.rs', from: 3, to: 9 });
    expect(sourceRange({ fn: { place: place(5), extent: [3, 9] } })).toMatchObject({ from: 3, to: 9 });
  });

  it('falls back to the definition line when there is no extent', () => {
    expect(sourceRange({ fn: { place: place(5), extent: null } })).toMatchObject({ from: 5, to: 45 });
  });

  it('shows a type by its extent and a module by the top of its file', () => {
    expect(sourceRange({ type: { place: place(10), extent: [8, 30] } })).toMatchObject({ from: 8, to: 30 });
    expect(sourceRange({ module: { file: 'x/src/a.rs' } })).toMatchObject({ from: 1, to: MODULE_PREVIEW_LINES });
  });

  it('prefers a selected method over its type, and is null without anything', () => {
    expect(sourceRange({ type: { place: place(1), extent: [1, 99] }, method: { place: place(50), extent: [48, 60] } })).toMatchObject({
      from: 48,
    });
    expect(sourceRange({})).toBeNull();
    expect(sourceRange({ module: { file: null } })).toBeNull();
  });
});

describe('widen and visibleItems', () => {
  it('grows both ends and stays inside the file', () => {
    expect(widen({ from: 100, to: 120 }, 500)).toMatchObject({ from: 100 - CONTEXT_STEP, to: 120 + CONTEXT_STEP });
    expect(widen({ from: 10, to: 490 }, 500)).toMatchObject({ from: 1, to: 500 });
  });

  it('keeps only pub items unless asked for all', () => {
    const items = [{ vis: 'pub' }, { vis: 'private' }, { vis: 'trait' }];
    expect(visibleItems(items, false)).toHaveLength(1);
    expect(visibleItems(items, true)).toHaveLength(3);
  });
});

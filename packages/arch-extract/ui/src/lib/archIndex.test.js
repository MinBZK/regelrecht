import { describe, it, expect } from 'vitest';
import { buildIndex, containerOf, levelRank, unitsAtLevel } from './archIndex.js';
import { makeModel } from '../test/fixtures.js';

describe('buildIndex', () => {
  it('indexes parents, children and roots in a stable order', () => {
    const index = buildIndex(makeModel());
    expect(index.roots).toEqual(['crate:a', 'crate:b', 'crate:c']);
    expect(index.childrenMap.get('crate:a')).toEqual(['mod:a::m1', 'mod:a::m2']);
    expect(index.parentOf('type:a::m1::T1')).toBe('mod:a::m1');
    expect(index.isAncestor('crate:a', 'type:a::m1::T1')).toBe(true);
    expect(index.isAncestor('crate:b', 'type:a::m1::T1')).toBe(false);
    expect(index.depthOf('type:a::m1::T1')).toBe(2);
  });

  it('treats a node whose parent is missing from the model as a root', () => {
    const model = makeModel();
    model.nodes.push({
      id: 'fn:ghost::orphan',
      kind: 'fn',
      level: 'code',
      lang: 'rust',
      name: 'orphan',
      path: 'x',
      parent: 'type:does-not-exist',
    });
    const index = buildIndex(model);
    expect(index.roots).toContain('fn:ghost::orphan');
    expect(index.parentOf('fn:ghost::orphan')).toBeUndefined();
  });
});

describe('unitsAtLevel', () => {
  it('takes every node at or above the level, coarse levels being subsets', () => {
    const index = buildIndex(makeModel());
    const containers = unitsAtLevel(index, 'container');
    const components = unitsAtLevel(index, 'component');
    const code = unitsAtLevel(index, 'code');

    expect(containers.units).toEqual(['crate:a', 'crate:b', 'crate:c']);
    expect(components.units.length).toBe(5); // 3 crates + 2 modules
    expect(code.units.length).toBe(10); // the whole model

    for (const id of containers.units) expect(components.unitSet.has(id)).toBe(true);
    for (const id of components.units) expect(code.unitSet.has(id)).toBe(true);
  });

  it('returns units in depth-first containment order', () => {
    const index = buildIndex(makeModel());
    const { units } = unitsAtLevel(index, 'code');
    expect(units.indexOf('mod:a::m1')).toBe(units.indexOf('crate:a') + 1);
    expect(units.indexOf('type:a::m1::T1')).toBe(units.indexOf('mod:a::m1') + 1);
  });

  it('promotes a deeper node with no in-level ancestor so nothing is unreachable', () => {
    const model = makeModel();
    model.nodes.push({
      id: 'fn:ghost::orphan',
      kind: 'fn',
      level: 'code',
      lang: 'rust',
      name: 'orphan',
      path: 'x',
      parent: 'type:does-not-exist',
    });
    const index = buildIndex(model);
    // At the container level a stray `code` node has no container to hide in,
    // so it becomes a unit itself rather than losing its relations.
    expect(unitsAtLevel(index, 'container').unitSet.has('fn:ghost::orphan')).toBe(true);
  });
});

describe('levelRank / containerOf', () => {
  it('ranks the levels coarse → fine and treats unknowns as finest', () => {
    expect(levelRank('container')).toBeLessThan(levelRank('component'));
    expect(levelRank('component')).toBeLessThan(levelRank('code'));
    expect(levelRank('nonsense')).toBe(levelRank('code'));
  });

  it('maps a node to its top-most ancestor', () => {
    const index = buildIndex(makeModel());
    expect(containerOf(index, 'type:a::m1::T1')).toBe('crate:a');
    expect(containerOf(index, 'crate:b')).toBe('crate:b');
  });
});

import { describe, it, expect } from 'vitest';
import { buildFlow, computeReveal, REVEAL_LIMIT } from './useArchGraph.js';
// The synthetic model lives in src/test/fixtures.js so the prototype layout
// tests exercise exactly the same graph.
import { ALL_KINDS, makeModel } from '../test/fixtures.js';



function parentMap(model) {
  return new Map(model.nodes.map((n) => [n.id, n.parent || null]));
}

/** Find the single aggregated edge with the given kind/from/to, or undefined. */
function findEdge(edges, kind, from, to) {
  return edges.find((e) => e.data.kind === kind && e.data.from === from && e.data.to === to);
}

describe('buildFlow edge lifting', () => {
  it('lifts every endpoint to its nearest visible ancestor (all collapsed → crate level)', () => {
    const model = makeModel();
    const { edges } = buildFlow(model, new Set(), ALL_KINDS);

    // Cross-crate uses lifts to crate:a → crate:b.
    expect(findEdge(edges, 'uses', 'crate:a', 'crate:b')).toBeTruthy();
    // Cross-crate impl lifts to crate:a → crate:c.
    expect(findEdge(edges, 'impl', 'crate:a', 'crate:c')).toBeTruthy();
    // No edge ever references a node below the crate level while collapsed.
    for (const e of edges) {
      expect(e.data.from.startsWith('crate:')).toBe(true);
      expect(e.data.to.startsWith('crate:')).toBe(true);
    }
  });

  it('aggregates same-kind relations between the same lifted pair into one weighted line', () => {
    const model = makeModel();
    const { edges } = buildFlow(model, new Set(), ALL_KINDS);

    const usesAB = findEdge(edges, 'uses', 'crate:a', 'crate:b');
    expect(usesAB.data.weight).toBe(2);
    // Both underlying pairs are retained for the reveal step.
    const pairs = usesAB.data.pairs.map((p) => `${p.from}->${p.to}`).sort();
    expect(pairs).toEqual(['type:a::m1::T1->type:b::V1', 'type:a::m1::T2->type:b::V1']);
    // A weight-2 line draws thicker than its per-kind base (1.5 for `uses`),
    // and stays within the ~1.5–9px band.
    expect(usesAB.style.strokeWidth).toBeGreaterThan(1.5);
    expect(usesAB.style.strokeWidth).toBeLessThanOrEqual(9);
  });

  it('turns a relation whose ends roll up to the same node into an internal counter, not a line', () => {
    const model = makeModel();
    const { nodes, edges } = buildFlow(model, new Set(), ALL_KINDS);

    // E3 (T1→U1) and E5 (T1→crate:a) both roll up to crate:a while collapsed.
    const crateA = nodes.find((n) => n.id === 'crate:a');
    expect(crateA.data.internalCount).toBe(2);
    // No self-line on crate:a.
    expect(edges.some((e) => e.data.from === 'crate:a' && e.data.to === 'crate:a')).toBe(false);
  });

  it('skips ancestor/descendant pairs as containment (no line)', () => {
    const model = makeModel();
    // Expand crate:a but not its modules: T1 lifts to mod:a::m1, crate:a stays
    // crate:a → crate:a is an ancestor of mod:a::m1, so E5 is containment.
    const { edges } = buildFlow(model, new Set(['crate:a']), ALL_KINDS);
    expect(findEdge(edges, 'uses', 'crate:a', 'mod:a::m1')).toBeFalsy();
    expect(findEdge(edges, 'uses', 'mod:a::m1', 'crate:a')).toBeFalsy();
  });

  it('refines a rolled-up relation when one side is expanded', () => {
    const model = makeModel();
    const collapsed = buildFlow(model, new Set(), ALL_KINDS);
    // Collapsed: E3 is internal to crate:a (a counter, no line).
    expect(findEdge(collapsed.edges, 'uses', 'mod:a::m1', 'mod:a::m2')).toBeFalsy();

    const expanded = buildFlow(model, new Set(['crate:a']), ALL_KINDS);
    // Expanding crate:a splits it: E3 now runs module→module as a real line…
    expect(findEdge(expanded.edges, 'uses', 'mod:a::m1', 'mod:a::m2')).toBeTruthy();
    // …and the cross-crate uses now leaves from the module, not the crate.
    expect(findEdge(expanded.edges, 'uses', 'mod:a::m1', 'crate:b')).toBeTruthy();
    expect(findEdge(expanded.edges, 'uses', 'crate:a', 'crate:b')).toBeFalsy();
  });

  it('excludes a disabled kind from lines, weights and the internal counter', () => {
    const model = makeModel();
    const { nodes, edges } = buildFlow(model, new Set(), new Set(['depends-on', 'impl']));

    // No `uses` line survives.
    expect(edges.some((e) => e.data.kind === 'uses')).toBe(false);
    // The two `uses` relations that were internal to crate:a no longer count.
    const crateA = nodes.find((n) => n.id === 'crate:a');
    expect(crateA.data.internalCount).toBe(0);
    // impl + depends-on lines remain.
    expect(findEdge(edges, 'impl', 'crate:a', 'crate:c')).toBeTruthy();
    expect(findEdge(edges, 'depends-on', 'crate:a', 'crate:b')).toBeTruthy();
  });

  it('reports visible/total relation stats over the enabled kinds', () => {
    const model = makeModel();
    const all = buildFlow(model, new Set(), ALL_KINDS);
    // 7 edges, all enabled, all placeable → total 7. Every one is either a line
    // or an internal counter while collapsed (no containment yet), so visible 7.
    expect(all.stats.total).toBe(7);
    expect(all.stats.visible).toBe(7);

    // Expanding crate:a makes E5 (T1→crate:a) containment: still counted in
    // total, but no longer visible as a relation.
    const expanded = buildFlow(model, new Set(['crate:a']), ALL_KINDS);
    expect(expanded.stats.total).toBe(7);
    expect(expanded.stats.visible).toBe(6);
  });
});

describe('computeReveal', () => {
  it('expands every ancestor chain of the underlying endpoints below the limit', () => {
    const model = makeModel();
    const { edges, childrenMap } = buildFlow(model, new Set(), ALL_KINDS);
    const usesAB = findEdge(edges, 'uses', 'crate:a', 'crate:b');

    const next = computeReveal({
      parentOf: parentMap(model),
      childrenMap,
      expanded: new Set(),
      data: usesAB.data,
    });

    // Ancestors of T1/T2 (mod:a::m1, crate:a) and of V1 (crate:b) are opened.
    expect(next.has('crate:a')).toBe(true);
    expect(next.has('mod:a::m1')).toBe(true);
    expect(next.has('crate:b')).toBe(true);
    // Nothing unrelated is opened.
    expect(next.has('mod:a::m2')).toBe(false);

    // After revealing, the rolled-up line splits into exact per-pair lines.
    const revealed = buildFlow(model, next, ALL_KINDS);
    expect(findEdge(revealed.edges, 'uses', 'type:a::m1::T1', 'type:b::V1')).toBeTruthy();
    expect(findEdge(revealed.edges, 'uses', 'type:a::m1::T2', 'type:b::V1')).toBeTruthy();
    expect(findEdge(revealed.edges, 'uses', 'crate:a', 'crate:b')).toBeFalsy();
  });

  it('opens only one level when the underlying pair count exceeds the limit', () => {
    const model = makeModel();
    const { childrenMap } = buildFlow(model, new Set(), ALL_KINDS);
    // Synthesize a heavy rolled-up edge with more than REVEAL_LIMIT pairs.
    const pairs = Array.from({ length: REVEAL_LIMIT + 5 }, () => ({
      from: 'type:a::m1::T1',
      to: 'type:b::V1',
    }));
    const data = { kind: 'uses', from: 'crate:a', to: 'crate:b', weight: pairs.length, pairs };

    const next = computeReveal({
      parentOf: parentMap(model),
      childrenMap,
      expanded: new Set(),
      data,
    });

    // Only the two lifted endpoints open (one level), not the deep chains.
    expect(next.has('crate:a')).toBe(true);
    expect(next.has('crate:b')).toBe(true);
    expect(next.has('mod:a::m1')).toBe(false);
  });
});

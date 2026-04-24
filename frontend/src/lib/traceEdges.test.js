import { describe, it, expect } from 'vitest';
import { flattenTraceSteps, edgeIdsForStep, graphNodeIdsForStep } from './traceEdges.js';

/**
 * Locks the contract between traceEdges and useLawGraph.js. Both files
 * encode the same edge/node ID formats; if either moves these tests
 * catch the drift.
 */

describe('flattenTraceSteps', () => {
  it('produces a linear depth-indexed list in DFS order', () => {
    const tree = {
      node_type: 'article',
      name: 'wet_A (is_rechthebbende)',
      children: [
        {
          node_type: 'requirement',
          name: 'leeftijd',
          children: [
            { node_type: 'resolve', name: 'leeftijd', resolve_type: 'PARAMETER' },
          ],
        },
        { node_type: 'action', name: 'is_rechthebbende' },
      ],
    };

    const steps = flattenTraceSteps(tree, 'wet_A');

    expect(steps.map((s) => [s.depth, s.nodeType])).toEqual([
      [0, 'article'],
      [1, 'requirement'],
      [2, 'resolve'],
      [1, 'action'],
    ]);
    expect(steps[0].lawId).toBe('wet_A');
  });

  it('switches lawId on cross_law_reference with `targetLaw#output` name', () => {
    const tree = {
      node_type: 'article',
      name: 'wet_A',
      children: [
        {
          node_type: 'cross_law_reference',
          name: 'wet_B#output_x',
          children: [
            { node_type: 'action', name: 'output_x' },
          ],
        },
      ],
    };

    const steps = flattenTraceSteps(tree, 'wet_A');
    const action = steps.find((s) => s.nodeType === 'action');
    expect(action.lawId).toBe('wet_B');
  });

  it('does not emit steps for unknown node types', () => {
    const tree = {
      node_type: 'unknown_type',
      name: 'anything',
      children: [{ node_type: 'action', name: 'x' }],
    };
    const steps = flattenTraceSteps(tree, 'wet_A');
    expect(steps.map((s) => s.nodeType)).toEqual(['action']);
  });

  it('labels nodes with a readable prefix per type', () => {
    const cases = [
      ['article', 'art 1', 'Article art 1'],
      ['action', 'x', 'Action: x'],
      ['cross_law_reference', 'wet_B#y', 'Cross-law reference: wet_B#y'],
      ['open_term_resolution', 'term', 'Open term (IoC): term'],
      ['hook_resolution', 'awb:3:46', 'Hook: awb:3:46'],
      ['override_resolution', 'out', 'Override: out'],
    ];
    for (const [node_type, name, expected] of cases) {
      const steps = flattenTraceSteps(
        { node_type, name, children: [] },
        'wet_A',
      );
      expect(steps[0].label).toBe(expected);
    }
  });
});

describe('edgeIdsForStep', () => {
  const edges = [
    // cross-law input: wet_A reads output_x from wet_B
    {
      id: 'wet_A-input-output_x->wet_B-output-output_x',
      source: 'wet_A-input-output_x',
      target: 'wet_B-output-output_x',
    },
    // implements: wet_C implements open_term `gezinslid_norm` of wet_A
    {
      id: 'impl:wet_C:5:3->wet_A:gezinslid_norm',
      source: 'wet_C-impl-gezinslid_norm',
      target: 'wet_A-delegate-gezinslid_norm',
    },
    // override: wet_D overrides output_x of wet_A
    {
      id: 'ovr:wet_D:2:1->wet_A:3:1',
      source: 'wet_D-output-output_x',
      target: 'wet_A-output-output_x',
    },
    // hook: awb fires on wet_A producer-article `3:2`
    {
      id: 'hook:algemene_wet_bestuursrecht:3:46->wet_A:3:2',
      source: 'algemene_wet_bestuursrecht-output-termijn',
      target: 'wet_A-output-x',
    },
  ];

  it('matches a cross-law-reference step to its input→output edge', () => {
    const step = {
      nodeType: 'cross_law_reference',
      lawId: 'wet_A',
      name: 'wet_B#output_x',
    };
    expect(edgeIdsForStep(step, edges)).toEqual([
      'wet_A-input-output_x->wet_B-output-output_x',
    ]);
  });

  it('matches an open_term_resolution to its `impl:` edge', () => {
    const step = {
      nodeType: 'open_term_resolution',
      lawId: 'wet_C',
      name: 'gezinslid_norm',
    };
    expect(edgeIdsForStep(step, edges)).toEqual([
      'impl:wet_C:5:3->wet_A:gezinslid_norm',
    ]);
  });

  it('matches an override_resolution to its `ovr:` edge', () => {
    const step = {
      nodeType: 'override_resolution',
      lawId: 'wet_D',
      name: 'irrelevant',
    };
    expect(edgeIdsForStep(step, edges)).toEqual(['ovr:wet_D:2:1->wet_A:3:1']);
  });

  it('matches a hook_resolution to its `hook:` edge using producer law', () => {
    const step = {
      nodeType: 'hook_resolution',
      // lawId = producer law (trace attribution)
      lawId: 'wet_A',
      // name = qualified hook ref `hookLaw:art`
      name: 'algemene_wet_bestuursrecht:3:46',
    };
    expect(edgeIdsForStep(step, edges)).toEqual([
      'hook:algemene_wet_bestuursrecht:3:46->wet_A:3:2',
    ]);
  });

  it('returns [] for non-highlight step types (articles, actions, etc.)', () => {
    for (const nodeType of ['article', 'action', 'requirement', 'resolve', 'operation', 'cached']) {
      expect(edgeIdsForStep({ nodeType, lawId: 'wet_A', name: 'x' }, edges)).toEqual([]);
    }
  });
});

describe('graphNodeIdsForStep', () => {
  const nodes = [
    { id: 'wet_A' },
    { id: 'wet_A-source-bsn' },
    { id: 'wet_A-input-output_x' },
    { id: 'wet_A-output-is_rechthebbende' },
    { id: 'wet_A-impl-gezinslid_norm' },
    { id: 'wet_A-delegate-gezinslid_norm' },
    { id: 'wet_B' },
    { id: 'wet_B-output-output_x' },
  ];

  it('always highlights the step lawId root', () => {
    const step = { nodeType: 'requirement', lawId: 'wet_A', name: 'x' };
    expect(graphNodeIdsForStep(step, nodes)).toEqual(['wet_A']);
  });

  it('resolves PARAMETER resolves to the source leaf', () => {
    const step = {
      nodeType: 'resolve',
      lawId: 'wet_A',
      name: 'bsn',
      resolveType: 'PARAMETER',
    };
    expect(graphNodeIdsForStep(step, nodes)).toEqual(['wet_A', 'wet_A-source-bsn']);
  });

  it('resolves INPUT and RESOLVED_INPUT to the input leaf', () => {
    for (const rt of ['INPUT', 'RESOLVED_INPUT']) {
      const step = { nodeType: 'resolve', lawId: 'wet_A', name: 'output_x', resolveType: rt };
      expect(graphNodeIdsForStep(step, nodes)).toContain('wet_A-input-output_x');
    }
  });

  it('resolves OUTPUT/DEFINITION to the current law output, else falls back across laws', () => {
    // Current law owns the leaf
    let step = { nodeType: 'resolve', lawId: 'wet_A', name: 'is_rechthebbende', resolveType: 'OUTPUT' };
    expect(graphNodeIdsForStep(step, nodes)).toContain('wet_A-output-is_rechthebbende');

    // Current law does NOT own the output, fallback to any law with it
    step = { nodeType: 'resolve', lawId: 'wet_A', name: 'output_x', resolveType: 'OUTPUT' };
    expect(graphNodeIdsForStep(step, nodes)).toContain('wet_B-output-output_x');
  });

  it('highlights the output leaf (+ fallback) for an action', () => {
    const step = { nodeType: 'action', lawId: 'wet_A', name: 'is_rechthebbende' };
    expect(graphNodeIdsForStep(step, nodes)).toContain('wet_A-output-is_rechthebbende');
  });

  it('highlights the input + target law for a cross_law_reference', () => {
    const step = {
      nodeType: 'cross_law_reference',
      lawId: 'wet_A',
      name: 'wet_B#output_x',
    };
    const ids = graphNodeIdsForStep(step, nodes);
    expect(ids).toEqual(
      expect.arrayContaining(['wet_A', 'wet_A-input-output_x', 'wet_B', 'wet_B-output-output_x']),
    );
  });

  it('highlights impl + delegate leaves for open_term_resolution', () => {
    const step = {
      nodeType: 'open_term_resolution',
      lawId: 'wet_A',
      name: 'gezinslid_norm',
    };
    expect(graphNodeIdsForStep(step, nodes)).toEqual(
      expect.arrayContaining(['wet_A-impl-gezinslid_norm', 'wet_A-delegate-gezinslid_norm']),
    );
  });

  it('parses an article name like `${law} (${output})` and highlights the output', () => {
    const step = {
      nodeType: 'article',
      lawId: 'wet_A',
      name: 'wet_A (is_rechthebbende)',
    };
    expect(graphNodeIdsForStep(step, nodes)).toContain('wet_A-output-is_rechthebbende');
  });
});

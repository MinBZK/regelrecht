/**
 * traceEdges — flatten a PathNode trace tree into a linear step list and
 * match each step to graph edge / node IDs for highlighting.
 *
 * Ported 1:1 from demo/graph/src/lib/traceEdges.ts on branch
 * feature/demo-leenstelsel-tegemoetkoming. Framework-agnostic pure JS.
 *
 * Edge ID formats MUST match what composables/useLawGraph.js emits:
 *   - cross-law input:  `${lawA}-input-${name}->${lawB}-output-${sourceOutput||name}`
 *   - implements:       `impl:${lawA}:${art}->${implLaw}:${openTerm}`
 *   - override:         `ovr:${lawA}:${art}->${ovrLaw}:${ovrArticle}`
 *   - hook:             `hook:${hookLaw}:${art}->${producerLaw}:${producerArt}`
 *
 * Leaf node ID formats (emitted by buildGraph in useLawGraph.js):
 *   - root:     `${lawId}`
 *   - source:   `${lawId}-source-${name}`    (parameters)
 *   - input:    `${lawId}-input-${name}`
 *   - output:   `${lawId}-output-${name}`
 *   - delegate: `${lawId}-delegate-${name}`
 *   - impl:     `${lawId}-impl-${name}`
 *
 * If you change either scheme, update BOTH this file and useLawGraph.js —
 * the integration test in traceEdges.test.js is the tripwire.
 */

/**
 * Whether a step is rendered when the user has the "Met highlights"
 * filter on. Centralised here so LawGraphView's nav and TraceStepList's
 * row filter can't drift apart.
 */
export function stepHasHighlights(step) {
  return step.edgeIds.length > 0 || step.nodeIds.length > 0;
}

const HIGHLIGHT_TYPES = new Set([
  'cross_law_reference',
  'open_term_resolution',
  'hook_resolution',
  'override_resolution',
]);

const CONTEXT_TYPES = new Set([
  'article',
  'action',
  'requirement',
  'resolve',
  'operation',
  'cached',
]);

function nodeLabel(node) {
  switch (node.node_type) {
    case 'cross_law_reference':
      return `Cross-law reference: ${node.name}`;
    case 'open_term_resolution':
      return `Open term (IoC): ${node.name}`;
    case 'hook_resolution':
      return `Hook: ${node.name}`;
    case 'override_resolution':
      return `Override: ${node.name}`;
    case 'article':
      return `Article ${node.name}`;
    case 'action':
      return `Action: ${node.name}`;
    case 'requirement':
      return `Requirement: ${node.name}`;
    case 'resolve':
      return `Resolve: ${node.name}`;
    case 'operation':
      return `Operation: ${node.name}`;
    case 'cached':
      return `Cached: ${node.name}`;
    default:
      return `${node.node_type}: ${node.name}`;
  }
}

/**
 * Walk the trace tree depth-first, producing a flat list of interesting +
 * structural steps. Tracks the current law_id as we descend into cross-law
 * references so that action/resolve steps inside the referenced subtree get
 * the correct law attribution.
 */
export function flattenTraceSteps(root, rootLawId) {
  const steps = [];

  function walk(node, currentLawId, depth) {
    const isHighlight = HIGHLIGHT_TYPES.has(node.node_type);
    const isContext = CONTEXT_TYPES.has(node.node_type);

    if (isHighlight || isContext) {
      steps.push({
        label: nodeLabel(node),
        nodeType: node.node_type,
        lawId: currentLawId,
        name: node.name,
        resolveType: node.resolve_type,
        result: node.result,
        message: node.message,
        durationUs: node.duration_us,
        depth,
        edgeIds: [],
        nodeIds: [],
      });
    }

    // For cross-law refs the subtree executes in the referenced law, so we
    // try to pin the descent lawId from the node name (`targetLaw#output`).
    let descendLawId = currentLawId;
    if (node.node_type === 'cross_law_reference') {
      const hashIdx = node.name.indexOf('#');
      if (hashIdx > 0) {
        descendLawId = node.name.substring(0, hashIdx);
      }
    }

    const children = node.children ?? [];
    for (const c of children) {
      walk(c, descendLawId, depth + 1);
    }
  }

  walk(root, rootLawId, 0);
  return steps;
}

/**
 * Pre-index edges so per-step matchers don't have to scan the full edge
 * list. `useTraceStepping.steps` builds this once per (graph) change and
 * passes it to every `edgeIdsForStep` call — cuts step enrichment from
 * O(steps × edges) to O(steps).
 *
 * Buckets mirror the four switch arms in `edgeIdsForStep`:
 *   - bySource           — `${law}-input-${name}` → edges with that source
 *   - byImplOpenTerm     — open-term name → `impl:` edges ending `:${name}`
 *   - byOvrSourceLaw     — law id → `ovr:${law}:` edges
 *   - byHookPrefix       — `hook:${name}->` → edges with that prefix
 */
export function buildEdgeIndex(edges) {
  const bySource = new Map();
  const byImplOpenTerm = new Map();
  const byOvrSourceLaw = new Map();
  const byHookPrefix = new Map();

  const push = (m, k, v) => {
    const cur = m.get(k);
    if (cur) cur.push(v); else m.set(k, [v]);
  };

  for (const e of edges) {
    if (typeof e.source === 'string') push(bySource, e.source, e);
    if (typeof e.id !== 'string') continue;
    if (e.id.startsWith('impl:')) {
      const colon = e.id.lastIndexOf(':');
      if (colon !== -1) push(byImplOpenTerm, e.id.substring(colon + 1), e);
    } else if (e.id.startsWith('ovr:')) {
      // `ovr:${lawA}:${art}->${lawB}:${art}` — bucket by `lawA`
      const head = e.id.indexOf(':');
      const tail = e.id.indexOf(':', head + 1);
      if (head !== -1 && tail !== -1) {
        push(byOvrSourceLaw, e.id.substring(head + 1, tail), e);
      }
    } else if (e.id.startsWith('hook:')) {
      const arrow = e.id.indexOf('->');
      if (arrow !== -1) push(byHookPrefix, e.id.substring(0, arrow + 2), e);
    }
  }

  return { bySource, byImplOpenTerm, byOvrSourceLaw, byHookPrefix };
}

/**
 * Pre-build a Set of node IDs so `graphNodeIdsForStep` doesn't have to
 * rebuild the same Set for every step. Hot enough on heavy graphs that
 * the per-step `new Set(nodes.map(...))` dominated profiling.
 */
export function buildNodeIdSet(nodes) {
  const out = new Set();
  for (const n of nodes) out.add(n.id);
  return out;
}

/**
 * Parse a trace node name into (targetLaw, localName) when it encodes a
 * cross-law target. Supports `law#output` and `law:article:lid` shapes.
 */
function splitQualifiedName(name) {
  const hashIdx = name.indexOf('#');
  if (hashIdx >= 0) {
    return [name.substring(0, hashIdx), name.substring(hashIdx + 1)];
  }
  const colonIdx = name.indexOf(':');
  if (colonIdx >= 0) {
    return [name.substring(0, colonIdx), name.substring(colonIdx + 1)];
  }
  return [null, name];
}

/**
 * Return edge IDs that should light up for the given step.
 *
 * For cross-law references the trace node name is typically of the form
 * `target_law#output_name`; we extract the output name and match on the
 * source leaf `${sourceLaw}-input-${output}`. Multiple candidate edges all
 * get highlighted.
 *
 * Pass a pre-built `index` from `buildEdgeIndex(edges)` to skip the full
 * edge scan — `useTraceStepping` does this for every step at once.
 * Without it, a transient index is built locally so the function still
 * works on its own (test-friendly).
 */
export function edgeIdsForStep(step, edges, index) {
  const idx = index || buildEdgeIndex(edges);
  switch (step.nodeType) {
    case 'cross_law_reference': {
      const [targetLaw, outputName] = splitQualifiedName(step.name);
      // TODO(PR3): when the consumer renames the input locally
      // (consumer-side `input.name` differs from `source_output`), this
      // match fails because we use the producer's output name. A richer
      // match would index `useLawGraph` edges by `data.refersToService`
      // plus the consumer's local input name. Carrying over the demo
      // limitation for now.
      const src = `${step.lawId}-input-${outputName}`;
      const bucket = idx.bySource.get(src);
      if (!bucket) return [];
      const out = [];
      for (const e of bucket) {
        if (targetLaw && typeof e.target === 'string'
            && !e.target.startsWith(`${targetLaw}-`)) continue;
        out.push(e.id);
      }
      return out;
    }
    case 'open_term_resolution': {
      // `lawId` is ambiguous on open_term steps — `flattenTraceSteps`
      // doesn't switch `descendLawId` on this node type, so the engine
      // emits it under whichever law was active (typically the higher
      // declaring law, not the implementing one). The `impl:` edge id
      // ends `:${openTerm}` regardless of either side, so we look up
      // by that suffix only. False positives are bounded: open_term
      // names are unique per declaring higher-law in practice.
      // Edge ID format: `impl:${implLawId}:${art}->${higherLaw}:${openTerm}`
      const bucket = idx.byImplOpenTerm.get(step.name);
      return bucket ? bucket.map((e) => e.id) : [];
    }
    case 'hook_resolution': {
      // The trace node's lawId is the producer law (where the hook fires).
      // The name is a qualified hook ref like `hookLaw:art`.
      // Edge ID format: `hook:${hookLaw}:${art}->${producerLaw}:${producerArt}`
      const hookPrefix = `hook:${step.name}->`;
      const bucket = idx.byHookPrefix.get(hookPrefix);
      if (!bucket) return [];
      const lawTail = `->${step.lawId}:`;
      const out = [];
      for (const e of bucket) {
        if (e.id.includes(lawTail)) out.push(e.id);
      }
      return out;
    }
    case 'override_resolution': {
      // Edge ID format: `ovr:${lawA}:${art}->${lawB}:${article}`
      // TODO(PR3): step.name carries the overridden output but goes
      // unused — when one law overrides multiple outputs all `ovr:`
      // edges from that law light up simultaneously. A precise match
      // would also constrain by source/target output name once the
      // engine starts emitting the output in the trace node.
      const bucket = idx.byOvrSourceLaw.get(step.lawId);
      return bucket ? bucket.map((e) => e.id) : [];
    }
    default:
      return [];
  }
}

/**
 * Return graph node IDs that should light up for the given step. Matches
 * trace nodes to leaf nodes (parameter/input/output/delegate/impl) plus
 * the root law node so users can see which law is executing.
 *
 * Pass a pre-built `nodeIdSet` from `buildNodeIdSet(nodes)` to skip
 * rebuilding the same Set per step. The fallback `findLeafByName` scan
 * still iterates `nodes`; that's only used when the primary id miss-hits.
 */
export function graphNodeIdsForStep(step, nodes, nodeIdSet) {
  const nodeSet = nodeIdSet || buildNodeIdSet(nodes);
  const out = [];
  const add = (id) => {
    if (id && nodeSet.has(id) && !out.includes(id)) out.push(id);
  };

  /**
   * Find any leaf node whose id ends with `-${suffix}-${name}`, regardless
   * of which law it belongs to. Fallback for hook-actions where the output
   * lives on a different law than the one firing.
   */
  const findLeafByName = (suffix, name) => {
    const tail = `-${suffix}-${name}`;
    const matches = [];
    for (const n of nodes) {
      if (n.id.endsWith(tail)) matches.push(n.id);
    }
    return matches;
  };

  // Always highlight the current law root so the user can track which law
  // is executing at every step.
  add(step.lawId);

  switch (step.nodeType) {
    case 'article': {
      // Article name is typically `${lawId} (${output})`; also highlight
      // the output leaf if the name encodes one.
      const m = step.name.match(/\(([^)]+)\)/);
      if (m) {
        const outName = m[1];
        add(`${step.lawId}-output-${outName}`);
        if (!nodeSet.has(`${step.lawId}-output-${outName}`)) {
          for (const id of findLeafByName('output', outName)) add(id);
        }
      }
      break;
    }
    case 'action': {
      // Action writes to an output. Prefer the current law; fall back to
      // any law that owns a leaf with the same name (hook-actions write
      // to outputs defined on the hook's originating law).
      const primary = `${step.lawId}-output-${step.name}`;
      if (nodeSet.has(primary)) {
        add(primary);
      } else {
        for (const id of findLeafByName('output', step.name)) add(id);
      }
      break;
    }
    case 'requirement':
      // Just the law root — nothing more specific.
      break;
    case 'resolve': {
      const rt = step.resolveType;
      if (rt === 'PARAMETER') {
        add(`${step.lawId}-source-${step.name}`);
      } else if (rt === 'INPUT' || rt === 'RESOLVED_INPUT') {
        add(`${step.lawId}-input-${step.name}`);
      } else if (rt === 'OUTPUT' || rt === 'DEFINITION') {
        const primary = `${step.lawId}-output-${step.name}`;
        if (nodeSet.has(primary)) {
          add(primary);
        } else {
          for (const id of findLeafByName('output', step.name)) add(id);
        }
      } else {
        // Fall back: any leaf whose suffix matches in the current law
        for (const suffix of ['source', 'input', 'output']) {
          const id = `${step.lawId}-${suffix}-${step.name}`;
          if (nodeSet.has(id)) {
            add(id);
            break;
          }
        }
        if (out.length <= 1) {
          for (const id of findLeafByName('output', step.name)) add(id);
          for (const id of findLeafByName('input', step.name)) add(id);
        }
      }
      break;
    }
    case 'cross_law_reference': {
      const [targetLaw, outputName] = splitQualifiedName(step.name);
      add(`${step.lawId}-input-${outputName}`);
      if (targetLaw) {
        add(targetLaw);
        add(`${targetLaw}-output-${outputName}`);
      }
      break;
    }
    case 'open_term_resolution':
      add(`${step.lawId}-impl-${step.name}`);
      add(`${step.lawId}-delegate-${step.name}`);
      break;
    case 'hook_resolution': {
      const [hookLaw] = splitQualifiedName(step.name);
      if (hookLaw) add(hookLaw);
      break;
    }
    case 'override_resolution':
      break;
  }

  return out;
}

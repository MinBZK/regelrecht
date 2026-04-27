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
 */
export function edgeIdsForStep(step, edges) {
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
      return edges
        .filter((e) => {
          if (e.source !== src) return false;
          if (targetLaw && typeof e.target === 'string') {
            return e.target.startsWith(`${targetLaw}-`);
          }
          return true;
        })
        .map((e) => e.id);
    }
    case 'open_term_resolution': {
      // name is the open_term id; lawId is the law that implements it.
      // Edge ID format: `impl:${implLawId}:${art}->${higherLaw}:${openTerm}`
      return edges
        .filter(
          (e) =>
            e.id.startsWith(`impl:${step.lawId}:`) && e.id.endsWith(`:${step.name}`),
        )
        .map((e) => e.id);
    }
    case 'hook_resolution': {
      // The trace node's lawId is the producer law (where the hook fires).
      // The name is a qualified hook ref like `hookLaw:art`.
      // Edge ID format: `hook:${hookLaw}:${art}->${producerLaw}:${producerArt}`
      const hookPrefix = `hook:${step.name}->`;
      return edges
        .filter((e) => {
          if (!e.id.startsWith(hookPrefix)) return false;
          return e.id.includes(`->${step.lawId}:`);
        })
        .map((e) => e.id);
    }
    case 'override_resolution': {
      // Edge ID format: `ovr:${lawA}:${art}->${lawB}:${article}`
      return edges
        .filter((e) => e.id.startsWith(`ovr:${step.lawId}:`))
        .map((e) => e.id);
    }
    default:
      return [];
  }
}

/**
 * Return graph node IDs that should light up for the given step. Matches
 * trace nodes to leaf nodes (parameter/input/output/delegate/impl) plus
 * the root law node so users can see which law is executing.
 */
export function graphNodeIdsForStep(step, nodes) {
  const nodeSet = new Set(nodes.map((n) => n.id));
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
    return nodes.map((n) => n.id).filter((id) => id.endsWith(tail));
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

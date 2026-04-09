/**
 * traceEdges — flattens a PathNode trace tree into a linear step list and
 * matches each step to graph edge IDs.
 *
 * Edge ID formats are determined in demo/graph/src/routes/+page.svelte:
 *   cross-law input: `${lawA}-input-${name}->${lawB}-output-${sourceOutput||name}`
 *   implements:      `impl:${lawA}:${art}->${implLaw}:${openTerm}`
 *   override:        `ovr:${lawA}:${art}->${ovrLaw}:${ovrArticle}`
 *   hook:            `hook:${lawA}:${art}->${producerLaw}:${producerArt}`
 */

import type { PathNode } from './wasmEngine';

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  [k: string]: unknown;
}

export interface GraphNode {
  id: string;
  [k: string]: unknown;
}

export interface Step {
  label: string;
  nodeType: string;
  lawId: string;
  name: string;
  resolveType?: string;
  result?: unknown;
  message?: string;
  durationUs?: number;
  depth: number;
  edgeIds: string[];
  nodeIds: string[]; // graph nodes (leaf / root) to highlight
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

function nodeLabel(node: PathNode): string {
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
export function flattenTraceSteps(root: PathNode, rootLawId: string): Step[] {
  const steps: Step[] = [];

  function walk(node: PathNode, currentLawId: string, depth: number): void {
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
 * Parse the name of a trace node into (targetLaw, outputName) if it's in the
 * form "law_id#output" (cross-law references) or "law_id:article:lid" (hook
 * targets). Returns [null, name] if no separator is present.
 */
function splitQualifiedName(name: string): [string | null, string] {
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
 * get highlighted — acceptable for a demo.
 */
export function edgeIdsForStep(step: Step, edges: GraphEdge[]): string[] {
  switch (step.nodeType) {
    case 'cross_law_reference': {
      const [targetLaw, outputName] = splitQualifiedName(step.name);
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
      // The name is a qualified hook ref like `hookLaw:art` (e.g. `algemene_wet_bestuursrecht:3:46`).
      // Edge ID format: `hook:${hookLaw}:${art}->${producerLaw}:${producerArt}`
      const hookPrefix = `hook:${step.name}->`;
      return edges
        .filter((e) => {
          if (!e.id.startsWith(hookPrefix)) return false;
          // Narrow to edges whose producer side matches the current law.
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
 * Return graph node IDs that should light up for the given step.
 * Matches trace nodes to leaf nodes in the graph (parameter/input/output/
 * delegate leaves) plus the root law node for article-level steps.
 *
 * Leaf node ID formats (from +page.svelte:385-526):
 *   root:      `${lawId}`
 *   source:    `${lawId}-source-${name}`  (parameters)
 *   input:     `${lawId}-input-${name}`
 *   output:    `${lawId}-output-${name}`
 *   delegate:  `${lawId}-delegate-${name}`
 *   impl:      `${lawId}-impl-${name}`
 */
export function graphNodeIdsForStep(step: Step, nodes: GraphNode[]): string[] {
  const nodeSet = new Set(nodes.map((n) => n.id));
  const out: string[] = [];
  const add = (id: string) => {
    if (id && nodeSet.has(id) && !out.includes(id)) out.push(id);
  };

  /**
   * Find any leaf node whose id ends with `-${suffix}-${name}`, regardless
   * of which law it belongs to. Used as a fallback for hook-actions where
   * an output from another law (e.g. AWB) is being written by a producer
   * law that doesn't own a leaf for it.
   */
  const findLeafByName = (suffix: string, name: string): string[] => {
    const tail = `-${suffix}-${name}`;
    return nodes.map((n) => n.id).filter((id) => id.endsWith(tail));
  };

  // Always highlight the current law (root node) so the user can see which
  // law is being executed at every step.
  add(step.lawId);

  switch (step.nodeType) {
    case 'article': {
      // Article name is typically `${lawId} (${output})`; the `lawId` is
      // already added. Try to also highlight the output leaf if the name
      // contains one.
      const m = step.name.match(/\(([^)]+)\)/);
      if (m) {
        const outName = m[1];
        add(`${step.lawId}-output-${outName}`);
        // Fallback: search across all laws (hook/IoC outputs live elsewhere)
        if (!nodeSet.has(`${step.lawId}-output-${outName}`)) {
          for (const id of findLeafByName('output', outName)) add(id);
        }
      }
      break;
    }
    case 'action': {
      // Action writes to an output field. Prefer the current law; if that
      // leaf doesn't exist (hook-actions write to outputs defined in the
      // hook's originating law), fall back to any law's leaf with this
      // name and highlight all of them.
      const primary = `${step.lawId}-output-${step.name}`;
      if (nodeSet.has(primary)) {
        add(primary);
      } else {
        for (const id of findLeafByName('output', step.name)) add(id);
      }
      break;
    }
    case 'requirement':
      // Just the law — nothing more specific.
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
        // Fall back: try any leaf whose suffix matches the name in current law
        for (const suffix of ['source', 'input', 'output']) {
          const id = `${step.lawId}-${suffix}-${step.name}`;
          if (nodeSet.has(id)) {
            add(id);
            break;
          }
        }
        // Ultimate fallback: search output/input leaves across all laws
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
        add(targetLaw); // also highlight the target law's root node
        add(`${targetLaw}-output-${outputName}`);
      }
      break;
    }
    case 'open_term_resolution':
      add(`${step.lawId}-impl-${step.name}`);
      add(`${step.lawId}-delegate-${step.name}`);
      break;
    case 'hook_resolution': {
      // Also highlight the hook-defining law's root if we can parse it.
      const [hookLaw] = splitQualifiedName(step.name);
      if (hookLaw) add(hookLaw);
      break;
    }
    case 'override_resolution':
      break;
  }

  return out;
}

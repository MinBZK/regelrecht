/**
 * Convert the object form of the graph - what a JSON endpoint returns, and
 * what a small hand-written fixture looks like - into the packed typed-array
 * form the renderer consumes.
 *
 * The renderer only ever sees the packed form, so this function is the single
 * place where the wire contract is interpreted. Edges reference nodes by `id`;
 * an edge pointing at an unknown id is dropped and counted, because a dangling
 * reference is a data-layer defect and must not take the view down with it.
 */

import { kindId, edgeTypeId, statusId } from './graphSchema.js';

/**
 * @param {{nodes: object[], edges: object[]}} graph
 * @returns {import('./graphSchema.js').PackedGraph & {ids: string[], dropped: number}}
 */
export function packGraph(graph) {
  const nodes = graph?.nodes ?? [];
  const edges = graph?.edges ?? [];
  const nodeCount = nodes.length;

  const positions = new Float32Array(nodeCount * 3);
  const kind = new Uint8Array(nodeCount);
  const status = new Uint8Array(nodeCount);
  const cluster = new Uint8Array(nodeCount);
  const weight = new Float32Array(nodeCount);
  const labels = new Array(nodeCount);
  const ids = new Array(nodeCount);
  const index = new Map();
  let duplicates = 0;

  for (let i = 0; i < nodeCount; i++) {
    const n = nodes[i];
    positions[i * 3] = Number(n.x) || 0;
    positions[i * 3 + 1] = Number(n.y) || 0;
    positions[i * 3 + 2] = Number(n.z) || 0;
    kind[i] = kindId(n.kind);
    status[i] = statusId(n.status);
    cluster[i] = (Number(n.cluster) || 0) & 0xff;
    weight[i] = Number.isFinite(n.weight) ? n.weight : 1;
    labels[i] = n.label ?? n.id ?? '';
    ids[i] = n.id;
    // A duplicate id is a data-layer defect: the later node wins the index and
    // every edge that meant the earlier one silently moves. Count it so the
    // caller can see it instead of chasing a wrong edge later.
    if (index.has(n.id)) duplicates++;
    index.set(n.id, i);
  }

  // Two passes so the typed arrays are allocated exactly once: count the
  // resolvable edges first, then fill.
  let kept = 0;
  for (let e = 0; e < edges.length; e++) {
    if (index.has(edges[e].source) && index.has(edges[e].target)) kept++;
  }
  const edgeSource = new Uint32Array(kept);
  const edgeTarget = new Uint32Array(kept);
  const edgeType = new Uint8Array(kept);
  let w = 0;
  for (let e = 0; e < edges.length; e++) {
    const s = index.get(edges[e].source);
    const t = index.get(edges[e].target);
    if (s === undefined || t === undefined) continue;
    edgeSource[w] = s;
    edgeTarget[w] = t;
    edgeType[w] = edgeTypeId(edges[e].type);
    w++;
  }

  return {
    nodeCount,
    edgeCount: kept,
    positions,
    kind,
    status,
    cluster,
    weight,
    labels,
    edgeSource,
    edgeTarget,
    edgeType,
    ids,
    dropped: edges.length - kept,
    duplicates,
  };
}

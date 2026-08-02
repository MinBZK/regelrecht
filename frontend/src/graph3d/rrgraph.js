/**
 * Reader for the `.rrgraph` payload the graph builder writes.
 *
 * The format is a JSON header plus little-endian typed-array sections (see
 * `packages/graph/src/payload.rs`). Decoding is therefore a handful of typed
 * array views over the same ArrayBuffer - no per-node object, no JSON parse of
 * the bulk - which is the only way 205.000 nodes arrive in a browser without a
 * multi-second stall.
 *
 * Two things this reader does beyond viewing:
 *
 * - **Article coordinates are relative to their law.** The payload stores them
 *   that way so a law's inside looks identical wherever the law sits; the
 *   renderer needs world coordinates, so the parent offset is added once here.
 * - **Sections are looked up by name and may be absent.** The format grows
 *   (enrichment status is being added), and a renderer that indexes sections
 *   positionally would break on the first new build.
 */

import { KIND_IDS, STATUS_IDS } from './graphSchema.js';

export const MAGIC = 'RRGRAPH';
/** The eighth magic byte is the format generation. */
export const MAGIC_GENERATION = 1;

const VIEWS = {
  f32: Float32Array,
  u32: Uint32Array,
  u16: Uint16Array,
  u8: Uint8Array,
  i32: Int32Array,
};

/** The geometry family for a node, from its kind and its regulatory layer. */
export function familyFor(kindName, layerName) {
  if (kindName === 'article') return KIND_IDS.artikel;
  // A node for a document nobody holds yet is a placeholder, not a regulation:
  // it gets its own silhouette so it never reads as harvested law.
  if (kindName === 'expected' || kindName === 'external') return KIND_IDS.beleidsregel;
  switch (layerName) {
    case 'AMVB':
      return KIND_IDS.amvb;
    case 'MINISTERIELE_REGELING':
      return KIND_IDS.ministeriele_regeling;
    case 'BELEIDSREGEL':
      return KIND_IDS.beleidsregel;
    case 'GEMEENTELIJKE_VERORDENING':
    case 'PROVINCIALE_VERORDENING':
    case 'WATERSCHAPS_VERORDENING':
      return KIND_IDS.uitvoeringsdocument;
    default:
      return KIND_IDS.law;
  }
}

/** Parse only the header. Cheap, and enough to decide whether to load at all. */
export function readHeader(buffer) {
  const bytes = new Uint8Array(buffer);
  // The magic is "RRGRAPH" plus a one-byte format generation, so only the
  // first seven bytes are text.
  const magic = String.fromCharCode(...bytes.subarray(0, 7));
  if (magic !== MAGIC) throw new Error('geen rrgraph-payload (magic klopt niet)');
  if (bytes[7] !== MAGIC_GENERATION) {
    throw new Error(`rrgraph-generatie ${bytes[7]} wordt niet gelezen`);
  }
  const view = new DataView(buffer);
  const headerLen = view.getUint32(8, true);
  const json = new TextDecoder().decode(bytes.subarray(12, 12 + headerLen));
  return JSON.parse(json);
}

/**
 * Decode a `.rrgraph` buffer into the packed form the renderer consumes.
 *
 * @param {ArrayBuffer} buffer
 * @param {object} [opts]
 * @param {boolean} [opts.lawLevelOnly] keep only the law-level block
 * @param {boolean} [opts.labels] materialise label strings (default true)
 * @returns {import('./graphSchema.js').PackedGraph & {header: object, ids: string[]}}
 */
export function decodeRrgraph(buffer, { lawLevelOnly = false, labels = true } = {}) {
  const header = readHeader(buffer);
  if (header.format !== 'rrgraph') throw new Error(`onbekend formaat: ${header.format}`);
  if (header.version !== 1) throw new Error(`onbekende payloadversie: ${header.version}`);

  const base = header.data_offset;
  const section = (name) => {
    const s = header.sections.find((x) => x.name === name);
    if (!s) return null;
    const Ctor = VIEWS[s.type];
    if (!Ctor) throw new Error(`onbekend sectietype: ${s.type}`);
    return new Ctor(buffer, base + s.offset, s.len);
  };

  const nodePos = section('node_pos');
  const nodeId = section('node_id');
  const nodeLabel = section('node_label');
  const nodeKind = section('node_kind');
  const nodeLayer = section('node_layer');
  const nodeWeight = section('node_weight');
  const nodeRank = section('node_rank');
  const nodeCluster = section('node_cluster');
  const nodeParent = section('node_parent');
  const nodeFlags = section('node_flags');
  const nodeStatus = section('node_status'); // not written yet; see below
  const edgeSrc = section('edge_src');
  const edgeDst = section('edge_dst');
  const edgeTypeSec = section('edge_type');
  const edgeCountSec = section('edge_count');

  const nodeCount = lawLevelOnly ? header.law_node_count : header.node_count;
  const rawEdgeCount = lawLevelOnly ? header.law_edge_count : header.edge_count;

  // World positions: article coordinates are stored relative to their law.
  const positions = new Float32Array(nodeCount * 3);
  positions.set(nodePos.subarray(0, nodeCount * 3));
  if (nodeParent) {
    for (let i = 0; i < nodeCount; i++) {
      const parent = nodeParent[i];
      if (parent === 0xffffffff || parent >= nodeCount) continue;
      positions[i * 3] += positions[parent * 3];
      positions[i * 3 + 1] += positions[parent * 3 + 1];
      positions[i * 3 + 2] += positions[parent * 3 + 2];
    }
  }

  const kinds = header.kinds ?? [];
  const layers = header.layers ?? [];
  const kind = new Uint8Array(nodeCount);
  for (let i = 0; i < nodeCount; i++) {
    kind[i] = familyFor(kinds[nodeKind?.[i] ?? 0], layers[nodeLayer?.[i] ?? 0]);
  }

  const cluster = new Uint16Array(nodeCount);
  const framework = new Uint8Array(nodeCount);
  for (let i = 0; i < nodeCount; i++) {
    const c = nodeCluster ? nodeCluster[i] : 0;
    const isFramework = c === header.framework_cluster || ((nodeFlags?.[i] ?? 0) & 1) === 1;
    framework[i] = isFramework ? 1 : 0;
    cluster[i] = isFramework ? 0 : c;
  }

  // Enrichment status is on its way into the payload. Until it lands every
  // node is `harvested`, which is exactly the picture the corpus deserves: a
  // grey field with colour only where work has been done.
  const status = new Uint8Array(nodeCount);
  if (nodeStatus) status.set(nodeStatus.subarray(0, nodeCount));
  else status.fill(STATUS_IDS.harvested);

  const weight = new Float32Array(nodeCount);
  const src = nodeWeight ?? nodeRank;
  if (src) weight.set(src.subarray(0, nodeCount));
  else weight.fill(1);

  const strings = header.strings ?? [];
  const ids = new Array(nodeCount);
  const labelArr = labels ? new Array(nodeCount) : null;
  for (let i = 0; i < nodeCount; i++) {
    ids[i] = strings[nodeId?.[i] ?? 0] ?? String(i);
    if (labelArr) labelArr[i] = strings[nodeLabel?.[i] ?? 0] ?? ids[i];
  }

  // Self-references exist in the corpus (a law binding to its own output).
  // They are a counter on the node, never a loop, so they are dropped here and
  // counted instead. Edges into the article block are dropped when only the
  // law level is loaded.
  const edgeSource = new Uint32Array(rawEdgeCount);
  const edgeTarget = new Uint32Array(rawEdgeCount);
  const edgeType = new Uint8Array(rawEdgeCount);
  const edgeWeight = new Uint32Array(rawEdgeCount);
  const selfRefs = new Uint32Array(nodeCount);
  let w = 0;
  for (let e = 0; e < rawEdgeCount; e++) {
    const s = edgeSrc[e];
    const t = edgeDst[e];
    if (s >= nodeCount || t >= nodeCount) continue;
    if (s === t) {
      selfRefs[s]++;
      continue;
    }
    edgeSource[w] = s;
    edgeTarget[w] = t;
    edgeType[w] = edgeTypeSec ? edgeTypeSec[e] : 0;
    edgeWeight[w] = edgeCountSec ? edgeCountSec[e] : 1;
    w++;
  }

  return {
    header,
    nodeCount,
    edgeCount: w,
    positions,
    kind,
    status,
    cluster,
    framework,
    weight,
    labels: labelArr,
    ids,
    selfRefs,
    edgeSource: edgeSource.subarray(0, w),
    edgeTarget: edgeTarget.subarray(0, w),
    edgeType: edgeType.subarray(0, w),
    edgeWeight: edgeWeight.subarray(0, w),
    edgeTypeNames: header.edge_types ?? [],
    lawNodeCount: header.law_node_count,
  };
}

/**
 * Fetch and decode a payload.
 * @param {string} url
 * @param {object} [opts] passed to decodeRrgraph
 */
export async function loadRrgraph(url, opts) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`payload ${url} niet geladen (HTTP ${res.status})`);
  return decodeRrgraph(await res.arrayBuffer(), opts);
}

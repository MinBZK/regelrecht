/**
 * Synthetic corpus graph at real scale.
 *
 * The data layer that reads the corpus and precomputes the layout is built
 * elsewhere; this generator stands in for it and produces exactly the shape
 * that layer promises:
 *
 *   node: id, label, x, y, z, kind, weight
 *   edge: source, target, type
 *
 * It emits that shape in the packed (typed-array) form the renderer consumes,
 * because the whole point is to run at corpus scale - 4.138 laws and millions
 * of citation lines - where an array of plain objects per node and per edge is
 * itself the bottleneck. `packGraph.js` converts the object form (what a small
 * JSON endpoint returns) into the same packed form, so both routes meet.
 *
 * Everything is deterministic given `seed`: same seed, same graph, same
 * positions. That mirrors the design's stability requirement (a law sits
 * tomorrow where it sat today) and makes the benchmarks comparable.
 */

import { KIND_IDS, EDGE_TYPE_IDS, STATUS_IDS } from './graphSchema.js';

/** Small, fast, seedable PRNG (mulberry32). */
export function mulberry32(seed) {
  let a = seed >>> 0;
  return function random() {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Box-Muller, one sample per call (the second is discarded; cheap enough). */
function gaussian(rnd) {
  const u = Math.max(rnd(), 1e-9);
  const v = rnd();
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}

// Distribution of regulatory layers, roughly what BWB looks like: mostly
// wetten and AMvB's, a long tail of ministeriële regelingen and policy rules.
const KIND_MIX = [
  ['law', 0.52],
  ['amvb', 0.2],
  ['ministeriele_regeling', 0.16],
  ['beleidsregel', 0.07],
  ['uitvoeringsdocument', 0.05],
];

// Enrichment status mix: most of the corpus is merely harvested.
const STATUS_MIX = [
  ['harvested', 0.72],
  ['enriched', 0.2],
  ['validated', 0.08],
];

function pickWeighted(mix, r) {
  let acc = 0;
  for (const [value, p] of mix) {
    acc += p;
    if (r < acc) return value;
  }
  return mix[mix.length - 1][0];
}

const WORDS_A = [
  'wet',
  'besluit',
  'regeling',
  'beleidsregel',
  'verordening',
  'uitvoeringsbesluit',
];
const WORDS_B = [
  'inkomstenbelasting',
  'zorgtoeslag',
  'huurtoeslag',
  'kinderopvang',
  'participatie',
  'omgeving',
  'onderwijs',
  'arbeid',
  'vreemdelingen',
  'milieu',
  'ruimtelijke ordening',
  'sociale verzekeringen',
  'bijstand',
  'studiefinanciering',
  'toeslagen',
  'algemene bepalingen',
];
const WORDS_C = [
  '',
  'nadere regels',
  'overgangsrecht',
  'uitvoering',
  'vaststelling',
  'wijziging',
];

function makeLabel(i, rnd) {
  const a = WORDS_A[(rnd() * WORDS_A.length) | 0];
  const b = WORDS_B[(rnd() * WORDS_B.length) | 0];
  const c = WORDS_C[(rnd() * WORDS_C.length) | 0];
  const head = a.charAt(0).toUpperCase() + a.slice(1);
  return c ? `${head} ${b} (${c}) ${1900 + (i % 126)}` : `${head} ${b} ${1900 + (i % 126)}`;
}

/**
 * @param {object} opts
 * @param {number} opts.nodeCount     laws in the graph
 * @param {number} opts.edgeCount     citation edges (aggregated law -> law)
 * @param {number} [opts.seed]
 * @param {number} [opts.clusters]    number of communities
 * @param {number} [opts.hubs]        framework laws (Awb/Awir-shaped stars)
 * @param {number} [opts.hubShare]    fraction of edges that start at a hub
 * @param {boolean} [opts.labels]     materialise the label strings (default true)
 * @returns {import('./graphSchema.js').PackedGraph}
 */
export function generateCorpusGraph({
  nodeCount,
  edgeCount,
  seed = 1,
  clusters = 7,
  hubs = 6,
  hubShare = 0.18,
  labels = true,
} = {}) {
  if (!Number.isInteger(nodeCount) || nodeCount < 1) {
    throw new Error('nodeCount must be a positive integer');
  }
  if (!Number.isInteger(edgeCount) || edgeCount < 0) {
    throw new Error('edgeCount must be a non-negative integer');
  }
  const rnd = mulberry32(seed);
  const clusterCount = Math.max(1, Math.min(clusters, nodeCount));

  const positions = new Float32Array(nodeCount * 3);
  const kind = new Uint8Array(nodeCount);
  const status = new Uint8Array(nodeCount);
  const cluster = new Uint8Array(nodeCount);
  const weight = new Float32Array(nodeCount);
  const labelArr = labels ? new Array(nodeCount) : null;

  // Cluster centres spread over a sphere with the Fibonacci lattice, so the
  // communities are evenly separated instead of randomly clumped.
  const radius = 120 * Math.cbrt(nodeCount / 4138);
  const centres = new Float32Array(clusterCount * 3);
  const golden = Math.PI * (3 - Math.sqrt(5));
  for (let c = 0; c < clusterCount; c++) {
    const y = clusterCount === 1 ? 0 : 1 - (c / (clusterCount - 1)) * 2;
    const r = Math.sqrt(Math.max(0, 1 - y * y));
    const theta = golden * c;
    centres[c * 3] = Math.cos(theta) * r * radius;
    centres[c * 3 + 1] = y * radius;
    centres[c * 3 + 2] = Math.sin(theta) * r * radius;
  }

  const spread = radius * 0.42;
  for (let i = 0; i < nodeCount; i++) {
    const c = (rnd() * clusterCount) | 0;
    cluster[i] = c;
    const isHub = i < hubs;
    // Framework laws sit near the centre of the whole corpus: they belong to
    // no single community, which is exactly what makes them framework laws.
    const cx = isHub ? 0 : centres[c * 3];
    const cy = isHub ? 0 : centres[c * 3 + 1];
    const cz = isHub ? 0 : centres[c * 3 + 2];
    const s = isHub ? spread * 0.25 : spread;
    positions[i * 3] = cx + gaussian(rnd) * s;
    positions[i * 3 + 1] = cy + gaussian(rnd) * s;
    positions[i * 3 + 2] = cz + gaussian(rnd) * s;

    kind[i] = KIND_IDS[isHub ? 'law' : pickWeighted(KIND_MIX, rnd())];
    status[i] = STATUS_IDS[pickWeighted(STATUS_MIX, rnd())];
    // Reverse-PageRank stand-in: a Pareto tail, hubs pinned at the top.
    weight[i] = isHub ? 60 + rnd() * 40 : Math.pow(1 - rnd() * 0.999, -0.7);
    if (labelArr) labelArr[i] = makeLabel(i, rnd);
  }

  const edgeSource = new Uint32Array(edgeCount);
  const edgeTarget = new Uint32Array(edgeCount);
  const edgeType = new Uint8Array(edgeCount);

  // Bucket node indices per cluster so within-community edges are one lookup
  // instead of a rejection loop.
  const clusterMembers = [];
  for (let c = 0; c < clusterCount; c++) clusterMembers.push([]);
  for (let i = 0; i < nodeCount; i++) clusterMembers[cluster[i]].push(i);

  const hubCount = Math.min(hubs, nodeCount);
  const citation = EDGE_TYPE_IDS.citation;
  for (let e = 0; e < edgeCount; e++) {
    const roll = rnd();
    let s;
    let t;
    if (hubCount > 0 && roll < hubShare) {
      // Framework-law star: one hub, any law. This is the shape the design
      // singles out as the thing that breaks naive layout and rendering.
      s = (rnd() * hubCount) | 0;
      t = (rnd() * nodeCount) | 0;
    } else if (roll < 0.82) {
      const members = clusterMembers[(rnd() * clusterCount) | 0];
      if (members.length < 2) {
        s = (rnd() * nodeCount) | 0;
        t = (rnd() * nodeCount) | 0;
      } else {
        s = members[(rnd() * members.length) | 0];
        t = members[(rnd() * members.length) | 0];
      }
    } else {
      s = (rnd() * nodeCount) | 0;
      t = (rnd() * nodeCount) | 0;
    }
    if (s === t) t = (t + 1) % nodeCount;
    edgeSource[e] = s;
    edgeTarget[e] = t;
    edgeType[e] = citation;
  }

  return {
    nodeCount,
    edgeCount,
    positions,
    kind,
    status,
    cluster,
    weight,
    labels: labelArr,
    edgeSource,
    edgeTarget,
    edgeType,
  };
}

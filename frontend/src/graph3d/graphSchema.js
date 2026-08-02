/**
 * The wire contract between the graph data layer and the renderer.
 *
 * Fixed fields, agreed with the data layer:
 *   node: id, label, x, y, z, kind, weight
 *   edge: source, target, type
 *
 * `kind` is `law` for the whole corpus today; the renderer already carries the
 * other regulatory layers because they decide the geometry family and adding
 * one later must not touch the renderer. Anything the renderer does not know
 * falls back to the default family instead of throwing - an unknown kind is a
 * data-layer extension, not a rendering error.
 *
 * The packed form is typed arrays, not objects. At 100.000 nodes an array of
 * plain objects costs tens of megabytes and a garbage-collection pause per
 * rebuild; the typed arrays are what the binary endpoint hands over anyway.
 *
 * @typedef {object} PackedGraph
 * @property {number} nodeCount
 * @property {number} edgeCount
 * @property {Float32Array} positions   xyz per node
 * @property {Uint8Array} kind          KIND_IDS value per node
 * @property {Uint8Array} status        STATUS_IDS value per node
 * @property {Uint8Array} cluster       community id per node
 * @property {Float32Array} weight      centrality stand-in per node
 * @property {string[]|null} labels     null when labels were not materialised
 * @property {Uint32Array} edgeSource
 * @property {Uint32Array} edgeTarget
 * @property {Uint8Array} edgeType      EDGE_TYPE_IDS value per edge
 */

/** Geometry family per node kind. Index into GEOMETRY_FAMILIES. */
export const KIND_IDS = Object.freeze({
  law: 0,
  amvb: 1,
  ministeriele_regeling: 2,
  beleidsregel: 3,
  uitvoeringsdocument: 4,
  artikel: 5,
});

export const KIND_NAMES = Object.freeze(
  Object.keys(KIND_IDS).sort((a, b) => KIND_IDS[a] - KIND_IDS[b]),
);

/**
 * One InstancedMesh per family, so the whole corpus draws in six calls.
 * Names map onto three.js geometry constructors in `nodeLayer.js`.
 */
export const GEOMETRY_FAMILIES = Object.freeze([
  'sphere', // wet
  'box', // AMvB
  'octahedron', // ministeriële regeling
  'tetrahedron', // beleidsregel
  'cylinder', // uitvoeringsdocument
  'smallSphere', // artikel
]);

export const EDGE_TYPE_IDS = Object.freeze({
  citation: 0,
  definition: 1,
  delegation: 2,
  applicability: 3,
  amendment: 4,
});

export const EDGE_TYPE_NAMES = Object.freeze(
  Object.keys(EDGE_TYPE_IDS).sort((a, b) => EDGE_TYPE_IDS[a] - EDGE_TYPE_IDS[b]),
);

/**
 * Enrichment status per node, and the whole basis of the colour rule: grey is
 * the resting state and almost everything, colour means somebody has worked on
 * this law. `enriching` is the live state - the enricher is inside this law
 * right now - and it exists here before there is a feed for it, so the renderer
 * can accept the update the moment the data layer sends one.
 */
export const STATUS_IDS = Object.freeze({
  harvested: 0,
  enriched: 1,
  validated: 2,
  enriching: 3,
});

/** Colour and interaction belong to these; harvested is grey and passive. */
export function isEnriched(status) {
  return status === STATUS_IDS.enriched || status === STATUS_IDS.validated || status === STATUS_IDS.enriching;
}

export const STATUS_NAMES = Object.freeze(
  Object.keys(STATUS_IDS).sort((a, b) => STATUS_IDS[a] - STATUS_IDS[b]),
);

/** Kind id for a wire value, falling back to the default family. */
export function kindId(kind) {
  const id = KIND_IDS[kind];
  return id === undefined ? KIND_IDS.law : id;
}

export function edgeTypeId(type) {
  const id = EDGE_TYPE_IDS[type];
  return id === undefined ? EDGE_TYPE_IDS.citation : id;
}

export function statusId(status) {
  const id = STATUS_IDS[status];
  return id === undefined ? STATUS_IDS.harvested : id;
}

/**
 * scene — the level-truncated graph the three prototypes all lay out.
 *
 * `buildScene(model, level)` is the shared first half of every prototype: pick
 * the units for the level (`archIndex.unitsAtLevel`), roll every relation up to
 * them (`archRollup.rollupRelations`), and hand back a plain graph plus the
 * numbers that prove nothing was dropped.
 *
 * It is deliberately geometry-free. `mapLayout` / `radialLayout` /
 * `matrixLayout` each turn the *same* scene into coordinates, which is the only
 * reason the three prototypes can be compared at all: any difference you see is
 * the schema technique, not a different graph.
 */
import { buildIndex, containerOf, levelRank, unitsAtLevel } from '../lib/archIndex.js';
import { rollupRelations } from '../lib/archRollup.js';

/** Every relation kind the model produces. */
export const ALL_EDGE_KINDS = Object.freeze(['depends-on', 'impl', 'uses']);

/**
 * @param {object} model                the model from `GET /api/model`
 * @param {string} level                'container' | 'component' | 'code'
 * @param {object} [opts]
 * @param {object} [opts.index]         a reusable `buildIndex(model)` result
 * @param {Set<string>} [opts.enabledKinds]
 * @returns {{
 *   level: string,
 *   index: object,
 *   units: Array<{id,node,name,kind,level,container,degree,internal,index:number}>,
 *   unitById: Map<string, object>,
 *   links: Array<{id,from,to,kind,weight,pairs}>,
 *   stats: {units:number,visible:number,total:number,internal:number,unplaced:number,modelNodes:number,modelEdges:number},
 * }}
 */
export function buildScene(model, level, opts = {}) {
  const index = opts.index || buildIndex(model);
  const enabledKinds = opts.enabledKinds || new Set(ALL_EDGE_KINDS);

  const { units: unitIds, unitSet } = unitsAtLevel(index, level);

  // The flat prototypes draw a parent and its child side by side, so a relation
  // between them is a real line here — unlike in the nested Vue Flow view,
  // where nesting already shows it.
  const { aggregates, internal, degree, stats } = rollupRelations({
    edges: model.edges,
    visibleIds: unitSet,
    parentOf: index.parentOf,
    enabledKinds,
    isAncestor: index.isAncestor,
    containmentAsNesting: false,
  });

  const units = unitIds.map((id, i) => {
    const node = index.byId.get(id);
    return {
      id,
      node,
      name: unitLabel(node, id, level),
      kind: node?.kind || 'unknown',
      level: node?.level || level,
      container: containerOf(index, id),
      degree: degree.get(id) || 0,
      internal: internal.get(id) || 0,
      index: i,
    };
  });
  const unitById = new Map(units.map((u) => [u.id, u]));

  const links = [...aggregates.values()].map((a) => ({
    id: `${a.kind}|${a.from}->${a.to}`,
    from: a.from,
    to: a.to,
    kind: a.kind,
    weight: a.weight,
    pairs: a.pairs,
  }));

  return {
    level,
    index,
    units,
    unitById,
    links,
    stats: {
      units: units.length,
      visible: stats.visible,
      total: stats.total,
      internal: stats.internal,
      unplaced: stats.unplaced,
      modelNodes: model.nodes.length,
      modelEdges: model.edges.length,
    },
  };
}

/**
 * A unit's label. Normally the node's own name — but a node that only became a
 * unit because it had no ancestor at this level (see `unitsAtLevel`) is shown
 * with its parent segment too, since a bare `from` sitting between the crates
 * is otherwise unreadable.
 */
function unitLabel(node, id, level) {
  if (!node) return id;
  if (levelRank(node.level) <= levelRank(level)) return node.name || id;
  const parts = id.split('::');
  return parts.slice(-2).join('::') || node.name || id;
}

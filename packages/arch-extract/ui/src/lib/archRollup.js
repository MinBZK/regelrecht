/**
 * archRollup — lift every relation to whatever detail level is on screen and
 * aggregate the result.
 *
 * This is the logic that used to live inside `useArchGraph.buildFlow()`. It is
 * extracted because *all four* renderings need exactly the same rollup: the
 * current expand/collapse view, and the Map / Radial / Matrix prototypes.
 * Sharing it is what makes the three prototypes comparable — they draw the same
 * numbers, only differently.
 *
 * The contract: **no relation disappears silently.** Every enabled edge ends up
 * in exactly one of three buckets, and the totals are reported back:
 *
 *  - an **aggregate** — a line between two different visible nodes, carrying
 *    the count of the underlying relations and the pairs themselves;
 *  - an **internal counter** — both ends roll up to the same visible node, so
 *    there is nothing to draw *between*; it becomes a number on that node;
 *  - **containment** — the two lifted ends are in an ancestor/descendant
 *    relation. In the nested Vue Flow view that is drawn as nesting rather than
 *    a line (`containmentAsNesting: true`). The flat prototypes draw both nodes
 *    side by side, so there nesting is not a substitute and the relation is
 *    kept as a normal line (`containmentAsNesting: false`).
 */

/**
 * @param {object} args
 * @param {Array<{from:string,to:string,kind:string}>} args.edges  model edges
 * @param {Set<string>} args.visibleIds     nodes currently on screen
 * @param {(id:string)=>string|undefined} args.parentOf
 * @param {Set<string>} args.enabledKinds   edge kinds to include
 * @param {(anc:string,desc:string)=>boolean} args.isAncestor
 * @param {boolean} [args.containmentAsNesting=true]
 * @returns {{
 *   aggregates: Map<string, {kind,from,to,weight,pairs}>,
 *   internal: Map<string, number>,
 *   degree: Map<string, number>,
 *   stats: {visible:number,total:number,internal:number,containment:number,unplaced:number},
 * }}
 */
export function rollupRelations({
  edges,
  visibleIds,
  parentOf,
  enabledKinds,
  isAncestor,
  containmentAsNesting = true,
}) {
  // Lift an id to its nearest visible ancestor. Roots are visible in every
  // level selection, so the walk terminates; it returns undefined only for an
  // id that is not in the tree at all (a dangling edge endpoint).
  const lift = (id) => {
    let cur = id;
    while (cur && !visibleIds.has(cur)) cur = parentOf(cur);
    return cur;
  };

  const aggregates = new Map(); // `${kind}|${from}->${to}` -> aggregate
  const internal = new Map(); // nodeId -> internal relation count
  const degree = new Map(); // nodeId -> rolled-up relations touching it
  let total = 0;
  let visible = 0;
  let internalCount = 0;
  let containment = 0;
  let unplaced = 0;

  const bump = (id, n) => degree.set(id, (degree.get(id) || 0) + n);

  for (const e of edges) {
    if (!enabledKinds.has(e.kind)) continue;
    const from = lift(e.from);
    const to = lift(e.to);
    if (!from || !to) {
      unplaced += 1;
      continue; // dangling endpoint, nothing to draw
    }
    total += 1;

    if (from === to) {
      internal.set(from, (internal.get(from) || 0) + 1);
      internalCount += 1;
      visible += 1;
      continue;
    }
    if (containmentAsNesting && (isAncestor(from, to) || isAncestor(to, from))) {
      containment += 1;
      continue;
    }

    const key = `${e.kind}|${from}->${to}`;
    let agg = aggregates.get(key);
    if (!agg) {
      agg = { kind: e.kind, from, to, weight: 0, pairs: [] };
      aggregates.set(key, agg);
    }
    agg.weight += 1;
    agg.pairs.push({ from: e.from, to: e.to });
    bump(from, 1);
    bump(to, 1);
    visible += 1;
  }

  return {
    aggregates,
    internal,
    degree,
    stats: { visible, total, internal: internalCount, containment, unplaced },
  };
}

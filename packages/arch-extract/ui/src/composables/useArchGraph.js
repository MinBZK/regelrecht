/**
 * useArchGraph — turns the code-derived architecture model into a lazily
 * expanded Vue Flow graph.
 *
 * The model (from `GET /api/model`) is a containment tree of ~2200 nodes
 * (crate → module → type → method) plus a thin set of relationship edges
 * (`depends-on`, `impl`, `uses`). Rendering all of it at once does not work, so
 * this composable starts at the crate level (nodes with no `parent`) and only
 * materialises a node's children when that node is expanded.
 *
 * Layout mirrors the nested-node approach of the editor's `useLawGraph.js`:
 * children are Vue Flow child nodes (`parentNode` + `extent: 'parent'`) with
 * positions relative to their parent, and each expanded parent is grown to a
 * padded grid that contains its (recursively laid out) children. Collapsed and
 * leaf nodes get a fixed header-sized box.
 *
 * Relationships are **edge-lifted**: every edge in the model is always
 * accounted for, rolled up to whatever detail level is currently on screen.
 * Each endpoint is lifted to its nearest *visible* ancestor; edges that lift to
 * the same node become an internal counter on that node instead of a line;
 * edges whose lifted ends are in an ancestor/descendant relation are containment
 * and drawn as nesting, not a line; the rest are aggregated per
 * `kind|from->to` into one weighted line. Expanding a node refines the lines
 * that touched it. See packages/arch-extract/README.md.
 *
 * That lifting/aggregation step now lives in `../lib/archRollup.js`, because the
 * Map / Radiaal / Matrix prototypes need exactly the same rollup — sharing it is
 * what makes the four views comparable. What stayed here is the part that is
 * specific to this view: the expand/collapse state and the nested grid layout.
 * This whole file goes away with the comparison rig; see ui/EVALUATIE.md.
 */
import { computed, ref, shallowRef } from 'vue';
import { MarkerType } from '@vue-flow/core';
import { useEdgeFilters } from './useEdgeFilters.js';
import { sortChildren } from '../lib/archIndex.js';
import { rollupRelations } from '../lib/archRollup.js';

// --- Layout constants -------------------------------------------------------
const NODE_W = 220; // width of a collapsed / leaf node
const HEADER_H = 46; // height of a node's own header strip
const PAD = 20; // inner padding around a parent's children
const GAP = 18; // gap between sibling children
const MAX_COLS = 5; // cap the grid width so deep crates don't sprawl sideways

// Layered root layout (see layoutRoots). Roots become topological layers on the
// aggregated `depends-on` graph so dependencies overwhelmingly flow one way.
const ROOT_LAYER_GAP_X = 140; // horizontal gap between dependency layers
const ROOT_COL_GAP_X = 70; // gap between wrapped sub-columns inside one layer
const ROOT_GAP_Y = 80; // vertical gap between stacked roots
const ROOT_MAX_PER_COL = 4; // wrap a layer into a new sub-column past this many

// Above this many underlying pairs, revealing a rolled-up line opens only one
// level instead of every ancestor chain, so one click never opens hundreds of
// nodes. Repeated clicks refine step by step. (Acceptance criterion 7.)
export const REVEAL_LIMIT = 25;

// Per-edge-kind styling. Visually distinct so `depends-on` / `impl` / `uses`
// are told apart at a glance (acceptance criterion). `strokeWidth` is the
// *base* dikte per soort; a rolled-up line scales up logarithmically from it
// (see scaledStrokeWidth) so the colour coding stays intact.
const EDGE_STYLE = {
  'depends-on': { stroke: '#6366f1', strokeWidth: 2.5 },
  impl: { stroke: '#10b981', strokeWidth: 2, strokeDasharray: '7 4' },
  uses: { stroke: '#94a3b8', strokeWidth: 1.5, strokeDasharray: '2 4' },
  calls: { stroke: '#f59e0b', strokeWidth: 1.5 },
};

export { EDGE_STYLE };

/**
 * Log-scale a rolled-up line's width off the per-kind base. weight 1 → base
 * (so a single relation keeps its kind's dikte); heavier lines grow toward a
 * ~9px cap. (Acceptance criterion 4.)
 */
function scaledStrokeWidth(base, weight) {
  const w = base + Math.log2(Math.max(1, weight)) * 1.2;
  return Math.min(9, Math.max(1.5, Math.round(w * 10) / 10));
}

/**
 * Order the crate/app roots into topological layers on the aggregated
 * `depends-on` relations, so dependencies overwhelmingly point one way instead
 * of sitting in an alphabetical row. This is a structural port of
 * `frontend/src/composables/useLawGraph.js` → `applyLayeredLayout()`
 * (Kahn-style layering with `dependents`/`dependencies`/`incomingCount` and
 * column wrapping); the explorer is a standalone npm project that cannot import
 * from `frontend/`, so the algorithm is reproduced here. Keep the two in step.
 *
 * Unlike the law graph it is size-aware: layer columns are spaced by the actual
 * node widths/heights (roots vary wildly once expanded) rather than a fixed
 * pitch, so expanded crates do not overlap.
 *
 * Writes absolute positions into `pos` for every root id.
 */
function layoutRoots(roots, rootDepends, sizes, pos) {
  const rootSet = new Set(roots);
  const dependents = new Map(); // id → ids that depend on it
  const dependencies = new Map(); // id → ids it depends on
  const incomingCount = new Map();
  for (const id of roots) {
    dependents.set(id, new Set());
    dependencies.set(id, new Set());
    incomingCount.set(id, 0);
  }

  for (const { from, to } of rootDepends) {
    if (from === to || !rootSet.has(from) || !rootSet.has(to)) continue;
    // `from depends-on to`: `to` gains a dependent, `from` gains a dependency.
    if (!dependents.get(to).has(from)) {
      dependents.get(to).add(from);
      dependencies.get(from).add(to);
      incomingCount.set(from, incomingCount.get(from) + 1);
    }
  }

  const layers = [];
  const processed = new Set();
  let currentLayer = roots.filter((id) => incomingCount.get(id) === 0);

  while (currentLayer.length > 0) {
    layers.push(currentLayer);
    for (const id of currentLayer) processed.add(id);
    const next = new Set();
    for (const id of currentLayer) {
      for (const dependent of dependents.get(id) || []) {
        if (processed.has(dependent)) continue;
        let allDepsDone = true;
        for (const dep of dependencies.get(dependent) || []) {
          if (!processed.has(dep)) {
            allDepsDone = false;
            break;
          }
        }
        if (allDepsDone) next.add(dependent);
      }
    }
    currentLayer = [...next];
  }
  // Any root left over (a dependency cycle) goes in a final layer so nothing
  // is dropped.
  const leftover = roots.filter((id) => !processed.has(id));
  if (leftover.length > 0) layers.push(leftover);

  // Place each layer as one or more stacked sub-columns, spaced by real sizes.
  let layerX = 0;
  for (const layer of layers) {
    let colX = layerX;
    let layerRight = layerX;
    for (let start = 0; start < layer.length; start += ROOT_MAX_PER_COL) {
      const chunk = layer.slice(start, start + ROOT_MAX_PER_COL);
      let colW = 0;
      let y = 0;
      for (const id of chunk) {
        const s = sizes.get(id) || { w: NODE_W, h: HEADER_H };
        pos.set(id, { x: colX, y });
        colW = Math.max(colW, s.w);
        y += s.h + ROOT_GAP_Y;
      }
      colX += colW + ROOT_COL_GAP_X;
      layerRight = colX;
    }
    layerX = layerRight + ROOT_LAYER_GAP_X;
  }
}

/**
 * Build the Vue Flow `nodes` + `edges` for the current `expanded` set and the
 * currently-enabled edge kinds.
 *
 * Pure function of (model, expanded, enabledKinds) so the whole lifting /
 * aggregation pipeline is testable without a DOM. `enabledKinds` is a Set of
 * edge kinds to include; anything not in it is filtered out *before*
 * aggregation, so weights and counters never include a disabled kind.
 */
function buildFlow(model, expanded, enabledKinds) {
  const byId = new Map(model.nodes.map((n) => [n.id, n]));
  const parentOf = (id) => byId.get(id)?.parent;

  const childrenMap = new Map();
  for (const n of model.nodes) {
    if (!n.parent) continue;
    if (!childrenMap.has(n.parent)) childrenMap.set(n.parent, []);
    childrenMap.get(n.parent).push(n.id);
  }
  const cmp = sortChildren(byId);
  for (const arr of childrenMap.values()) arr.sort(cmp);

  const roots = model.nodes
    .filter((n) => !n.parent)
    .map((n) => n.id)
    .sort(cmp);

  const sizes = new Map(); // id -> { w, h }
  const pos = new Map(); // id -> { x, y } (relative to parent; absolute for roots)

  const isExpandable = (id) => childrenMap.has(id);
  const visibleChildren = (id) => (expanded.has(id) ? childrenMap.get(id) || [] : []);

  // Bottom-up: size a node from its (recursively sized) visible children.
  function layout(id) {
    const kids = visibleChildren(id);
    if (kids.length === 0) {
      const size = { w: NODE_W, h: HEADER_H };
      sizes.set(id, size);
      return size;
    }

    const boxes = kids.map((k) => ({ id: k, ...layout(k) }));
    const cols = Math.min(Math.max(1, Math.ceil(Math.sqrt(boxes.length))), MAX_COLS);
    const rows = Math.ceil(boxes.length / cols);

    const colW = new Array(cols).fill(0);
    const rowH = new Array(rows).fill(0);
    boxes.forEach((b, i) => {
      const c = i % cols;
      const r = Math.floor(i / cols);
      colW[c] = Math.max(colW[c], b.w);
      rowH[r] = Math.max(rowH[r], b.h);
    });

    const colX = [];
    let cx = PAD;
    for (let c = 0; c < cols; c++) {
      colX[c] = cx;
      cx += colW[c] + GAP;
    }
    const innerW = cx - GAP + PAD;

    const rowY = [];
    let cy = HEADER_H;
    for (let r = 0; r < rows; r++) {
      rowY[r] = cy;
      cy += rowH[r] + GAP;
    }
    const innerH = cy - GAP + PAD;

    boxes.forEach((b, i) => {
      const c = i % cols;
      const r = Math.floor(i / cols);
      pos.set(b.id, { x: colX[c], y: rowY[r] });
    });

    const size = { w: Math.max(innerW, NODE_W), h: Math.max(innerH, HEADER_H) };
    sizes.set(id, size);
    return size;
  }

  roots.forEach(layout);

  // Root positions: topological layers on the aggregated `depends-on` graph.
  // Aggregate to the root level regardless of the kind filter so the layout is
  // stable as filters toggle.
  const rootOf = (id) => {
    let cur = id;
    while (cur && byId.get(cur)?.parent) cur = byId.get(cur).parent;
    return cur;
  };
  const rootDepends = [];
  for (const e of model.edges) {
    if (e.kind !== 'depends-on') continue;
    const rf = rootOf(e.from);
    const rt = rootOf(e.to);
    if (rf && rt && rf !== rt) rootDepends.push({ from: rf, to: rt });
  }
  layoutRoots(roots, rootDepends, sizes, pos);

  // Emit Vue Flow nodes in pre-order (parent before child, as Vue Flow expects).
  // Accumulate absolute positions so edge-lifting can pick a handle side from
  // the two visible endpoints' centres.
  const flowNodes = [];
  const absCenter = new Map(); // id -> { x, y } absolute centre
  const emit = (id, depth, parentAbs) => {
    const n = byId.get(id);
    const size = sizes.get(id);
    const p = pos.get(id) || { x: 0, y: 0 };
    const abs = { x: parentAbs.x + p.x, y: parentAbs.y + p.y };
    absCenter.set(id, { x: abs.x + size.w / 2, y: abs.y + size.h / 2 });
    const exp = expanded.has(id);
    const expandable = isExpandable(id);
    flowNodes.push({
      id,
      type: 'arch',
      position: { x: p.x, y: p.y },
      ...(n.parent ? { parentNode: n.parent, extent: 'parent' } : {}),
      draggable: false,
      data: {
        node: n,
        expandable,
        expanded: exp,
        childCount: (childrenMap.get(id) || []).length,
        // Set below, once edges are lifted: relations that roll up to this node.
        internalCount: 0,
      },
      class: [
        'arch-node',
        `kind-${n.kind}`,
        `level-${n.level}`,
        exp ? 'is-expanded' : '',
        expandable ? 'is-expandable' : '',
      ]
        .filter(Boolean)
        .join(' '),
      style: { width: `${size.w}px`, height: `${size.h}px` },
      // depth + 1 so every node outranks the edge layer (zIndex 0 below);
      // deeper nodes still stack above their own container.
      zIndex: depth + 1,
    });
    for (const k of visibleChildren(id)) emit(k, depth + 1, abs);
  };
  roots.forEach((id) => emit(id, 0, { x: 0, y: 0 }));

  const visibleIds = new Set(flowNodes.map((n) => n.id));

  // Is `anc` a (strict) ancestor of `desc`?
  const isAncestor = (anc, desc) => {
    let cur = parentOf(desc);
    while (cur) {
      if (cur === anc) return true;
      cur = parentOf(cur);
    }
    return false;
  };

  // --- Edge lifting + aggregation ------------------------------------------
  // Shared with the Map / Radial / Matrix prototypes (see lib/archRollup.js).
  // In this nested view containment *is* drawn — as nesting — so those
  // relations must not also become a line.
  const { aggregates, internal, stats } = rollupRelations({
    edges: model.edges,
    visibleIds,
    parentOf,
    enabledKinds,
    isAncestor,
    containmentAsNesting: true,
  });

  // Publish internal counters onto their nodes.
  for (const n of flowNodes) {
    const c = internal.get(n.id);
    if (c) n.data.internalCount = c;
  }

  // Choose a handle side per aggregated edge from the two centres' dominant
  // axis, so lines leave/enter on the facing sides instead of always L→R.
  const pickHandles = (a, b) => {
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    if (Math.abs(dx) >= Math.abs(dy)) {
      return dx >= 0 ? ['source-right', 'target-left'] : ['source-left', 'target-right'];
    }
    return dy >= 0 ? ['source-bottom', 'target-top'] : ['source-top', 'target-bottom'];
  };

  const flowEdges = [];
  for (const agg of aggregates.values()) {
    const base = EDGE_STYLE[agg.kind] || EDGE_STYLE.uses;
    const strokeWidth = scaledStrokeWidth(base.strokeWidth, agg.weight);
    const ca = absCenter.get(agg.from) || { x: 0, y: 0 };
    const cb = absCenter.get(agg.to) || { x: 0, y: 0 };
    const [sourceHandle, targetHandle] = pickHandles(ca, cb);
    flowEdges.push({
      id: `${agg.kind}|${agg.from}->${agg.to}`,
      source: agg.from,
      target: agg.to,
      sourceHandle,
      targetHandle,
      class: `edge-${agg.kind}`,
      type: 'arch',
      data: {
        kind: agg.kind,
        weight: agg.weight,
        from: agg.from,
        to: agg.to,
        pairs: agg.pairs,
      },
      style: { ...base, strokeWidth },
      markerEnd: { type: MarkerType.ArrowClosed, color: base.stroke, width: 16, height: 16 },
      // Below every node (which start at 1). Keeps Vue Flow's invisible 20px
      // interaction path from stealing clicks off the node toggles.
      zIndex: 0,
    });
  }

  return {
    nodes: flowNodes,
    edges: flowEdges,
    childrenMap,
    stats: { visible: stats.visible, total: stats.total },
  };
}

export { buildFlow };

/**
 * Compute the next `expanded` set when a rolled-up line's badge is clicked.
 *
 * Pure so the reveal policy is unit-testable. Below `limit` underlying pairs it
 * fully reveals — expands every ancestor of every underlying endpoint, so the
 * line splits into its exact sub-lines. At or above the limit it opens only one
 * level (expands the two currently-lifted endpoints), so a single click never
 * opens hundreds of nodes; repeated clicks refine step by step. (Criteria 6, 7.)
 */
export function computeReveal({ parentOf, childrenMap, expanded, data, limit = REVEAL_LIMIT }) {
  const next = new Set(expanded);
  const addAncestors = (id) => {
    let cur = parentOf.get(id);
    while (cur) {
      next.add(cur);
      cur = parentOf.get(cur);
    }
  };

  if (data.pairs.length > limit) {
    if (childrenMap.has(data.from)) next.add(data.from);
    if (childrenMap.has(data.to)) next.add(data.to);
  } else {
    for (const { from, to } of data.pairs) {
      addAncestors(from);
      addAncestors(to);
    }
  }
  return next;
}

export function useArchGraph() {
  const model = shallowRef(null);
  const expanded = ref(new Set());
  const { enabledKinds, toggleKind, kindEnabled, FILTERABLE_KINDS } = useEdgeFilters();

  // Cache maps for subtree expansion / reveal without rebuilding them.
  let childrenMap = new Map();
  let parentOf = new Map();

  const built = computed(() => {
    if (!model.value) return { nodes: [], edges: [], stats: { visible: 0, total: 0 } };
    const result = buildFlow(model.value, expanded.value, enabledKinds.value);
    childrenMap = result.childrenMap;
    return result;
  });

  const nodes = computed(() => built.value.nodes);
  const edges = computed(() => built.value.edges);
  const stats = computed(() => built.value.stats);

  function setModel(m) {
    model.value = m;
    expanded.value = new Set(); // start collapsed at crate level
    parentOf = new Map(m.nodes.map((n) => [n.id, n.parent || null]));
  }

  function toggle(id) {
    const next = new Set(expanded.value);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expanded.value = next;
  }

  /** Expand a node and every descendant — "fully unfold this crate". */
  function expandSubtree(id) {
    const next = new Set(expanded.value);
    const stack = [id];
    while (stack.length > 0) {
      const cur = stack.pop();
      if (!childrenMap.has(cur)) continue;
      next.add(cur);
      for (const c of childrenMap.get(cur)) stack.push(c);
    }
    expanded.value = next;
  }

  function collapseAll() {
    expanded.value = new Set();
  }

  /**
   * Reveal a rolled-up line. Returns the ids the caller should `fitView` to
   * (the two lifted endpoints, which contain the newly-split lines).
   */
  function revealEdge(data) {
    expanded.value = computeReveal({ parentOf, childrenMap, expanded: expanded.value, data });
    return [data.from, data.to];
  }

  return {
    model,
    nodes,
    edges,
    stats,
    expanded,
    enabledKinds,
    toggleKind,
    kindEnabled,
    FILTERABLE_KINDS,
    setModel,
    toggle,
    expandSubtree,
    collapseAll,
    revealEdge,
  };
}

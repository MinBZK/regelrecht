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
 */
import { computed, ref, shallowRef } from 'vue';
import { MarkerType } from '@vue-flow/core';

// --- Layout constants -------------------------------------------------------
const NODE_W = 220; // width of a collapsed / leaf node
const HEADER_H = 46; // height of a node's own header strip
const PAD = 20; // inner padding around a parent's children
const GAP = 18; // gap between sibling children
const MAX_COLS = 5; // cap the grid width so deep crates don't sprawl sideways
const ROOT_GAP_X = 80;
const ROOT_GAP_Y = 80;
const ROOT_ROW_MAX_W = 1800; // wrap the crate row past this width

// Sort order for children: coarse kinds first, then alphabetically. Keeps a
// crate's modules above its loose fns, types above their methods, etc. The
// roots (Rust `crate`s and JS `app`s) are grouped so the two tiers do not
// interleave; below an app, directories sort above the files they group.
const KIND_RANK = {
  crate: 0,
  app: 1,
  binary: 2,
  dir: 3,
  module: 4,
  component: 5,
  composable: 6,
  trait: 7,
  struct: 8,
  enum: 9,
  fn: 10,
  method: 11,
};

// Per-edge-kind styling. Visually distinct so `depends-on` / `impl` / `uses`
// are told apart at a glance (acceptance criterion).
const EDGE_STYLE = {
  'depends-on': { stroke: '#6366f1', strokeWidth: 2.5 },
  impl: { stroke: '#10b981', strokeWidth: 2, strokeDasharray: '7 4' },
  uses: { stroke: '#94a3b8', strokeWidth: 1.5, strokeDasharray: '2 4' },
  calls: { stroke: '#f59e0b', strokeWidth: 1.5 },
};

function sortChildren(byId) {
  return (a, b) => {
    const na = byId.get(a);
    const nb = byId.get(b);
    const ra = KIND_RANK[na?.kind] ?? 9;
    const rb = KIND_RANK[nb?.kind] ?? 9;
    if (ra !== rb) return ra - rb;
    return (na?.name || a).localeCompare(nb?.name || b);
  };
}

/**
 * Build the Vue Flow `nodes` + `edges` for the current `expanded` set.
 * Pure function of (model, expanded) so it is trivial to recompute on toggle.
 */
function buildFlow(model, expanded) {
  const byId = new Map(model.nodes.map((n) => [n.id, n]));

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

  // Place crate roots in a wrapping row.
  let rx = 0;
  let ry = 0;
  let rowMaxH = 0;
  for (const id of roots) {
    const { w, h } = sizes.get(id);
    if (rx > 0 && rx + w > ROOT_ROW_MAX_W) {
      rx = 0;
      ry += rowMaxH + ROOT_GAP_Y;
      rowMaxH = 0;
    }
    pos.set(id, { x: rx, y: ry });
    rx += w + ROOT_GAP_X;
    rowMaxH = Math.max(rowMaxH, h);
  }

  // Emit Vue Flow nodes in pre-order (parent before child, as Vue Flow expects).
  const flowNodes = [];
  const emit = (id, depth) => {
    const n = byId.get(id);
    const size = sizes.get(id);
    const p = pos.get(id) || { x: 0, y: 0 };
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
    for (const k of visibleChildren(id)) emit(k, depth + 1);
  };
  roots.forEach((id) => emit(id, 0));

  // Edges between currently-visible nodes only.
  const visibleIds = new Set(flowNodes.map((n) => n.id));
  const flowEdges = [];
  for (const e of model.edges) {
    if (!visibleIds.has(e.from) || !visibleIds.has(e.to)) continue;
    const style = EDGE_STYLE[e.kind] || EDGE_STYLE.uses;
    flowEdges.push({
      id: `${e.kind}:${e.from}->${e.to}`,
      source: e.from,
      target: e.to,
      class: `edge-${e.kind}`,
      data: { kind: e.kind },
      type: 'default',
      style,
      markerEnd: { type: MarkerType.ArrowClosed, color: style.stroke, width: 16, height: 16 },
      // Below every node (which start at 1). Edges used to sit at 2000, which
      // drew the lines over the node boxes and — worse — put Vue Flow's
      // invisible 20px-wide `.vue-flow__edge-interaction` hit path on top of
      // the expand toggles, swallowing the click on most nodes.
      zIndex: 0,
    });
  }

  return { nodes: flowNodes, edges: flowEdges, childrenMap };
}

export function useArchGraph() {
  const model = shallowRef(null);
  const expanded = ref(new Set());

  // Cache childrenMap for subtree expansion without rebuilding the whole map.
  let childrenMap = new Map();

  const built = computed(() => {
    if (!model.value) return { nodes: [], edges: [] };
    const result = buildFlow(model.value, expanded.value);
    childrenMap = result.childrenMap;
    return result;
  });

  const nodes = computed(() => built.value.nodes);
  const edges = computed(() => built.value.edges);

  function setModel(m) {
    model.value = m;
    expanded.value = new Set(); // start collapsed at crate level
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

  return { model, nodes, edges, expanded, setModel, toggle, expandSubtree, collapseAll };
}

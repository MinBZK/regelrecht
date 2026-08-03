/**
 * common — derived, memoised helpers shared by the three prototype renderers.
 *
 * Everything here is a pure function of a layout result, cached per layout
 * object (layouts are immutable and cached themselves, so a WeakMap keyed on
 * the layout is exactly the right lifetime).
 */

const incidenceCache = new WeakMap();
const hubCache = new WeakMap();

/** node id → indices of the relations touching it, in `layout.edges` order. */
export function incidence(layout) {
  let map = incidenceCache.get(layout);
  if (map) return map;
  map = new Map();
  const add = (id, i) => {
    let arr = map.get(id);
    if (!arr) {
      arr = [];
      map.set(id, arr);
    }
    arr.push(i);
  };
  (layout.edges || []).forEach((e, i) => {
    add(e.from, i);
    add(e.to, i);
  });
  incidenceCache.set(layout, map);
  return map;
}

/** The `n` highest-degree units, descending. These always get a label. */
export function topHubs(layout, n = 12) {
  let hubs = hubCache.get(layout);
  if (!hubs) {
    hubs = layout.nodes.slice().sort((a, b) => b.degree - a.degree);
    hubCache.set(layout, hubs);
  }
  return hubs.slice(0, n);
}

/** The world-space rectangle currently on screen, for culling. */
export function visibleRect(env, margin = 40) {
  const { scale, tx, ty, width, height } = env;
  return {
    minX: (0 - tx) / scale - margin,
    minY: (0 - ty) / scale - margin,
    maxX: (width - tx) / scale + margin,
    maxY: (height - ty) / scale + margin,
  };
}

/** Line width for a rolled-up relation: log-scaled, like the current view. */
export function relationWidth(weight, base = 0.6) {
  return base + Math.log2(Math.max(1, weight)) * base * 0.9;
}

/** Fit `text` into `maxWidth` screen pixels, with an ellipsis if it does not. */
export function ellipsize(ctx, text, maxWidth) {
  if (ctx.measureText(text).width <= maxWidth) return text;
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (ctx.measureText(`${text.slice(0, mid)}…`).width <= maxWidth) lo = mid;
    else hi = mid - 1;
  }
  return lo > 0 ? `${text.slice(0, lo)}…` : '';
}

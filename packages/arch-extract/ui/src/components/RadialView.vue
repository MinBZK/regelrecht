<script setup>
/**
 * RadialView — prototype "Radiaal": one ring, bundled relations.
 *
 * Geometry from `layoutRadial` (ring order = containment order, relations
 * routed through the tree and straightened by β). Reading guide:
 *
 *  - the coloured band outside the ring is the container each unit belongs to,
 *    so every subsystem is one contiguous arc;
 *  - a rope that dives deep toward the centre crosses containers; a rope that
 *    hugs the rim stays inside one;
 *  - a relation is drawn from its source colour to a neutral end, so the
 *    direction is readable without arrowheads at this density.
 */
import { ref, shallowRef } from 'vue';
import ArchCanvas from './ArchCanvas.vue';
import { containerColorFactory, edgeColor, kindColor, palette, withAlpha } from '../render/palette.js';
import { ellipsize, incidence, relationWidth, topHubs } from '../render/common.js';

const props = defineProps({
  view: { type: Object, required: true },
});
const emit = defineEmits(['select', 'hover']);

const hoverId = ref(null);
const colorOf = shallowRef(null);

function containerColor(layout, id) {
  if (!colorOf.value) {
    colorOf.value = containerColorFactory(new Set(layout.nodes.map((n) => n.container)));
  }
  return colorOf.value(id);
}

function focusSetsFor(layout) {
  if (!hoverId.value) return null;
  const inc = incidence(layout).get(hoverId.value);
  if (!inc) return null;
  const edges = new Set(inc);
  const nodes = new Set([hoverId.value]);
  for (const i of inc) {
    nodes.add(layout.edges[i].from);
    nodes.add(layout.edges[i].to);
  }
  return { edges, nodes };
}

function ringRadius(layout) {
  return Math.max(...layout.nodes.map((n) => Math.hypot(n.x, n.y)), 1);
}

function drawLayout(ctx, env, layout, alpha, grow) {
  if (!layout || alpha <= 0.01) return;
  const p = palette();
  const focus = focusSetsFor(layout);
  const R = ringRadius(layout);

  // 1. Container band just outside the ring. Butt caps, not the round ones the
  //    bundles use: a one-unit container would otherwise render as a blob far
  //    wider than the slot it owns.
  ctx.lineCap = 'butt';
  ctx.lineWidth = Math.max(6, R * 0.035);
  for (const g of layout.groups) {
    ctx.strokeStyle = withAlpha(containerColor(layout, g.id), 0.75 * alpha);
    ctx.beginPath();
    ctx.arc(0, 0, R + ctx.lineWidth * 1.4, g.startAngle, g.endAngle);
    ctx.stroke();
  }

  // 2. Bundled relations.
  ctx.lineCap = 'round';
  for (let i = 0; i < layout.edges.length; i += 1) {
    const e = layout.edges[i];
    const dim = focus ? (focus.edges.has(i) ? 1 : 0.07) : 1;
    if (dim < 0.15 && alpha < 0.6) continue;
    const pts = e.points;
    if (pts.length < 2) continue;
    const strong = dim > 0.5;
    ctx.strokeStyle = withAlpha(edgeColor(e.kind), (focus ? (strong ? 0.85 : 0.05) : 0.22) * alpha);
    ctx.lineWidth = (relationWidth(e.weight, 1.0) * (strong && focus ? 2 : 1)) / env.scale;
    ctx.beginPath();
    ctx.moveTo(pts[0].x, pts[0].y);
    for (let j = 1; j < pts.length; j += 1) ctx.lineTo(pts[j].x, pts[j].y);
    ctx.stroke();
  }

  // 3. Dots on the ring.
  for (const n of layout.nodes) {
    const dim = focus && !focus.nodes.has(n.id) ? 0.18 : 1;
    ctx.fillStyle = withAlpha(kindColor(n.kind), 0.95 * alpha * dim);
    // World-space radius, but never smaller than ~1.4 screen px, so a
    // low-degree unit stays a visible dot when zoomed out.
    const r = Math.max(n.r * grow, 1.4 / env.scale);
    ctx.beginPath();
    ctx.arc(n.x, n.y, r, 0, Math.PI * 2);
    ctx.fill();
  }

  // Only the layer that is mostly on screen labels itself: two sets of
  // labels during a cross-fade is unreadable.
  if (alpha < 0.5) return;

  // 4. Labels — container names on the band, hub names on the rim.
  ctx.save();
  ctx.setTransform(env.dpr, 0, 0, env.dpr, 0, 0);
  ctx.textBaseline = 'middle';
  for (const g of layout.groups) {
    const mid = (g.startAngle + g.endAngle) / 2;
    const rr = R * 1.1;
    const sx = Math.cos(mid) * rr * env.scale + env.tx;
    const sy = Math.sin(mid) * rr * env.scale + env.ty;
    ctx.font = '600 12px system-ui, sans-serif';
    ctx.textAlign = Math.cos(mid) >= 0 ? 'left' : 'right';
    ctx.lineWidth = 3;
    ctx.strokeStyle = withAlpha(p['--bg'], 0.9 * alpha);
    ctx.strokeText(g.name, sx, sy);
    ctx.fillStyle = withAlpha(p['--text'], alpha);
    ctx.fillText(g.name, sx, sy);
  }
  // At the container level the band labels already name every dot; labelling
  // hubs on top of that would just duplicate them.
  ctx.textAlign = 'center';
  for (const n of layout.nodes.length > 40 ? topHubs(layout, 10) : []) {
    const sx = n.x * env.scale + env.tx;
    const sy = n.y * env.scale + env.ty;
    if (sx < -60 || sy < -20 || sx > env.width + 60 || sy > env.height + 20) continue;
    ctx.font = '600 11px system-ui, sans-serif';
    const text = ellipsize(ctx, n.name, 150);
    ctx.lineWidth = 3;
    ctx.strokeStyle = withAlpha(p['--bg'], 0.9 * alpha);
    ctx.strokeText(text, sx, sy - 9);
    ctx.fillStyle = withAlpha(p['--text'], alpha);
    ctx.fillText(text, sx, sy - 9);
  }
  ctx.restore();
}

function onHover(id) {
  hoverId.value = id;
  emit('hover', id);
}

function draw(ctx, env) {
  const dpr = ctx.getTransform().a;
  const e = { ...env, dpr };
  ctx.save();
  ctx.translate(env.tx, env.ty);
  ctx.scale(env.scale, env.scale);
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  const t = env.blend.next ? env.blend.t : 0;
  drawLayout(ctx, e, env.base, 1 - t, 1 - 0.3 * t);
  if (env.next) drawLayout(ctx, e, env.next, t, 0.7 + 0.3 * t);
  ctx.restore();
}

function pick(wx, wy, layout, scale) {
  const reach = Math.max(4 / scale, 3);
  let best = null;
  let bestD = Infinity;
  for (const n of layout.nodes) {
    const d = Math.hypot(wx - n.x, wy - n.y);
    if (d < Math.max(n.r, reach) && d < bestD) {
      bestD = d;
      best = n.id;
    }
  }
  return best;
}
</script>

<template>
  <ArchCanvas
    :view="view"
    :draw="draw"
    :pick="pick"
    @hover="onHover"
    @select="(id) => emit('select', id)"
  />
</template>

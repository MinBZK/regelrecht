<script setup>
/**
 * MapView — prototype "Map": auto-laid-out blocks and lines.
 *
 * The geometry comes from `layoutMap` (dagre, left→right ranks); this component
 * only paints it and handles hover. Reading guide for the evaluation:
 *
 *  - box size = number of rolled-up relations, so hubs are literally the
 *    biggest blocks;
 *  - horizontal position = rank in the dependency order, so an arrow pointing
 *    left is a back-edge (a cycle or a layer violation);
 *  - the tinted hulls are the containers, drawn *after* the layout — a hull that
 *    is torn apart is a subsystem the code does not agree with.
 */
import { ref, shallowRef } from 'vue';
import ArchCanvas from './ArchCanvas.vue';
import { containerColorFactory, edgeColor, kindColor, palette, withAlpha } from '../render/palette.js';
import { ellipsize, incidence, relationWidth, topHubs, visibleRect } from '../render/common.js';

const props = defineProps({
  /** A `usePrototypeView(model, layoutMap, kinds)` handle, owned by the parent. */
  view: { type: Object, required: true },
});
const emit = defineEmits(['select', 'hover']);

/** How many blocks may print their own name at once. See the label pass. */
const IN_BOX_LABEL_BUDGET = 30;

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

function drawLayout(ctx, env, layout, alpha, grow) {
  if (!layout || alpha <= 0.01) return;
  const p = palette();
  const rect = visibleRect(env);
  const focus = focusSetsFor(layout);

  // 1. Container hulls, behind everything.
  ctx.lineWidth = 1.5 / env.scale;
  for (const g of layout.groups) {
    if (g.maxX < rect.minX || g.minX > rect.maxX || g.maxY < rect.minY || g.minY > rect.maxY) continue;
    const c = containerColor(layout, g.id);
    ctx.fillStyle = withAlpha(c, 0.07 * alpha);
    ctx.strokeStyle = withAlpha(c, 0.35 * alpha);
    const pad = 6 / env.scale;
    ctx.beginPath();
    ctx.rect(g.minX - pad, g.minY - pad, g.maxX - g.minX + pad * 2, g.maxY - g.minY + pad * 2);
    ctx.fill();
    ctx.stroke();
  }

  // 2. Relations.
  for (let i = 0; i < layout.edges.length; i += 1) {
    const e = layout.edges[i];
    const dim = focus ? (focus.edges.has(i) ? 1 : 0.1) : 1;
    if (dim < 0.15 && alpha < 0.6) continue;
    const pts = e.points;
    let visible = false;
    for (const pt of pts) {
      if (pt.x >= rect.minX && pt.x <= rect.maxX && pt.y >= rect.minY && pt.y <= rect.maxY) {
        visible = true;
        break;
      }
    }
    if (!visible) continue;
    ctx.strokeStyle = withAlpha(edgeColor(e.kind), 0.42 * alpha * dim);
    ctx.lineWidth = (relationWidth(e.weight, 1.1) * (dim > 0.5 ? 1.6 : 1)) / env.scale;
    ctx.beginPath();
    ctx.moveTo(pts[0].x, pts[0].y);
    for (let j = 1; j < pts.length; j += 1) ctx.lineTo(pts[j].x, pts[j].y);
    ctx.stroke();
  }

  // 3. Blocks.
  ctx.lineWidth = 1 / env.scale;
  for (const n of layout.nodes) {
    const w = n.w * grow;
    const h = n.h * grow;
    if (n.x + w < rect.minX || n.x - w > rect.maxX || n.y + h < rect.minY || n.y - h > rect.maxY) continue;
    const dim = focus && !focus.nodes.has(n.id) ? 0.18 : 1;
    const color = n.level === 'container' ? containerColor(layout, n.id) : kindColor(n.kind);
    ctx.fillStyle = withAlpha(color, (n.id === hoverId.value ? 1 : 0.85) * alpha * dim);
    ctx.strokeStyle = withAlpha(p['--surface'], 0.9 * alpha * dim);
    ctx.beginPath();
    ctx.rect(n.x - w / 2, n.y - h / 2, w, h);
    ctx.fill();
    ctx.stroke();
  }

  // Only the layer that is mostly on screen labels itself: two sets of
  // labels during a cross-fade is unreadable.
  if (alpha < 0.5) return;

  // 4. Labels: wherever they fit on screen, plus the ten biggest hubs always —
  //    "who is a hub" must be answerable without hovering (criterion 11).
  ctx.save();
  ctx.setTransform(env.dpr, 0, 0, env.dpr, 0, 0);
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  // Deeper down the ten biggest hubs are named whatever their block does: a
  // hub is precisely the node whose name has to be readable, and inside its own
  // block the name would be cut to fit. At the container level nothing needs
  // that help — everything is on screen and gets a name below.
  const hubs = layout.nodes.length > 40 ? topHubs(layout, 10) : [];
  const labelled = new Set(hubs.map((n) => n.id));
  // Blocks that are big enough to carry their own name do, but only the
  // biggest IN_BOX_LABEL_BUDGET of the ones actually on screen. Block size
  // grows with the relation count, so at the `code` level a screenful holds
  // hundreds of blocks over the size threshold and the drawing would vanish
  // under its own text.
  const onScreen = [];
  for (const n of layout.nodes) {
    if (labelled.has(n.id)) continue;
    const wpx = n.w * grow * env.scale;
    const hpx = n.h * grow * env.scale;
    if (wpx < 44 || hpx < 11) continue;
    const sx = n.x * env.scale + env.tx;
    const sy = n.y * env.scale + env.ty;
    if (sx < 0 || sy < 0 || sx > env.width || sy > env.height) continue;
    onScreen.push({ n, hpx, wpx });
  }
  onScreen.sort((a, b) => b.wpx - a.wpx);
  for (const { n, hpx } of onScreen.slice(0, IN_BOX_LABEL_BUDGET)) {
    drawLabel(ctx, env, n, alpha, p, Math.min(13, Math.max(9, hpx * 0.6)), false);
    labelled.add(n.id);
  }
  // At the container level everything fits, so nothing stays anonymous.
  const rest = layout.nodes.length <= 40 ? layout.nodes.filter((n) => !labelled.has(n.id)) : [];
  for (const n of [...hubs, ...rest]) {
    drawLabel(ctx, env, n, alpha, p, 12, true);
  }
  ctx.restore();
}

function drawLabel(ctx, env, n, alpha, p, size, halo) {
  const sx = n.x * env.scale + env.tx;
  const sy = n.y * env.scale + env.ty;
  if (sx < -80 || sy < -20 || sx > env.width + 80 || sy > env.height + 20) return;
  ctx.font = `${halo ? 600 : 500} ${size}px system-ui, sans-serif`;
  const text = ellipsize(ctx, n.name, halo ? 170 : Math.max(30, n.w * env.scale - 6));
  if (halo) {
    ctx.lineWidth = 3;
    ctx.strokeStyle = withAlpha(p['--bg'], 0.9 * alpha);
    ctx.strokeText(text, sx, sy);
  }
  ctx.fillStyle = withAlpha(halo ? p['--text'] : p['--surface'], alpha);
  ctx.fillText(text, sx, sy);
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
  drawLayout(ctx, e, env.base, 1 - t, 1 - 0.25 * t);
  if (env.next) drawLayout(ctx, e, env.next, t, 0.75 + 0.25 * t);
  ctx.restore();
}

function pick(wx, wy, layout) {
  let best = null;
  for (const n of layout.nodes) {
    if (Math.abs(wx - n.x) <= n.w / 2 && Math.abs(wy - n.y) <= n.h / 2) best = n.id;
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

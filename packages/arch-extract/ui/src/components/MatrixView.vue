<script setup>
/**
 * MatrixView — prototype "Matrix": an adjacency matrix (DSM).
 *
 * Geometry from `layoutMatrix` (containment order, refined by iterated
 * barycentre sorting). Reading guide:
 *
 *  - a cell at (row, column) means "row uses column"; colour is the relation
 *    kind, opacity the rolled-up count;
 *  - a **square block on the diagonal** is a cluster — a set of units that talk
 *    mostly to each other;
 *  - the coloured strip along the axes is the container each row belongs to. A
 *    diagonal block spanning two strip colours is a cluster the folder
 *    structure does not name.
 *
 * What it deliberately does **not** show: direction. In a DSM ordered
 * *topologically*, everything below the diagonal is a back-edge and cycles jump
 * out. This one is ordered for **clustering** instead (barycentre), and the two
 * goals conflict — on the real model 571 of 1175 relations sit below the
 * diagonal at the component level, which is noise, not 571 cycles. Reading
 * direction off this matrix would be wrong; see EVALUATIE.md.
 *
 * Only the ~1500 non-empty cells are drawn, never the n² grid, which is what
 * makes the `code` level (2422 × 2422 ≈ 5.9M cells) cost the same as the others.
 */
import { ref, shallowRef } from 'vue';
import ArchCanvas from './ArchCanvas.vue';
import { containerColorFactory, darken, edgeColor, palette, withAlpha } from '../render/palette.js';
import { ellipsize, topHubs } from '../render/common.js';
import { WORLD_SIZE } from '../lib/normalize.js';

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

function drawLayout(ctx, env, layout, alpha) {
  if (!layout || alpha <= 0.01) return;
  const p = palette();
  const cell = layout.cell;
  // The matrix square itself, not the padded bounds the view frames.
  const origin = -WORLD_SIZE / 2;
  const strip = Math.max(cell, WORLD_SIZE * 0.012);
  const hovered = hoverId.value ? layout.nodes.find((n) => n.id === hoverId.value) : null;

  // 1. Matrix background + diagonal.
  ctx.fillStyle = withAlpha(p['--surface'], 0.55 * alpha);
  ctx.fillRect(origin, origin, -origin * 2, -origin * 2);
  ctx.strokeStyle = withAlpha(p['--border'], 0.9 * alpha);
  ctx.lineWidth = Math.max(cell * 0.15, 0.4 / env.scale);
  ctx.beginPath();
  ctx.moveTo(origin, origin);
  ctx.lineTo(-origin, -origin);
  ctx.stroke();

  // 2. Container strips along both axes + their separator lines.
  for (const g of layout.groups) {
    const c = containerColor(layout, g.id);
    const y0 = origin + g.start * cell;
    const len = (g.end - g.start + 1) * cell;
    ctx.fillStyle = withAlpha(c, 0.85 * alpha);
    ctx.fillRect(origin - strip * 1.3, y0, strip, len);
    ctx.fillRect(y0, origin - strip * 1.3, len, strip);
    ctx.strokeStyle = withAlpha(c, 0.18 * alpha);
    ctx.lineWidth = Math.max(cell * 0.08, 0.3 / env.scale);
    ctx.beginPath();
    ctx.moveTo(origin, y0);
    ctx.lineTo(-origin, y0);
    ctx.moveTo(y0, origin);
    ctx.lineTo(y0, -origin);
    ctx.stroke();
  }

  // 3. Cells. Never smaller than a couple of screen pixels: a sub-pixel cell
  //    gets antialiased across two pixels at half the opacity each and the
  //    whole matrix washes out. At 823 or 2422 rows that is the normal case.
  const size = Math.max(cell, 2.2 / env.scale);
  for (const c of layout.cells) {
    const inFocus = hovered ? c.row === hovered.row || c.col === hovered.row : true;
    const a = (hovered ? (inFocus ? 1 : 0.15) : 1) * alpha;
    // A cell is often a single pixel, so it has to be solid to register at all;
    // the rolled-up count only darkens it further.
    ctx.fillStyle = withAlpha(darken(edgeColor(c.kind), 0.3), Math.min(1, a * (0.8 + Math.log2(1 + c.weight) * 0.2)));
    ctx.fillRect(origin + c.col * cell, origin + c.row * cell, size, size);
  }

  // 4. Cross-hair on the hovered row/column.
  if (hovered) {
    ctx.fillStyle = withAlpha(p['--accent'], 0.14 * alpha);
    ctx.fillRect(origin, origin + hovered.row * cell, -origin * 2, Math.max(cell, 1 / env.scale));
    ctx.fillRect(origin + hovered.row * cell, origin, Math.max(cell, 1 / env.scale), -origin * 2);
  }

  // Only the layer that is mostly on screen labels itself: two sets of
  // labels during a cross-fade is unreadable.
  if (alpha < 0.5) return;

  // 5. Labels: row names once a row is tall enough to hold one, container names
  //    on the strip otherwise, plus the hubs.
  ctx.save();
  ctx.setTransform(env.dpr, 0, 0, env.dpr, 0, 0);
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'right';
  const cellPx = cell * env.scale;
  if (cellPx >= 7) {
    for (const n of layout.nodes) {
      const sy = (origin + (n.row + 0.5) * cell) * env.scale + env.ty;
      if (sy < -10 || sy > env.height + 10) continue;
      const sx = (origin - strip * 1.5) * env.scale + env.tx - 6;
      ctx.font = `500 ${Math.min(12, cellPx * 0.85)}px system-ui, sans-serif`;
      ctx.fillStyle = withAlpha(n.id === hoverId.value ? p['--text'] : p['--text-muted'], alpha);
      ctx.fillText(ellipsize(ctx, n.name, 170), sx, sy);
    }
  } else {
    for (const g of layout.groups) {
      // A container is only one run of rows if the ordering agrees with the
      // folder structure; where it does not, it is scattered over many short
      // runs. Labelling every scrap would bury the axis, so only runs that are
      // tall enough to read get a name — and the gaps are the finding.
      if ((g.end - g.start + 1) * cell * env.scale < 18) continue;
      const sy = (origin + ((g.start + g.end) / 2 + 0.5) * cell) * env.scale + env.ty;
      if (sy < -10 || sy > env.height + 10) continue;
      const sx = (origin - strip * 1.5) * env.scale + env.tx - 6;
      ctx.font = '600 11px system-ui, sans-serif';
      ctx.fillStyle = withAlpha(p['--text'], alpha);
      ctx.fillText(ellipsize(ctx, g.name, 150), sx, sy);
    }
  }
  ctx.textAlign = 'left';
  for (const n of topHubs(layout, 8)) {
    const sy = (origin + (n.row + 0.5) * cell) * env.scale + env.ty;
    const sx = -origin * env.scale + env.tx + 8;
    if (sy < -10 || sy > env.height + 10) continue;
    ctx.font = '600 11px system-ui, sans-serif';
    const text = ellipsize(ctx, `${n.name} (${n.degree})`, 180);
    ctx.lineWidth = 3;
    ctx.strokeStyle = withAlpha(p['--bg'], 0.9 * alpha);
    ctx.strokeText(text, sx, sy);
    ctx.fillStyle = withAlpha(p['--text'], alpha);
    ctx.fillText(text, sx, sy);
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
  const t = env.blend.next ? env.blend.t : 0;
  drawLayout(ctx, e, env.base, 1 - t);
  if (env.next) drawLayout(ctx, e, env.next, t);
  ctx.restore();
}

function pick(wx, wy, layout) {
  const origin = -WORLD_SIZE / 2;
  const row = Math.floor((wy - origin) / layout.cell);
  if (row < 0 || row >= layout.order.length) return null;
  if (wx < origin - layout.cell * 4 || wx > -origin) return null;
  return layout.order[row];
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

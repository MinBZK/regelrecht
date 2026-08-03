<script setup>
/**
 * ArchCanvas — the canvas stage the three prototypes draw on.
 *
 * Why canvas and not Vue Flow / DOM: at the `code` level a prototype has to put
 * 2422 nodes and 1541 relations on screen and stay responsive while you scroll
 * (criterion 10). 2422 DOM nodes with transforms do not; a canvas redraw of the
 * same scene is a couple of milliseconds. It also makes the cross-fade between
 * two levels (criterion 8) a matter of one `globalAlpha`, instead of animating
 * two DOM trees at once.
 *
 * This component owns only the plumbing — sizing, device pixel ratio, wheel and
 * drag input, the redraw loop and hit-testing. *What* is drawn is the `draw`
 * prop; each prototype supplies its own.
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';

const props = defineProps({
  /** The `usePrototypeView()` result: transform, blend, layouts, timing. */
  view: { type: Object, required: true },
  /** `(ctx, env) => void` — paints one frame in screen coordinates. */
  draw: { type: Function, required: true },
  /** `(worldX, worldY, layout) => string|null` — id under the cursor. */
  pick: { type: Function, default: null },
});

const emit = defineEmits(['hover', 'select']);

const wrap = ref(null);
const canvas = ref(null);
const width = ref(0);
const height = ref(0);
const hoverId = ref(null);
const dragging = ref(false);

let ctx = null;
let raf = 0;
let dirty = true;
let observer = null;
let dragMoved = false;
let last = { x: 0, y: 0 };
let pointer = { x: 0, y: 0, inside: false };

function requestDraw() {
  dirty = true;
}

function resize() {
  const el = wrap.value;
  const cv = canvas.value;
  if (!el || !cv) return;
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  const w = el.clientWidth;
  const h = el.clientHeight;
  if (!w || !h) return;
  const first = width.value === 0;
  width.value = w;
  height.value = h;
  cv.width = Math.round(w * dpr);
  cv.height = Math.round(h * dpr);
  cv.style.width = `${w}px`;
  cv.style.height = `${h}px`;
  ctx = cv.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  const bounds = props.view.activeLayout.value?.bounds;
  if (bounds) {
    if (first) props.view.panzoom.fit(bounds, w, h);
    else props.view.panzoom.refit(bounds, w, h);
  }
  requestDraw();
}

function frame() {
  raf = requestAnimationFrame(frame);
  if (!dirty || !ctx) return;
  dirty = false;
  const t0 = performance.now();
  const { panzoom, blend, baseLayout, nextLayout } = props.view;
  ctx.clearRect(0, 0, width.value, height.value);
  props.draw(ctx, {
    width: width.value,
    height: height.value,
    scale: panzoom.scale.value,
    tx: panzoom.tx.value,
    ty: panzoom.ty.value,
    zoom: panzoom.zoom.value,
    blend: blend.value,
    base: baseLayout.value,
    next: nextLayout.value,
    hoverId: hoverId.value,
    pointer,
  });
  props.view.recordFrame(performance.now() - t0);
}

function onWheel(event) {
  event.preventDefault();
  const rect = canvas.value.getBoundingClientRect();
  props.view.panzoom.wheelZoom(event.clientX - rect.left, event.clientY - rect.top, event.deltaY);
  requestDraw();
}

function onPointerDown(event) {
  dragging.value = true;
  dragMoved = false;
  last = { x: event.clientX, y: event.clientY };
  canvas.value.setPointerCapture?.(event.pointerId);
}

function onPointerMove(event) {
  const rect = canvas.value.getBoundingClientRect();
  pointer = { x: event.clientX - rect.left, y: event.clientY - rect.top, inside: true };
  if (dragging.value) {
    const dx = event.clientX - last.x;
    const dy = event.clientY - last.y;
    if (Math.abs(dx) + Math.abs(dy) > 2) dragMoved = true;
    props.view.panzoom.panBy(dx, dy);
    last = { x: event.clientX, y: event.clientY };
    requestDraw();
    return;
  }
  updateHover();
}

function onPointerUp(event) {
  dragging.value = false;
  canvas.value.releasePointerCapture?.(event.pointerId);
  if (!dragMoved) emit('select', hoverId.value);
}

function onPointerLeave() {
  pointer = { ...pointer, inside: false };
  if (hoverId.value !== null) {
    hoverId.value = null;
    emit('hover', null);
    requestDraw();
  }
}

function updateHover() {
  if (!props.pick) return;
  const layout = props.view.activeLayout.value;
  if (!layout) return;
  const world = props.view.panzoom.toWorld(pointer.x, pointer.y);
  const id = props.pick(world.x, world.y, layout, props.view.panzoom.scale.value);
  if (id !== hoverId.value) {
    hoverId.value = id;
    emit('hover', id);
    requestDraw();
  }
}

/** Re-frame the whole model — the escape hatch after getting lost. */
function resetView() {
  const bounds = props.view.activeLayout.value?.bounds;
  if (bounds) props.view.panzoom.fit(bounds, width.value, height.value);
  requestDraw();
}

onMounted(() => {
  observer = new ResizeObserver(resize);
  observer.observe(wrap.value);
  resize();
  props.view.warm();
  raf = requestAnimationFrame(frame);
});

onBeforeUnmount(() => {
  cancelAnimationFrame(raf);
  observer?.disconnect();
});

// Any change to the transform, the level blend or the laid-out data repaints.
watch(
  () => [
    props.view.panzoom.scale.value,
    props.view.panzoom.tx.value,
    props.view.panzoom.ty.value,
    props.view.baseLayout.value,
    props.view.nextLayout.value,
  ],
  requestDraw,
);

defineExpose({ requestDraw, resetView });
</script>

<template>
  <div ref="wrap" class="arch-stage">
    <canvas
      ref="canvas"
      class="arch-stage__canvas"
      :class="{ 'is-dragging': dragging, 'is-hovering': hoverId }"
      @wheel="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointerleave="onPointerLeave"
      @dblclick="resetView"
    ></canvas>
    <slot :hover-id="hoverId" :pointer="pointer"></slot>
  </div>
</template>

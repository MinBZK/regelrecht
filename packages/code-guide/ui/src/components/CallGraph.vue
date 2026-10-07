<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { bestWrap } from '../lib/callgraph.js';

// The canvas: a Cytoscape instance drawing `elements` with `layout`. It knows
// nothing about the guide; the pane around it decides what is highlighted and
// what a click means.
//
// - hover a node: its callers and callees light up, the rest fades (unless a
//   highlight is shown, which wins);
// - tap a node: `select` (with `additive` for Shift); tap the background:
//   `clear`; double-tap: `open`;
// - drag a node to move it; the wheel zooms, dragging the background pans.
const props = defineProps({
  elements: { type: Array, required: true },
  layout: { type: Object, required: true },
  /** `{ nodes: Set, edges: Set }` or null. */
  highlight: { type: Object, default: null },
  hideRest: { type: Boolean, default: false },
  selected: { type: Array, default: () => [] },
  /** A node to keep in view after the next layout (one just opened up). */
  centre: { type: String, default: null },
});
const emit = defineEmits(['select', 'clear', 'open', 'hover']);

const container = ref(null);
const failed = ref(null);
let cy = null;
let theme = null;
let scheme = null;
let lib = null;
let resizer = null;
let themeObserver = null;

// Cytoscape caches where its canvas is on the page, and drops that cache when
// an element above it scrolls. It finds those elements by walking up
// `parentNode`, which stops at a shadow root, so it never sees the design
// system's page scrolling. Without this every click after a scroll lands
// beside the node it was aimed at. Dropping the cache before Cytoscape handles
// a pointer event costs one `getBoundingClientRect`.
const POINTER_EVENTS = ['pointerdown', 'pointermove', 'pointerover', 'wheel', 'touchstart'];
function dropPositionCache() {
  cy?.renderer()?.invalidateContainerClientCoordsCache?.();
}
function resized() {
  dropPositionCache();
  cy?.resize();
}

/** Closer than this, a small graph's boxes and labels look blown up. */
const MAX_FIT_ZOOM = 1.2;

/** Everything in view, but a small graph no larger than `MAX_FIT_ZOOM`. */
function fitAll() {
  if (!cy) return;
  const visible = cy.elements().not('.hidden');
  cy.fit(visible, 24);
  if (cy.zoom() > MAX_FIT_ZOOM) {
    cy.zoom(MAX_FIT_ZOOM);
    cy.center(visible);
  }
}

// A function just added to the graph (`centre` changed) is shown where the
// reader was looking, at the same zoom; every other new graph is shown whole.
let shownCentre = null;
function placeView() {
  if (!cy) return;
  const added = props.centre && props.centre !== shownCentre ? cy.getElementById(props.centre) : null;
  shownCentre = props.centre;
  if (added?.length) cy.center(added);
  else fitAll();
}

/**
 * Wraps ranks of a layered layout that are long, so the graph takes the shape
 * of the canvas instead of one long line; see `wrapRanks`. Of every possible
 * row length it keeps the one at which the whole graph fits largest.
 */
function wrapLong() {
  if (!cy || props.layout.name !== 'dagre') return;
  const leaves = cy.nodes().not('.hidden').not(':parent');
  if (!leaves.length) return;
  const axis = props.layout.rankDir === 'LR' ? 'x' : 'y';
  const gap = axis === 'y' ? 28 : 14;
  const items = leaves.map((n) => ({ id: n.id(), ...n.position(), w: n.width(), h: n.height(), parent: n.data('parent') }));
  const best = bestWrap(items, { axis, gap, width: cy.width(), height: cy.height() });
  cy.batch(() => leaves.forEach((n) => n.position(best.get(n.id()))));
}

function runLayout() {
  if (!cy) return;
  const visible = cy.elements().not('.hidden');
  const layout = visible.layout({ ...props.layout, fit: false });
  layout.one('layoutstop', () => {
    wrapLong();
    placeView();
  });
  layout.run();
}

function applyClasses() {
  if (!cy) return;
  const hl = props.highlight;
  cy.batch(() => {
    cy.elements().removeClass('faded hidden hl in out sel');
    if (hl) {
      const off = props.hideRest ? 'hidden' : 'faded';
      cy.nodes().forEach((n) => n.addClass(hl.nodes.has(n.id()) ? 'hl' : off));
      cy.edges().forEach((e) => e.addClass(hl.edges.has(e.id()) ? 'hl' : off));
    }
    // A box shows its functions: it must not fade or hide them when one of
    // them is highlighted.
    cy.nodes(':parent')
      .filter((p) => p.children('.hl').length > 0)
      .removeClass('faded hidden')
      .addClass('hl');
    for (const id of props.selected) cy.getElementById(id).addClass('sel');
  });
}

function hover(node) {
  if (!cy || props.highlight) return;
  cy.batch(() => {
    cy.elements().removeClass('faded in out');
    if (!node) return;
    const near = node.closedNeighborhood();
    cy.elements().not(near).addClass('faded');
    node.incomers('edge').addClass('in');
    node.outgoers('edge').addClass('out');
  });
}

// Cytoscape reports leaving a node only for a move within the canvas, so a
// pointer that leaves the canvas from a node would keep it hovered; and a
// rebuild removes the hovered node without a word.
function unhover() {
  hover(null);
  emit('hover', null);
}

function rebuild() {
  if (!cy) return;
  unhover();
  cy.batch(() => {
    cy.elements().remove();
    cy.add(props.elements);
  });
  applyClasses();
  runLayout();
}

function restyle() {
  if (!cy || !lib) return;
  theme = lib.resolveTheme(container.value);
  cy.style(lib.styleSheet(theme));
}

onMounted(async () => {
  try {
    const [cyMod, themeMod] = await Promise.all([import('../lib/cy.js'), import('../lib/theme.js')]);
    lib = { ...cyMod, ...themeMod };
    if (!container.value) return;
    theme = lib.resolveTheme(container.value);
    cy = lib.createCy({ container: container.value, elements: props.elements, theme });
    // The nodes are pixels on a canvas, so a browser test finds them through
    // the instance; nothing in the app reads this.
    container.value.cy = cy;
    applyClasses();
    runLayout();
    cy.on('tap', 'node', (e) => emit('select', e.target.id(), !!e.originalEvent?.shiftKey));
    cy.on('tap', (e) => {
      if (e.target === cy) emit('clear');
    });
    cy.on('dbltap', 'node', (e) => emit('open', e.target.id()));
    cy.on('mouseover', 'node', (e) => {
      hover(e.target);
      emit('hover', e.target.id());
    });
    cy.on('mouseout', 'node', () => {
      hover(null);
      emit('hover', null);
    });
    scheme = globalThis.matchMedia?.('(prefers-color-scheme: dark)');
    scheme?.addEventListener?.('change', restyle);
    // A theme set on the page itself (an attribute on <html>) changes the
    // tokens without changing the system's colour scheme.
    if (globalThis.MutationObserver) {
      themeObserver = new MutationObserver(restyle);
      themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    }
    for (const type of POINTER_EVENTS) {
      container.value.addEventListener(type, dropPositionCache, { capture: true, passive: true });
    }
    container.value.addEventListener('pointerleave', unhover);
    if (globalThis.ResizeObserver) {
      resizer = new ResizeObserver(resized);
      resizer.observe(container.value);
    }
  } catch (e) {
    failed.value = e.message;
  }
});
onBeforeUnmount(() => {
  scheme?.removeEventListener?.('change', restyle);
  for (const type of POINTER_EVENTS) container.value?.removeEventListener(type, dropPositionCache, { capture: true });
  container.value?.removeEventListener('pointerleave', unhover);
  resizer?.disconnect();
  themeObserver?.disconnect();
  cy?.destroy();
  cy = null;
});

watch(() => props.elements, rebuild);
// The options object is rebuilt on every navigation; only a different layout
// or spacing is a reason to lay out again (new elements are laid out anyway).
watch(() => props.layout.id, runLayout);
watch([() => props.highlight, () => props.hideRest, () => props.selected], () => {
  applyClasses();
  // Hiding changes what is laid out; showing again keeps the positions.
});

defineExpose({
  /** Everything in view. */
  fit: fitAll,
  relayout: runLayout,
  zoom: (factor) =>
    cy?.zoom({ level: cy.zoom() * factor, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } }),
  /** Centres the view on a node without changing the zoom. */
  centre: (id) => cy?.center(cy.getElementById(id)),
});
</script>

<template>
  <nldd-banner v-if="failed" variant="critical" text="The graph could not be drawn" :supporting-text="failed" />
  <!-- Cytoscape needs a plain element with a definite size to draw into. -->
  <div ref="container" class="call-graph-canvas" role="img" aria-label="Call graph; the same calls are listed under the graph and on the Details page"></div>
</template>

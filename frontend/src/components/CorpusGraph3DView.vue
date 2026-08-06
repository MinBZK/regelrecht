<script setup>
/**
 * The 3D corpus graph.
 *
 * This component owns the DOM and the interaction; everything about the
 * rendering itself lives in `src/graph3d/`. The canvas is the one thing the
 * design system has no component for, so it is a bare <canvas> with NDD
 * controls floating above it - and nothing else here is hand-rolled.
 *
 * The graph arrives packed (typed arrays, see graph3d/graphSchema.js), either
 * from a `.rrgraph` payload (`src`), handed in ready-made (`graph`), or - when
 * neither is there - synthesised at corpus scale by `generateCorpusGraph`.
 *
 * The colour rule drives the interaction here: grey is a law that has only been
 * harvested. You can point at it and read its name; the detail panel is for
 * what has been enriched, because that is where there is something to show. The
 * status can be updated afterwards (`applyStatusUpdates`) without rebuilding
 * anything, which is the seam a live view of a running enricher hangs on.
 */
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
import './graph/graph3d-styles.css';
import { GraphScene } from '../graph3d/GraphScene.js';
import { generateCorpusGraph } from '../graph3d/generateCorpusGraph.js';
import { readPalette, releasePaletteProbe } from '../graph3d/palette.js';
import { KIND_NAMES, STATUS_NAMES, isEnriched } from '../graph3d/graphSchema.js';
import { loadRrgraph } from '../graph3d/rrgraph.js';

const props = defineProps({
  /** URL of a `.rrgraph` payload. Takes precedence over `graph`. */
  src: { type: String, default: null },
  /** Load only the law-level block of the payload, not the article level. */
  lawLevelOnly: { type: Boolean, default: true },
  /** Packed graph. When absent, a synthetic corpus of `nodes`/`edges` is used. */
  graph: { type: Object, default: null },
  nodes: { type: Number, default: 4138 },
  edges: { type: Number, default: 50000 },
  /** Label budget; above a few thousand the frame budget goes into text. */
  labelBudget: { type: Number, default: 400 },
});

const emit = defineEmits(['select']);

const stageEl = ref(null);
const canvasEl = ref(null);
const scene = shallowRef(null);
const ready = ref(false);
const error = ref(null);

const viewMode = ref('gewicht'); // 'structuur' | 'gewicht'
const showLabels = ref(true);
const selection = ref(null);
const fps = ref(0);
const nodeCount = ref(0);
const edgeCount = ref(0);
const labelsUnavailable = ref(false);
const loading = ref(false);
const enrichedCount = ref(0);

let resizeObserver = null;
let statsTimer = null;
let hoverRaf = 0;
let pendingHover = null;
let dragging = false;
let pressPoint = null;

const reducedMotion =
  typeof matchMedia === 'function'
    ? matchMedia('(prefers-reduced-motion: reduce)').matches
    : false;

const selectionSummary = computed(() => {
  const s = selection.value;
  if (!s) return null;
  return {
    label: s.label,
    kind: KIND_NAMES[s.kind] ?? 'law',
    status: STATUS_NAMES[s.status] ?? 'harvested',
    degree: s.degree,
    truncated: s.truncated,
    enriched: s.enriched,
  };
});

async function packedGraph() {
  if (props.src) {
    return loadRrgraph(props.src, { lawLevelOnly: props.lawLevelOnly });
  }
  if (props.graph) return props.graph;
  return generateCorpusGraph({
    nodeCount: props.nodes,
    edgeCount: props.edges,
    seed: 7,
  });
}

function describeNode(scn, index, highlight) {
  const g = scn.graph;
  return {
    index,
    id: g.ids ? g.ids[index] : String(index),
    label: g.labels ? g.labels[index] : String(index),
    kind: g.kind[index],
    status: g.status[index],
    weight: g.weight[index],
    degree: highlight?.degree ?? 0,
    truncated: highlight?.truncated ?? false,
    // Grey means harvested only: the host gets the name, and knows there is no
    // article level or marking panel to open behind it.
    enriched: isEnriched(g.status[index]),
  };
}

async function build() {
  if (!canvasEl.value) return;
  loading.value = true;
  try {
    const graph = await packedGraph();
    if (!canvasEl.value) return; // unmounted while the payload was in flight
    const scn = new GraphScene(canvasEl.value, graph, {
      palette: readPalette(),
      labelBudget: props.labelBudget,
      showLabels: showLabels.value,
      reducedMotion,
      weightMode: viewMode.value === 'gewicht',
    });
    scene.value = scn;
    nodeCount.value = graph.nodeCount;
    edgeCount.value = graph.edgeCount;
    labelsUnavailable.value = scn.labelsUnavailable;
    let enriched = 0;
    for (let i = 0; i < graph.nodeCount; i++) if (isEnriched(graph.status[i])) enriched++;
    enrichedCount.value = enriched;
    scn.start();
    ready.value = true;
    statsTimer = setInterval(() => {
      const s = scn.stats.summary();
      fps.value = Math.round(s.fps);
    }, 500);
  } catch (err) {
    error.value = err?.message ?? String(err);
  } finally {
    loading.value = false;
  }
}

/**
 * Push enrichment-status updates into the running scene.
 *
 * Exposed rather than watched, because the source of these updates does not
 * exist yet: when it does (a poll of the enrichment progress, or a stream), it
 * calls this and one node changes colour. Nothing is re-fetched and nothing
 * moves.
 */
function applyStatusUpdates(updates) {
  return scene.value?.applyStatusUpdates(updates) ?? 0;
}

function applyStatusUpdatesById(updates) {
  return scene.value?.applyStatusUpdatesById(updates) ?? 0;
}

function canvasPoint(event) {
  const rect = canvasEl.value.getBoundingClientRect();
  return { x: event.clientX - rect.left, y: event.clientY - rect.top };
}

function onPointerDown(event) {
  dragging = true;
  pressPoint = canvasPoint(event);
}

function onPointerUp() {
  dragging = false;
}

function onPointerMove(event) {
  if (!scene.value) return;
  // No picking while the camera is being dragged: the id pass is a second full
  // render of every node, and orbiting is exactly when the frame budget is
  // already tight.
  if (dragging) return;
  pendingHover = canvasPoint(event);
  // One pick per animation frame at most: the id pass is cheap but it is a
  // full render, and a mouse can fire far more move events than frames.
  if (hoverRaf) return;
  hoverRaf = requestAnimationFrame(() => {
    hoverRaf = 0;
    const p = pendingHover;
    if (!p || !scene.value) return;
    const idx = scene.value.pickAt(p.x, p.y);
    scene.value.hover(idx);
  });
}

function onPointerLeave() {
  if (scene.value) scene.value.hover(-1);
}

function onClick(event) {
  if (!scene.value) return;
  const p = canvasPoint(event);
  // OrbitControls does not swallow the click at the end of a drag, so without
  // this every rotation would end in a selection or a deselection.
  if (pressPoint && Math.hypot(p.x - pressPoint.x, p.y - pressPoint.y) > 4) return;
  const idx = scene.value.pickAt(p.x, p.y);
  if (idx < 0) {
    selection.value = null;
    scene.value.select(-1);
    emit('select', null);
    return;
  }
  const highlight = scene.value.select(idx);
  const node = describeNode(scene.value, idx, highlight);
  selection.value = node;
  emit('select', node);
}

function onDoubleClick(event) {
  if (!scene.value) return;
  const p = canvasPoint(event);
  const idx = scene.value.pickAt(p.x, p.y);
  if (idx < 0) scene.value.fitAll();
  else scene.value.focusNode(idx);
}

function fitAll() {
  scene.value?.fitAll();
}

function onViewMode(event) {
  const value = event?.detail?.value ?? event?.target?.value ?? viewMode.value;
  viewMode.value = value;
  scene.value?.setWeightMode(value === 'gewicht');
}

function onToggleLabels(event) {
  const checked = event?.detail?.checked ?? event?.target?.checked ?? !showLabels.value;
  showLabels.value = checked;
  // Not just hiding the mesh: with labels off the LOD pass must stop too,
  // otherwise it keeps rewriting glyph buffers nobody sees.
  scene.value?.setLabelsEnabled(checked);
}

function onContextLost(event) {
  event.preventDefault();
  error.value = 'de grafische context ging verloren (driver-reset of te veel canvassen)';
}

function teardown() {
  if (statsTimer) clearInterval(statsTimer);
  statsTimer = null;
  if (hoverRaf) cancelAnimationFrame(hoverRaf);
  hoverRaf = 0;
  scene.value?.dispose();
  scene.value = null;
  ready.value = false;
}

onMounted(() => {
  canvasEl.value?.addEventListener('webglcontextlost', onContextLost);
  build();
  if (typeof ResizeObserver === 'function' && stageEl.value) {
    resizeObserver = new ResizeObserver(() => scene.value?.resize());
    resizeObserver.observe(stageEl.value);
  }
});

onBeforeUnmount(() => {
  canvasEl.value?.removeEventListener('webglcontextlost', onContextLost);
  resizeObserver?.disconnect();
  teardown();
  releasePaletteProbe();
});

// A different graph is a different scene: tear the old one down completely
// (timer, animation frame, WebGL context) before building the new one.
watch(
  () => [props.src, props.graph, props.nodes, props.edges],
  () => {
    teardown();
    error.value = null;
    labelsUnavailable.value = false;
    selection.value = null;
    build();
  },
);

defineExpose({ scene, fitAll, applyStatusUpdates, applyStatusUpdatesById });
</script>

<template>
  <div class="graph3d-stage" ref="stageEl">
    <canvas
      ref="canvasEl"
      tabindex="0"
      aria-label="Corpusgraaf, driedimensionaal. Gebruik de lijstweergave voor een toegankelijk alternatief."
      @pointerdown="onPointerDown"
      @pointerup="onPointerUp"
      @pointermove="onPointerMove"
      @pointerleave="onPointerLeave"
      @click="onClick"
      @dblclick="onDoubleClick"
    ></canvas>

    <nldd-activity-indicator
      v-if="loading"
      class="graph3d-loading"
      label="Corpusgraaf laden"
    ></nldd-activity-indicator>

    <div class="graph3d-hud">
      <nldd-segmented-control
        size="sm"
        width="fit-content"
        :value="viewMode"
        @change="onViewMode"
      >
        <nldd-segmented-control-item value="structuur" text="Structuur" />
        <nldd-segmented-control-item value="gewicht" text="Gewicht" />
      </nldd-segmented-control>

      <nldd-switch-field
        label="Labels"
        size="sm"
        :checked="showLabels || null"
        @change="onToggleLabels"
      ></nldd-switch-field>

      <nldd-button size="sm" variant="secondary" text="Alles in beeld" @click="fitAll"></nldd-button>

      <nldd-tag
        v-if="labelsUnavailable"
        color="warning"
        size="sm"
        text="Labels niet beschikbaar"
      ></nldd-tag>
    </div>

    <div class="graph3d-readout">
      <nldd-tag
        color="neutral"
        size="sm"
        :text="`${nodeCount} wetten · ${edgeCount} verwijzingen · ${enrichedCount} verrijkt · ${fps} fps`"
      ></nldd-tag>
      <nldd-tag
        v-if="selectionSummary"
        :color="selectionSummary.enriched ? 'info' : 'neutral'"
        size="sm"
        :text="
          selectionSummary.enriched
            ? `${selectionSummary.label} · ${selectionSummary.kind} · ${selectionSummary.status} · graad ${selectionSummary.degree}`
            : `${selectionSummary.label} · alleen geoogst, nog niet verrijkt`
        "
      ></nldd-tag>
    </div>

    <nldd-banner
      v-if="error"
      variant="error"
      :text="`De graaf kon niet worden geladen: ${error}`"
    ></nldd-banner>
  </div>
</template>

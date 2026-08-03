<script setup>
/**
 * ArchExplorer — the shell: toolbar, view switcher, detail panel.
 *
 * The explorer is currently a **comparison rig**. Three candidate schema
 * techniques for the overview problem — Map, Radiaal, Matrix — sit next to the
 * original click-driven view, so the choice between them can be made on what
 * they actually look like at full scale. See `EVALUATIE.md` next to this
 * directory. The rig is temporary: the follow-up ticket keeps one and removes
 * the rest.
 *
 * The three prototypes share everything except where they put things: the same
 * model, the same edge filters, the same rollup (`lib/archRollup.js`), the same
 * world box and the same zoom → level mapping (`useSemanticZoom.js`). Their
 * per-prototype state lives in `usePrototypeView()` handles owned here, so
 * switching views keeps each prototype's own pan/zoom and its cached layouts.
 */
import { computed, ref, watch } from 'vue';

import CurrentFlowView from './CurrentFlowView.vue';
import MapView from './MapView.vue';
import RadialView from './RadialView.vue';
import MatrixView from './MatrixView.vue';
import DetailPanel from './DetailPanel.vue';

import { useColorScheme } from '../composables/useColorScheme.js';
import { useEdgeFilters } from '../composables/useEdgeFilters.js';
import { usePrototypeView } from '../composables/usePrototypeView.js';
import { useViewMode } from '../composables/useViewMode.js';
import { LEVEL_ZOOM_THRESHOLDS } from '../composables/useSemanticZoom.js';
import { layoutMap } from '../layouts/mapLayout.js';
import { layoutRadial } from '../layouts/radialLayout.js';
import { layoutMatrix } from '../layouts/matrixLayout.js';
import { invalidatePalette } from '../render/palette.js';

const props = defineProps({
  model: { type: Object, required: true },
  prose: { type: Object, default: () => ({}) },
});

const { viewMode, setViewMode, VIEW_MODES } = useViewMode();
const { enabledKinds, toggleKind, kindEnabled, FILTERABLE_KINDS } = useEdgeFilters();

const modelRef = computed(() => props.model);
const kindsRef = computed(() => enabledKinds.value);

// One handle per prototype. Creating all three up front costs only the coarse
// container layout (a few ms); the deeper levels are warmed by whichever
// prototype is actually mounted.
const views = {
  map: usePrototypeView(modelRef, layoutMap, kindsRef),
  radial: usePrototypeView(modelRef, layoutRadial, kindsRef),
  matrix: usePrototypeView(modelRef, layoutMatrix, kindsRef),
};
const activeView = computed(() => views[viewMode.value] || null);
const isPrototype = computed(() => Boolean(activeView.value));

// --- Selection / detail panel ---------------------------------------------
const selectedId = ref(null);
const hoveredId = ref(null);
const selectedNode = computed(() =>
  selectedId.value ? props.model.nodes.find((n) => n.id === selectedId.value) || null : null,
);
const selectedProse = computed(() =>
  selectedId.value ? props.prose?.[selectedId.value] || null : null,
);
const hoveredNode = computed(() =>
  hoveredId.value ? props.model.nodes.find((n) => n.id === hoveredId.value) || null : null,
);
const hoveredDegree = computed(() => {
  const v = activeView.value;
  if (!v || !hoveredId.value) return null;
  const { next, t } = v.blend.value;
  const layout = next && t >= 0.5 ? v.nextLayout.value : v.baseLayout.value;
  return layout?.nodes.find((n) => n.id === hoveredId.value)?.degree ?? null;
});

// The current view keeps reporting its own numbers, since its rollup depends on
// what happens to be expanded rather than on a level.
const flowStats = ref({ visible: 0, total: 0 });

// --- Toolbar read-outs -----------------------------------------------------
const nodeCount = computed(() => props.model?.nodes.length ?? 0);
const edgeCount = computed(() => props.model?.edges.length ?? 0);

const LEVEL_LABEL = { container: 'container', component: 'component', code: 'code' };

const levelReadout = computed(() => {
  const v = activeView.value;
  if (!v) return null;
  const { base, next, t } = v.blend.value;
  const zoom = v.panzoom.zoom.value;
  const pct = Math.round(t * 100);
  return {
    text: next && pct > 0 ? `${LEVEL_LABEL[base]} → ${LEVEL_LABEL[next]} ${pct}%` : LEVEL_LABEL[base],
    zoom: zoom >= 10 ? zoom.toFixed(0) : zoom.toFixed(1),
  };
});

const coverage = computed(() => {
  const s = activeView.value?.stats.value;
  if (!s) return null;
  return s;
});

const perf = computed(() => {
  const v = activeView.value;
  if (!v) return null;
  return {
    layoutMs: v.timings.value[v.level.value] ?? null,
    frameMs: v.frameMs.value,
  };
});

// --- Theme ----------------------------------------------------------------
const { colorScheme, cycleColorScheme } = useColorScheme();
const themeLabel = computed(
  () => ({ auto: 'Auto', light: 'Licht', dark: 'Donker' })[colorScheme.value] || 'Auto',
);
// The canvas resolves the CSS custom properties itself, so it has to be told
// when they change.
watch(colorScheme, () => {
  requestAnimationFrame(() => invalidatePalette());
});

// Which CSS legend swatch class an edge kind maps to.
const KIND_SWATCH = { 'depends-on': 'lg-depends', impl: 'lg-impl', uses: 'lg-uses' };

const zoomHint = `Scrollen wisselt het detailniveau: component vanaf ${LEVEL_ZOOM_THRESHOLDS.component}×, code vanaf ${LEVEL_ZOOM_THRESHOLDS.code}× ingezoomd. Dubbelklik = terug naar het geheel.`;
</script>

<template>
  <div class="arch-explorer">
    <header class="arch-toolbar">
      <strong class="arch-toolbar__title">Architectuurverkenner</strong>

      <div class="arch-modes" role="group" aria-label="Weergave kiezen">
        <button
          v-for="m in VIEW_MODES"
          :key="m.id"
          type="button"
          class="arch-mode"
          :class="{ 'arch-mode--on': viewMode === m.id }"
          :aria-pressed="viewMode === m.id"
          :title="m.hint"
          @click="setViewMode(m.id)"
        >
          {{ m.label }}
        </button>
      </div>

      <span v-if="isPrototype" class="arch-toolbar__stat" :title="zoomHint">
        {{ nodeCount }} nodes / {{ edgeCount }} relaties · niveau
        <strong>{{ levelReadout.text }}</strong> ({{ levelReadout.zoom }}× zoom)
      </span>
      <span v-else class="arch-toolbar__stat">
        {{ nodeCount }} nodes · {{ flowStats.visible }}/{{ flowStats.total }} relaties zichtbaar
      </span>

      <span v-if="isPrototype && coverage" class="arch-toolbar__stat" title="Zichtbaar op dit niveau: eigen knopen, en relaties als lijn of als interne teller op een knoop.">
        {{ coverage.units }} knopen · {{ coverage.visible }}/{{ coverage.total }} relaties
        <template v-if="coverage.internal">({{ coverage.internal }} intern)</template>
      </span>

      <div class="arch-toolbar__spacer"></div>

      <span v-if="perf" class="arch-toolbar__perf" title="Rekentijd van de layout voor dit niveau, en de mediane tekentijd per frame.">
        layout {{ perf.layoutMs ?? '…' }} ms · frame {{ perf.frameMs }} ms
      </span>

      <div class="arch-filters" role="group" aria-label="Relatiesoorten filteren">
        <button
          v-for="kind in FILTERABLE_KINDS"
          :key="kind"
          type="button"
          class="arch-filter"
          :class="{ 'arch-filter--off': !kindEnabled(kind) }"
          :aria-pressed="kindEnabled(kind)"
          :title="kindEnabled(kind) ? `${kind} verbergen` : `${kind} tonen`"
          @click="toggleKind(kind)"
        >
          <i class="lg" :class="KIND_SWATCH[kind]"></i>{{ kind }}
        </button>
      </div>

      <button type="button" class="arch-btn" @click="cycleColorScheme" :title="`Thema: ${themeLabel}`">
        Thema: {{ themeLabel }}
      </button>
    </header>

    <div class="arch-canvas">
      <MapView
        v-if="viewMode === 'map'"
        :view="views.map"
        @select="(id) => (selectedId = id)"
        @hover="(id) => (hoveredId = id)"
      />
      <RadialView
        v-else-if="viewMode === 'radial'"
        :view="views.radial"
        @select="(id) => (selectedId = id)"
        @hover="(id) => (hoveredId = id)"
      />
      <MatrixView
        v-else-if="viewMode === 'matrix'"
        :view="views.matrix"
        @select="(id) => (selectedId = id)"
        @hover="(id) => (hoveredId = id)"
      />
      <CurrentFlowView
        v-else
        :model="model"
        @select="(id) => (selectedId = id)"
        @stats="(s) => (flowStats = s)"
      />

      <p v-if="isPrototype" class="arch-hint">{{ zoomHint }}</p>
      <p v-if="isPrototype && hoveredNode" class="arch-hovercard">
        <span class="arch-hovercard__kind">{{ hoveredNode.kind }}</span>
        <strong>{{ hoveredNode.name }}</strong>
        <span v-if="hoveredDegree !== null" class="arch-hovercard__deg">{{ hoveredDegree }} verbindingen</span>
      </p>

      <DetailPanel :node="selectedNode" :prose="selectedProse" @close="selectedId = null" />
    </div>
  </div>
</template>

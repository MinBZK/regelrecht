<template>
  <nldd-app-view>
    <nldd-page sticky-header>
      <!-- Sticky header: branding + playback/zoom/view controls -->
      <nldd-container slot="header" padding="8">
        <nldd-toolbar size="md">
          <nldd-toolbar-item slot="start">
            <img
              src="/assets/rijkswapen.svg"
              alt="Rijkswapen"
              class="app-logo"
            />
          </nldd-toolbar-item>
          <nldd-toolbar-title
            slot="start"
            text="Wetgevingsproces"
            supporting-text="Van wetsvoorstel tot geldend recht — het wetgevingsproces als GitFlow"
          ></nldd-toolbar-title>

          <!-- Playback: step navigation -->
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="chevron-double-left"
              text="Naar begin"
              :disabled="activeStep <= 0 || undefined"
              @click="resetSteps"
            ></nldd-icon-button>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="chevron-left"
              text="Stap terug"
              :disabled="activeStep <= 0 || undefined"
              @click="stepBack"
            ></nldd-icon-button>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="chevron-right"
              text="Stap vooruit"
              :disabled="activeStep >= maxStep || undefined"
              @click="stepForward"
            ></nldd-icon-button>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="chevron-double-right"
              text="Naar einde"
              :disabled="activeStep >= maxStep || undefined"
              @click="goToEnd"
            ></nldd-icon-button>
          </nldd-toolbar-item>

          <!-- Playback: play/pause. The design system has no media-transport
               glyph, so this is a labelled button; `isPlaying` is the single
               source of truth (a plain button keeps no internal toggle state),
               and the filled `primary` variant signals the playing state. -->
          <nldd-toolbar-item slot="end">
            <nldd-button
              :text="isPlaying ? 'Pauzeren' : 'Afspelen'"
              :variant="isPlaying ? 'primary' : 'secondary'"
              @click="togglePlay"
            ></nldd-button>
          </nldd-toolbar-item>

          <nldd-toolbar-item slot="end">
            <nldd-tag :text="`${activeStep + 1} / ${maxStep + 1}`"></nldd-tag>
          </nldd-toolbar-item>

          <!-- Zoom -->
          <nldd-toolbar-item slot="end">
            <nldd-icon-button icon="plus" text="Zoom in" @click="diagramRef?.zoomIn()"></nldd-icon-button>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button icon="minus" text="Zoom uit" @click="diagramRef?.zoomOut()"></nldd-icon-button>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button icon="arrow-2-counter-clockwise" text="Centreren" @click="diagramRef?.resetView()"></nldd-icon-button>
          </nldd-toolbar-item>

          <!-- View mode -->
          <nldd-toolbar-item slot="end">
            <nldd-segmented-control
              size="md"
              width="fit-content"
              :value="viewMode"
              @change="onViewModeChange"
            >
              <nldd-segmented-control-item value="simple" text="Eenvoudig"></nldd-segmented-control-item>
              <nldd-segmented-control-item value="advanced" text="Uitgebreid"></nldd-segmented-control-item>
              <nldd-segmented-control-item value="woo" text="Wet open overheid"></nldd-segmented-control-item>
            </nldd-segmented-control>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>

      <!-- Main content: the hand-rolled SVG GitFlow canvas (documented design-
           system exception — no nldd equivalent for a graph canvas). -->
      <nldd-full-bleed-section>
        <div class="diagram-area" @click="selectedStageId = null">
          <FlowDiagram
            ref="diagramRef"
            :stages="currentStages"
            :branches="currentBranches"
            :connections="currentConnections"
            :phases="currentPhases"
            :timeline="currentTimeline"
            :active-step="activeStep"
            :selected-id="selectedStageId"
            @select-stage="onSelectStage"
          />
          <FlowLegend />
        </div>
      </nldd-full-bleed-section>
    </nldd-page>

    <!-- Detail panel as a design-system sheet -->
    <StageDetail
      :stage="selectedStage"
      @close="selectedStageId = null"
    />
  </nldd-app-view>
</template>

<script setup>
import { ref, computed, onUnmounted } from 'vue';
import {
  stages as simpleStages,
  branches as simpleBranches,
  connections as simpleConnections,
} from './data/flowDataSimple.js';
import {
  stages as advancedStages,
  branches as advancedBranches,
  connections as advancedConnections,
  phases as advancedPhases,
} from './data/flowDataAdvanced.js';
import {
  stages as wooStages,
  branches as wooBranches,
  connections as wooConnections,
  phases as wooPhases,
  timelineMarkers as wooTimeline,
} from './data/flowDataWoo.js';
import FlowDiagram from './components/FlowDiagram.vue';
import FlowLegend from './components/FlowLegend.vue';
import StageDetail from './components/StageDetail.vue';

const viewMode = ref('simple');
const diagramRef = ref(null);

const datasets = {
  simple: { stages: simpleStages, branches: simpleBranches, connections: simpleConnections, phases: null, timeline: null },
  advanced: { stages: advancedStages, branches: advancedBranches, connections: advancedConnections, phases: advancedPhases, timeline: null },
  woo: { stages: wooStages, branches: wooBranches, connections: wooConnections, phases: wooPhases, timeline: wooTimeline },
};

const currentData = computed(() => datasets[viewMode.value]);
const currentStages = computed(() => currentData.value.stages);
const currentBranches = computed(() => currentData.value.branches);
const currentConnections = computed(() => currentData.value.connections);
const currentPhases = computed(() => currentData.value.phases);
const currentTimeline = computed(() => currentData.value.timeline);

const maxStep = computed(() => Math.max(0, ...currentStages.value.map((s) => s.step)));
const activeStep = ref(0);
const selectedStageId = ref(null);
const isPlaying = ref(false);
let playInterval = null;

function setViewMode(mode) {
  stopPlay();
  viewMode.value = mode;
  activeStep.value = 0;
  selectedStageId.value = null;
}

// nldd-segmented-control emits a `change` CustomEvent carrying the selected
// item value in `detail.value`.
function onViewModeChange(e) {
  const mode = e.detail?.value;
  if (mode && mode !== viewMode.value) setViewMode(mode);
}

const selectedStage = computed(() => {
  if (!selectedStageId.value) return null;
  return currentStages.value.find((s) => s.id === selectedStageId.value) || null;
});

function stepForward() {
  if (activeStep.value < maxStep.value) {
    activeStep.value++;
  } else {
    stopPlay();
  }
}

function stepBack() {
  if (activeStep.value > 0) {
    activeStep.value--;
  }
}

function resetSteps() {
  stopPlay();
  activeStep.value = 0;
  selectedStageId.value = null;
}

function goToEnd() {
  stopPlay();
  activeStep.value = maxStep.value;
}

function togglePlay() {
  if (isPlaying.value) {
    stopPlay();
  } else {
    startPlay();
  }
}

function startPlay() {
  if (activeStep.value >= maxStep.value) {
    activeStep.value = 0;
  }
  isPlaying.value = true;
  stepForward();
  playInterval = setInterval(() => {
    if (activeStep.value >= maxStep.value) {
      stopPlay();
      return;
    }
    stepForward();
  }, 1500);
}

function stopPlay() {
  isPlaying.value = false;
  if (playInterval) {
    clearInterval(playInterval);
    playInterval = null;
  }
}

function onSelectStage(id) {
  selectedStageId.value = selectedStageId.value === id ? null : id;
  stopPlay();
}

onUnmounted(() => {
  stopPlay();
});
</script>

<style scoped>
/* Brand logo sizing inside the design-system toolbar (the design system has no
   logo slot for this app-level mark). */
.app-logo {
  height: 32px;
  width: auto;
  display: block;
}

/* Centre the SVG canvas (the documented design-system exception) inside the
   full-bleed section. */
.diagram-area {
  display: flex;
  flex-direction: column;
  align-items: center;
}
</style>

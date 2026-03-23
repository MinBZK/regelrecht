<template>
  <rr-page>
    <div class="app">
      <!-- Sticky Header -->
      <header class="app__header">
        <div class="app__header-content">
          <img
            src="/assets/rijkswapen.svg"
            alt="Rijkswapen"
            class="app__logo"
          />
          <div class="app__header-text">
            <h1 class="app__title">Wetgevingsproces</h1>
            <p class="app__subtitle">Van wetsvoorstel tot geldend recht — het wetgevingsproces als GitFlow</p>
          </div>

          <!-- Playback controls -->
          <div class="header-controls">
            <button class="header-btn" @click="resetSteps" :disabled="activeStep <= 0" title="Begin" aria-label="Terug naar begin">&#x23EE;</button>
            <button class="header-btn" @click="stepBack" :disabled="activeStep <= 0" title="Vorige" aria-label="Stap terug">&#x258F;&#x25C0;</button>
            <button class="header-btn" :class="{ 'header-btn--playing': isPlaying }" @click="togglePlay" :title="isPlaying ? 'Pauzeren' : 'Afspelen'" :aria-label="isPlaying ? 'Pauzeren' : 'Afspelen'">{{ isPlaying ? '⏸' : '▶' }}</button>
            <button class="header-btn" @click="stepForward" :disabled="activeStep >= maxStep" title="Volgende" aria-label="Stap vooruit">&#x25B6;&#x258F;</button>
            <button class="header-btn" @click="goToEnd" :disabled="activeStep >= maxStep" title="Einde" aria-label="Naar einde">&#x23ED;</button>
            <span class="header-controls__step">{{ activeStep + 1 }}/{{ maxStep + 1 }}</span>
          </div>

          <!-- Zoom controls -->
          <div class="header-controls">
            <button class="header-btn" @click="diagramRef?.zoomIn()" title="Zoom in">+</button>
            <button class="header-btn" @click="diagramRef?.zoomOut()" title="Zoom uit">&minus;</button>
            <button class="header-btn" @click="diagramRef?.resetView()" title="Centreren">&#x27F2;</button>
          </div>

          <!-- View toggle -->
          <div class="app__toggle">
            <div class="toggle-bar">
              <button
                class="toggle-bar__item"
                :class="{ 'toggle-bar__item--active': viewMode === 'simple' }"
                @click="setViewMode('simple')"
              >Eenvoudig</button>
              <button
                class="toggle-bar__item"
                :class="{ 'toggle-bar__item--active': viewMode === 'advanced' }"
                @click="setViewMode('advanced')"
              >Uitgebreid</button>
              <button
                class="toggle-bar__item"
                :class="{ 'toggle-bar__item--active': viewMode === 'woo' }"
                @click="setViewMode('woo')"
              >Wet open overheid</button>
            </div>
          </div>
        </div>
      </header>

      <!-- Main content area -->
      <div class="app__content" @click="selectedStageId = null">
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

      <!-- Detail panel -->
      <StageDetail
        :stage="selectedStage"
        @close="selectedStageId = null"
      />
    </div>
  </rr-page>
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
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 100vh;
}

.app__header {
  background: var(--color-primary);
  color: white;
  padding: var(--spacing-2) var(--spacing-6);
  flex-shrink: 0;
  position: sticky;
  top: 0;
  z-index: 20;
}

.app__header-content {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
  max-width: 1600px;
  margin: 0 auto;
}

.app__logo {
  height: 36px;
  width: auto;
  filter: brightness(0) invert(1);
}

.app__header-text {
  flex: 1;
  min-width: 0;
}

.app__title {
  font-size: var(--font-size-lg);
  font-weight: 700;
  margin: 0;
  line-height: var(--line-height-tight);
}

.app__subtitle {
  font-size: var(--font-size-xs);
  margin: 1px 0 0;
  opacity: 0.75;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Header controls (playback + zoom) */
.header-controls {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}

.header-btn {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: var(--border-radius-sm);
  background: transparent;
  color: rgba(255, 255, 255, 0.8);
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--transition-fast);
}

.header-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
  color: white;
}

.header-btn:disabled {
  opacity: 0.3;
  cursor: default;
}

.header-btn--playing {
  background: rgba(255, 255, 255, 0.2);
  color: white;
}

.header-controls__step {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  min-width: 40px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

/* View toggle */
.app__toggle {
  flex-shrink: 0;
}

.toggle-bar {
  display: flex;
  border-radius: var(--border-radius-md);
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.3);
}

.toggle-bar__item {
  padding: 5px 14px;
  font-size: var(--font-size-xs);
  font-family: var(--font-family);
  font-weight: 400;
  border: none;
  cursor: pointer;
  color: rgba(255, 255, 255, 0.8);
  background: transparent;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.toggle-bar__item:hover {
  background: rgba(255, 255, 255, 0.1);
}

.toggle-bar__item--active {
  background: rgba(255, 255, 255, 0.2);
  color: white;
  font-weight: 700;
}

.app__content {
  flex: 1;
  overflow: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
}
</style>

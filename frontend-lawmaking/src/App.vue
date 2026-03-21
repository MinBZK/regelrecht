<template>
  <rr-page>
    <div class="app">
      <!-- Header -->
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
        </div>
      </header>

      <!-- Main content area -->
      <div class="app__content" @click="selectedStageId = null">
        <FlowDiagram
          :active-step="activeStep"
          :selected-id="selectedStageId"
          @select-stage="onSelectStage"
        />
        <FlowLegend />
      </div>

      <!-- Controls -->
      <FlowControls
        :active-step="activeStep"
        :max-step="maxStep"
        :is-playing="isPlaying"
        @back="stepBack"
        @forward="stepForward"
        @reset="resetSteps"
        @end="goToEnd"
        @toggle-play="togglePlay"
      />

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
import { stages } from './data/flowData.js';
import FlowDiagram from './components/FlowDiagram.vue';
import FlowControls from './components/FlowControls.vue';
import FlowLegend from './components/FlowLegend.vue';
import StageDetail from './components/StageDetail.vue';

const maxStep = computed(() => Math.max(...stages.map((s) => s.step)));
const activeStep = ref(0);
const selectedStageId = ref(null);
const isPlaying = ref(false);
let playInterval = null;

const selectedStage = computed(() => {
  if (!selectedStageId.value) return null;
  return stages.find((s) => s.id === selectedStageId.value) || null;
});

function stepForward() {
  if (activeStep.value < maxStep.value) {
    activeStep.value++;
  } else {
    stopPlay();
  }
}

function stepBack() {
  if (activeStep.value > -1) {
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

<style>
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 100vh;
}

.app__header {
  background: var(--color-primary);
  color: white;
  padding: var(--spacing-4) var(--spacing-6);
  flex-shrink: 0;
}

.app__header-content {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
  max-width: 1200px;
  margin: 0 auto;
}

.app__logo {
  height: 40px;
  width: auto;
  filter: brightness(0) invert(1);
}

.app__header-text {
  flex: 1;
}

.app__title {
  font-size: var(--font-size-xl);
  font-weight: 700;
  margin: 0;
  line-height: var(--line-height-tight);
}

.app__subtitle {
  font-size: var(--font-size-sm);
  margin: 2px 0 0;
  opacity: 0.85;
}

.app__content {
  flex: 1;
  overflow: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
}
</style>

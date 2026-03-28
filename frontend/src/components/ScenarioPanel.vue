<script setup>
import { ref, watch } from 'vue';
import { useEngine } from '../composables/useEngine.js';
import ScenarioBuilder from './ScenarioBuilder.vue';
import ScenarioGherkin from './ScenarioGherkin.vue';

const props = defineProps({
  lawId: { type: String, required: true },
  lawYaml: { type: String, default: null },
});

const mode = ref('form');
const { ready, initError, initEngine, loadDependency, getEngine } = useEngine();

// Initialize the engine on mount
initEngine().catch(() => {});

// Load the current law into the engine when YAML is available
watch(
  [() => props.lawYaml, ready],
  async ([yaml, isReady]) => {
    if (!isReady || !yaml) return;
    const engine = getEngine();
    try {
      if (engine.hasLaw(props.lawId)) {
        engine.unloadLaw(props.lawId);
      }
      engine.loadLaw(yaml);
    } catch {
      // ScenarioBuilder/ScenarioGherkin handle errors independently
    }
  },
  { immediate: true },
);

function onModeChange(event) {
  const value = event.target?.value ?? event.detail?.[0];
  if (value) mode.value = value;
}
</script>

<template>
  <div class="scenario-panel">
    <!-- Init error -->
    <div v-if="initError" class="scenario-error">
      WASM engine failed to load: {{ initError.message }}
      <div class="scenario-error-hint">
        Run <code>just wasm-build</code> to build the WASM module.
      </div>
    </div>

    <template v-else>
      <!-- Mode toggle -->
      <div class="scenario-mode-bar">
        <rr-segmented-control size="md" :value="mode" @change="onModeChange">
          <rr-segmented-control-item value="form">Formulier</rr-segmented-control-item>
          <rr-segmented-control-item value="gherkin">Gherkin</rr-segmented-control-item>
        </rr-segmented-control>
      </div>

      <!-- Form mode -->
      <ScenarioBuilder
        v-if="mode === 'form'"
        :law-id="lawId"
        :law-yaml="lawYaml"
        :engine="getEngine()"
        :ready="ready"
      />

      <!-- Gherkin mode -->
      <ScenarioGherkin
        v-if="mode === 'gherkin'"
        :law-id="lawId"
        :engine="getEngine()"
        :ready="ready"
        :load-dependency="loadDependency"
      />
    </template>
  </div>
</template>

<style scoped>
.scenario-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.scenario-mode-bar {
  padding: 8px 16px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.scenario-error {
  padding: 12px 16px;
  background: #fee;
  color: #c00;
  font-size: 13px;
}
.scenario-error-hint {
  margin-top: 4px;
  font-size: 12px;
  color: #999;
}
.scenario-error-hint code {
  background: #eee;
  padding: 1px 4px;
  border-radius: 3px;
}
</style>

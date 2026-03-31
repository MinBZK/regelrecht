<script setup>
import { ref, computed, watch } from 'vue';
import { useDependencies } from '../composables/useDependencies.js';
import { useScenarios } from '../composables/useScenarios.js';
import { parseFeature } from '../gherkin/parser.js';
import { mapFeatureToForm, getEffectiveSetup } from '../gherkin/formMapper.js';
import ScenarioForm from './ScenarioForm.vue';

const props = defineProps({
  lawId: { type: String, required: true },
  lawYaml: { type: String, default: null },
  engine: { type: Object, default: null },
  ready: { type: Boolean, default: false },
});

const emit = defineEmits(['executed']);

// --- Dependencies ---
const {
  loading: depsLoading,
  progress: depsProgress,
  error: depsError,
  loadAllDependencies,
} = useDependencies();

// --- Scenario loading ---
const lawIdRef = computed(() => props.lawId);
const {
  scenarios: scenarioFiles,
  selectedScenario: selectedScenarioFile,
  featureText,
  loading: scenariosLoading,
  selectScenario: selectScenarioFile,
} = useScenarios(lawIdRef);

const formState = ref(null);

// Parse feature file when loaded
watch(featureText, (text) => {
  if (!text) {
    formState.value = null;
    return;
  }
  try {
    const parsed = parseFeature(text);
    formState.value = mapFeatureToForm(parsed);
  } catch {
    formState.value = null;
  }
});

// Cache for fetched law YAML texts
const yamlCache = {};

async function fetchLawYaml(lawId) {
  if (yamlCache[lawId]) return yamlCache[lawId];
  const res = await fetch(`/api/corpus/laws/${encodeURIComponent(lawId)}`);
  if (!res.ok) throw new Error(`Failed to fetch law '${lawId}': ${res.status}`);
  const text = await res.text();
  yamlCache[lawId] = text;
  return text;
}

// --- Load dependencies when law YAML changes ---
let watchVersion = 0;

watch(
  [() => props.lawYaml, () => props.ready],
  async ([lawYaml, isReady]) => {
    if (!lawYaml || !isReady || !props.engine) return;
    const version = ++watchVersion;
    await loadAllDependencies(lawYaml, props.engine, fetchLawYaml);
    if (version !== watchVersion) return;

    // Also load dependencies from scenario background
    if (formState.value?.background?.dependencies) {
      for (const depId of formState.value.background.dependencies) {
        try {
          if (!props.engine.hasLaw(depId)) {
            const yaml = await fetchLawYaml(depId);
            props.engine.loadLaw(yaml);
          }
        } catch (e) {
          console.warn(`Failed to load scenario dependency '${depId}':`, e);
        }
      }
    }
  },
  { immediate: true },
);

// Compute effective setup per scenario
function getSetup(index) {
  if (!formState.value) return null;
  return getEffectiveSetup(formState.value, index);
}

function onScenarioFileSelect(event) {
  const filename = event.target.value;
  if (filename) selectScenarioFile(filename);
}

function onScenarioExecuted(data) {
  emit('executed', data);
}
</script>

<template>
  <div class="sb-container">
    <div class="sb-scroll">
      <!-- Feature file selector -->
      <div class="sb-section" v-if="scenarioFiles.length > 1 || scenariosLoading">
        <div v-if="scenariosLoading" class="sb-loading">Scenario's laden...</div>
        <select
          v-else
          class="sb-select"
          :value="selectedScenarioFile"
          @change="onScenarioFileSelect"
        >
          <option v-for="sf in scenarioFiles" :key="sf.filename" :value="sf.filename">
            {{ sf.filename }}
          </option>
        </select>
      </div>

      <!-- Dependencies loading -->
      <div v-if="depsLoading" class="sb-section sb-deps-loading">
        <div class="sb-section-title">Afhankelijkheden laden</div>
        <div class="sb-loading">{{ depsProgress }}</div>
      </div>
      <div v-else-if="depsError" class="sb-section sb-error">
        Fout: {{ depsError }}
      </div>

      <!-- Scenario accordion -->
      <template v-if="formState">
        <details
          v-for="(scenario, i) in formState.scenarios"
          :key="i"
          class="sb-accordion"
          :open="i === 0 || undefined"
        >
          <summary class="sb-accordion-header">
            <span class="sb-accordion-title">{{ scenario.name }}</span>
          </summary>
          <div class="sb-accordion-body">
            <ScenarioForm
              :scenario="scenario"
              :setup="getSetup(i)"
              :engine="engine"
              :ready="ready"
              :law-id="lawId"
              @executed="onScenarioExecuted"
            />
          </div>
        </details>
      </template>

      <!-- No scenarios -->
      <div v-else-if="!scenariosLoading && !depsLoading" class="sb-section sb-empty">
        Geen scenario's beschikbaar voor deze wet.
      </div>
    </div>
  </div>
</template>

<style scoped>
.sb-container {
  height: 100%;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.sb-scroll {
  height: 100%;
  overflow-y: auto;
}

.sb-section {
  padding: 12px 16px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sb-section-title {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 4px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sb-select {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 6px;
  font-size: 13px;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
  background: white;
}

.sb-loading {
  font-size: 12px;
  color: var(--semantics-text-color-secondary, #666);
  font-style: italic;
}

.sb-deps-loading {
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
}

.sb-error {
  background: #fee;
  color: #c00;
  font-size: 13px;
}

.sb-empty {
  color: var(--semantics-text-color-secondary, #666);
  font-size: 13px;
  text-align: center;
  padding: 32px 16px;
}

/* Accordion */
.sb-accordion {
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sb-accordion-header {
  padding: 10px 16px;
  cursor: pointer;
  user-select: none;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sb-accordion-header:hover {
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
}

.sb-accordion-header::marker {
  color: var(--semantics-text-color-secondary, #999);
}

.sb-accordion-title {
  flex: 1;
}

.sb-accordion-body {
  padding: 0 16px 12px;
}
</style>

<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { useDependencies } from '../composables/useDependencies.js';
import { useScenarios } from '../composables/useScenarios.js';
import { parseFeature } from '../gherkin/parser.js';
import { mapFeatureToForm, getEffectiveSetup, formStateToGherkin, syncEditedValues } from '../gherkin/formMapper.js';
import { matchStatus } from '../utils/outputFormat.js';
import { buildArticleMap } from '../utils/articleMapping.js';
import ScenarioForm from './ScenarioForm.vue';

const props = defineProps({
  lawId: { type: String, required: true },
  lawYaml: { type: String, default: null },
  engine: { type: Object, default: null },
  ready: { type: Boolean, default: false },
  /** Articles array from useLaw() for article mapping */
  articles: { type: Array, default: () => [] },
});

const emit = defineEmits(['executed']);

// --- Article mapping ---
const articleMap = computed(() => buildArticleMap(props.articles));

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
  saving,
  saveError,
  selectScenario: selectScenarioFile,
  saveScenario,
} = useScenarios(lawIdRef);

const formState = ref(null);
const saveSuccess = ref(false);

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

// --- Dependencies ready tracking ---
const depsReady = ref(false);

// --- Load dependencies when law YAML changes ---
let watchVersion = 0;

watch(
  [() => props.lawYaml, () => props.ready, formState],
  async ([lawYaml, isReady]) => {
    if (!lawYaml || !isReady || !props.engine) return;
    const version = ++watchVersion;
    depsReady.value = false;

    await loadAllDependencies(lawYaml, props.engine, fetchLawYaml);
    if (version !== watchVersion) return;

    // Also load dependencies from scenario background + per-scenario steps
    if (formState.value) {
      const allDeps = new Set();
      for (const dep of formState.value.background?.dependencies || []) allDeps.add(dep);
      for (const sc of formState.value.scenarios || []) {
        for (const dep of sc.setup?.dependencies || []) allDeps.add(dep);
      }
      for (const depId of allDeps) {
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

    if (version === watchVersion) {
      depsReady.value = true;
    }
  },
  { immediate: true },
);

// --- Template refs for ScenarioForm instances ---
const scenarioRefs = ref([]);

// --- Per-scenario result tracking ---
const scenarioResults = ref(new Map());

function onScenarioResult(index, data) {
  const updated = new Map(scenarioResults.value);
  updated.set(index, data);
  scenarioResults.value = updated;
}

function scenarioStatus(index) {
  const data = scenarioResults.value.get(index);
  if (!data) return null;
  if (data.error) return 'failed';
  if (!data.result) return null;

  const scenario = formState.value?.scenarios[index];
  if (!scenario) return null;

  const checkable = (scenario.assertions || []).filter(
    (a) => a.outputName && a.value != null,
  );
  if (checkable.length === 0) return null;

  for (const a of checkable) {
    const status = matchStatus(
      a.outputName,
      data.result.outputs?.[a.outputName],
      { [a.outputName]: String(a.value) },
    );
    if (status === 'failed') return 'failed';
  }
  return 'passed';
}

// Reset results and refs when formState changes (new scenario file loaded)
watch(formState, () => {
  scenarioResults.value = new Map();
  scenarioRefs.value = [];
});

// --- Auto-execute all scenarios sequentially when deps are ready ---
let autoExecuteVersion = 0;

watch(
  [depsReady, formState],
  async ([ready, state]) => {
    if (!ready || !state || !state.scenarios?.length) return;
    const version = ++autoExecuteVersion;

    // Wait one tick so ScenarioForm refs are mounted
    await nextTick();
    if (version !== autoExecuteVersion) return;

    for (let i = 0; i < state.scenarios.length; i++) {
      if (version !== autoExecuteVersion) return;
      const formRef = scenarioRefs.value[i];
      if (formRef?.execute) {
        formRef.execute();
        // Collect result after execution
        const data = formRef.getExecutionData?.();
        if (data) onScenarioResult(i, data);
      }
    }
  },
);

// --- Details handler: emit to right panel ---
function onShowDetails(index) {
  // Prefer fresh data from the form ref, fall back to cached results
  const formRef = scenarioRefs.value[index];
  const data = formRef?.getExecutionData?.() || scenarioResults.value.get(index);
  if (data) {
    emit('executed', {
      result: data.result,
      traceText: data.traceText,
      error: data.error,
      expectations: data.expectations || {},
    });
  }
}

// Memoized setup per scenario (avoids new object on every render)
const scenarioSetups = computed(() => {
  if (!formState.value) return [];
  return formState.value.scenarios.map((_, i) => getEffectiveSetup(formState.value, i));
});

function onScenarioFileSelect(event) {
  const filename = event.target.value;
  if (filename) selectScenarioFile(filename);
}

async function onSave() {
  if (!formState.value || !selectedScenarioFile.value) return;

  // Sync edited input values back to formState before serializing
  for (let i = 0; i < (formState.value.scenarios || []).length; i++) {
    const formRef = scenarioRefs.value[i];
    if (!formRef?.getFormValues) continue;
    const values = formRef.getFormValues();
    syncEditedValues(formState.value, i, values);
  }

  saveSuccess.value = false;
  try {
    const gherkin = formStateToGherkin(formState.value);
    await saveScenario(selectedScenarioFile.value, gherkin);
    saveSuccess.value = true;
    setTimeout(() => { saveSuccess.value = false; }, 3000);
  } catch {
    // saveError is already set by the composable
  }
}
</script>

<template>
  <div class="sb-container">
    <div class="sb-scroll">
      <!-- Feature file selector + save button -->
      <div class="sb-section sb-toolbar" v-if="scenarioFiles.length > 0 || scenariosLoading">
        <div v-if="scenariosLoading" class="sb-loading">Scenario's laden...</div>
        <template v-else>
          <select
            v-if="scenarioFiles.length > 1"
            class="sb-select"
            :value="selectedScenarioFile"
            @change="onScenarioFileSelect"
          >
            <option v-for="sf in scenarioFiles" :key="sf.filename" :value="sf.filename">
              {{ sf.filename }}
            </option>
          </select>
          <button
            v-if="formState"
            class="sb-save-btn"
            :disabled="saving"
            @click="onSave"
          >
            {{ saving ? 'Opslaan...' : 'Opslaan' }}
          </button>
        </template>
      </div>

      <!-- Save feedback -->
      <div v-if="saveSuccess" class="sb-section sb-save-success">Opgeslagen</div>
      <div v-if="saveError" class="sb-section sb-error">
        Opslaan mislukt: {{ saveError.message || saveError }}
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
        <ndd-spacer size="8"></ndd-spacer>
        <details
          v-for="(scenario, i) in formState.scenarios"
          :key="i"
          class="sb-accordion"
        >
          <summary
            class="sb-accordion-header"
            :class="{
              'sb-header--pass': scenarioStatus(i) === 'passed',
              'sb-header--fail': scenarioStatus(i) === 'failed',
            }"
          >
            <span class="sb-accordion-title">{{ scenario.name }}</span>
            <span v-if="scenarioStatus(i) === 'passed'" class="sb-badge sb-badge--pass">&#x2713;</span>
            <span v-else-if="scenarioStatus(i) === 'failed'" class="sb-badge sb-badge--fail">&#x2717;</span>
          </summary>
          <div class="sb-accordion-body">
            <ScenarioForm
              v-if="scenarioSetups[i]"
              :ref="(el) => { scenarioRefs[i] = el; }"
              :scenario="scenario"
              :setup="scenarioSetups[i]"
              :engine="engine"
              :ready="ready"
              :law-id="lawId"
              :article-map="articleMap"
              @show-details="() => onShowDetails(i)"
              @executed="(data) => onScenarioResult(i, data)"
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
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
}

.sb-scroll {
  height: 100%;
  overflow-y: auto;
}

.sb-section {
  padding: 12px 16px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sb-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.sb-section-title {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 4px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sb-select {
  flex: 1;
  padding: 6px 8px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 6px;
  font-size: 13px;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  background: white;
}

.sb-save-btn {
  padding: 6px 14px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 6px;
  font-size: 13px;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  background: var(--semantics-surfaces-color-primary, white);
  color: var(--semantics-text-color-primary, #1C2029);
  cursor: pointer;
  white-space: nowrap;
}

.sb-save-btn:hover:not(:disabled) {
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
}

.sb-save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
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

.sb-save-success {
  background: #efe;
  color: #060;
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

.sb-header--pass {
  background: #e8f5e9;
}

.sb-header--pass:hover {
  background: #c8e6c9;
}

.sb-header--fail {
  background: #ffebee;
}

.sb-header--fail:hover {
  background: #ffcdd2;
}

.sb-accordion-title {
  flex: 1;
}

.sb-badge {
  font-size: 12px;
  font-weight: 700;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  flex-shrink: 0;
}

.sb-badge--pass {
  background: #2e7d32;
  color: white;
}

.sb-badge--fail {
  background: #c62828;
  color: white;
}

.sb-accordion-body {
  padding: 0 16px 12px;
}
</style>

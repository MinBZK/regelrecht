<script setup>
import { ref, computed, watch } from 'vue';
import yaml from 'js-yaml';
import { useDependencies } from '../composables/useDependencies.js';
import { useDataSourceSchema } from '../composables/useDataSourceSchema.js';
import { generateGherkin } from '../gherkin/generator.js';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioResults from './ScenarioResults.vue';

const props = defineProps({
  lawId: { type: String, required: true },
  lawYaml: { type: String, default: null },
  engine: { type: Object, default: null },
  ready: { type: Boolean, default: false },
});

// --- Dependencies ---
const {
  loading: depsLoading,
  loadedDeps,
  progress: depsProgress,
  error: depsError,
  loadAllDependencies,
} = useDependencies();

// --- Data source schema ---
const {
  dataSourceGroups,
  outputs: lawOutputs,
  parameters: lawParameters,
  buildSchema,
} = useDataSourceSchema();

// --- Form state ---
const calculationDate = ref('2025-01-01');
const parameterValues = ref({});
const dataSourceRows = ref({});  // keyed by "lawId:articleNumber"
const selectedOutputs = ref([]);
const expectations = ref({});

// --- Execution state ---
const result = ref(null);
const running = ref(false);
const runError = ref(null);

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
watch(
  [() => props.lawYaml, () => props.ready],
  async ([lawYaml, isReady]) => {
    if (!lawYaml || !isReady || !props.engine) return;

    // Reset state
    result.value = null;
    runError.value = null;

    // Load dependencies
    await loadAllDependencies(lawYaml, props.engine, fetchLawYaml);

    // Build schema from main law + deps
    await buildSchema(lawYaml, loadedDeps.value, fetchLawYaml);

    // Initialize parameter values
    const params = {};
    for (const p of lawParameters.value) {
      params[p.name] = parameterValues.value[p.name] || '';
    }
    parameterValues.value = params;

    // Initialize selected outputs with all outputs checked
    selectedOutputs.value = lawOutputs.value.map((o) => o.name);
  },
  { immediate: true },
);

// --- Data source row getter/setter ---
function getRows(group) {
  const key = `${group.lawId}:${group.articleNumber}`;
  return dataSourceRows.value[key] || [];
}

function setRows(group, rows) {
  const key = `${group.lawId}:${group.articleNumber}`;
  dataSourceRows.value = { ...dataSourceRows.value, [key]: rows };
}

// --- Gherkin preview ---
const gherkinPreview = computed(() => {
  const dataSources = dataSourceGroups.value.map((group) => {
    const key = `${group.lawId}:${group.articleNumber}`;
    return {
      sourceName: `${group.lawId}_${group.articleNumber}`,
      keyField: group.keyField,
      rows: dataSourceRows.value[key] || [],
    };
  });

  const outputs = selectedOutputs.value.map((name) => ({
    name,
    expectedValue: expectations.value[name] ?? null,
  }));

  const mainLaw = props.lawYaml ? yaml.load(props.lawYaml) : null;
  const lawName = mainLaw?.name?.startsWith?.('#')
    ? resolveLawName(mainLaw)
    : (mainLaw?.name || props.lawId);

  return generateGherkin({
    lawId: props.lawId,
    lawName,
    calculationDate: calculationDate.value,
    parameters: parameterValues.value,
    dataSources,
    selectedOutputs: outputs,
  });
});

function resolveLawName(law) {
  const name = law.name;
  if (typeof name === 'string' && name.startsWith('#')) {
    const outputName = name.slice(1);
    for (const article of law.articles || []) {
      const actions = article.machine_readable?.execution?.actions;
      if (!actions) continue;
      for (const action of actions) {
        if (action.output === outputName && typeof action.value === 'string') {
          return action.value;
        }
      }
    }
  }
  return name || law.$id || '';
}

// --- Execute ---
async function execute() {
  if (!props.engine || !props.ready) return;

  running.value = true;
  result.value = null;
  runError.value = null;

  try {
    const engine = props.engine;

    // Clear previous data sources
    engine.clearDataSources();

    // Register data source tables
    for (const group of dataSourceGroups.value) {
      const key = `${group.lawId}:${group.articleNumber}`;
      const rows = dataSourceRows.value[key] || [];
      if (rows.length === 0) continue;

      // Use a descriptive source name
      const sourceName = `${group.lawId}_art${group.articleNumber}`;
      engine.registerDataSource(sourceName, group.keyField, rows);
    }

    // Build parameters
    const params = {};
    for (const [k, v] of Object.entries(parameterValues.value)) {
      if (v !== '' && v !== null && v !== undefined) {
        params[k] = v;
      }
    }

    // Execute for the first selected output (the engine returns all outputs)
    const outputName = selectedOutputs.value[0];
    if (!outputName) {
      throw new Error('Selecteer minimaal één output');
    }

    result.value = engine.execute(
      props.lawId,
      outputName,
      params,
      calculationDate.value,
    );
  } catch (e) {
    runError.value = e.message || String(e);
  } finally {
    running.value = false;
  }
}

// --- Output toggle ---
function toggleOutput(name) {
  const idx = selectedOutputs.value.indexOf(name);
  if (idx >= 0) {
    selectedOutputs.value = selectedOutputs.value.filter((n) => n !== name);
  } else {
    selectedOutputs.value = [...selectedOutputs.value, name];
  }
}

function setExpectation(name, value) {
  expectations.value = { ...expectations.value, [name]: value };
}

// Count data sources with data
const filledSourceCount = computed(() => {
  let count = 0;
  for (const group of dataSourceGroups.value) {
    const key = `${group.lawId}:${group.articleNumber}`;
    if ((dataSourceRows.value[key] || []).length > 0) count++;
  }
  return count;
});
</script>

<template>
  <div class="sb-container">
    <!-- Split pane layout -->
    <div class="sb-split">
      <!-- LEFT: Input panel -->
      <div class="sb-input-panel">
        <div class="sb-scroll">
          <!-- Dependencies loading -->
          <div v-if="depsLoading" class="sb-section sb-deps-loading">
            <div class="sb-section-title">Afhankelijkheden laden</div>
            <div class="sb-deps-progress">{{ depsProgress }}</div>
          </div>
          <div v-else-if="depsError" class="sb-section sb-deps-error">
            Fout: {{ depsError }}
          </div>

          <!-- Calculation date -->
          <div class="sb-section">
            <label class="sb-label">Berekeningsdatum</label>
            <input
              type="date"
              class="sb-date-input"
              v-model="calculationDate"
            />
          </div>

          <!-- Parameters -->
          <div v-if="lawParameters.length > 0" class="sb-section">
            <div class="sb-section-title">Parameters</div>
            <div v-for="param in lawParameters" :key="param.name" class="sb-param-row">
              <label class="sb-param-label">{{ param.name }}</label>
              <input
                class="sb-param-input"
                :type="param.type === 'number' ? 'number' : 'text'"
                :value="parameterValues[param.name] || ''"
                @input="parameterValues = { ...parameterValues, [param.name]: $event.target.value }"
                :placeholder="param.name"
              />
            </div>
          </div>

          <!-- Data sources -->
          <div v-if="dataSourceGroups.length > 0" class="sb-section">
            <div class="sb-section-title">
              Gegevensbronnen
              <span class="sb-section-badge" v-if="!depsLoading">
                {{ filledSourceCount }}/{{ dataSourceGroups.length }}
              </span>
            </div>

            <DataSourceTable
              v-for="group in dataSourceGroups"
              :key="`${group.lawId}:${group.articleNumber}`"
              :title="group.lawName"
              :key-field="group.keyField"
              :fields="group.fields"
              :model-value="getRows(group)"
              @update:model-value="setRows(group, $event)"
            />
          </div>

          <!-- Outputs -->
          <div v-if="lawOutputs.length > 0" class="sb-section">
            <div class="sb-section-title">Output</div>
            <div v-for="output in lawOutputs" :key="output.name" class="sb-output-row">
              <label class="sb-output-check">
                <input
                  type="checkbox"
                  :checked="selectedOutputs.includes(output.name)"
                  @change="toggleOutput(output.name)"
                />
                <span>{{ output.name }}</span>
              </label>
              <div v-if="selectedOutputs.includes(output.name)" class="sb-output-expect">
                <label class="sb-expect-label">Verwacht:</label>
                <template v-if="output.type === 'boolean'">
                  <label class="sb-radio">
                    <input
                      type="radio"
                      :name="`expect-${output.name}`"
                      value="true"
                      :checked="expectations[output.name] === 'true'"
                      @change="setExpectation(output.name, 'true')"
                    />
                    ja
                  </label>
                  <label class="sb-radio">
                    <input
                      type="radio"
                      :name="`expect-${output.name}`"
                      value="false"
                      :checked="expectations[output.name] === 'false'"
                      @change="setExpectation(output.name, 'false')"
                    />
                    nee
                  </label>
                  <label class="sb-radio">
                    <input
                      type="radio"
                      :name="`expect-${output.name}`"
                      value=""
                      :checked="!expectations[output.name]"
                      @change="setExpectation(output.name, null)"
                    />
                    &mdash;
                  </label>
                </template>
                <template v-else>
                  <input
                    class="sb-expect-input"
                    type="text"
                    :value="expectations[output.name] || ''"
                    @input="setExpectation(output.name, $event.target.value || null)"
                    placeholder="waarde"
                  />
                </template>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- RIGHT: Scenario panel -->
      <div class="sb-scenario-panel">
        <div class="sb-scroll">
          <!-- Gherkin preview -->
          <div class="sb-gherkin-preview">
            <pre class="sb-gherkin-code">{{ gherkinPreview }}</pre>
          </div>

          <!-- Execute button -->
          <div class="sb-execute-bar">
            <button
              class="sb-execute-btn"
              @click="execute"
              :disabled="!ready || running || selectedOutputs.length === 0"
              type="button"
            >
              {{ running ? 'Bezig...' : 'Uitvoeren \u25B6' }}
            </button>
          </div>

          <!-- Results -->
          <ScenarioResults
            :result="result"
            :expectations="expectations"
            :error="runError"
            :running="running"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sb-container {
  height: 100%;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.sb-split {
  display: flex;
  height: 100%;
}

.sb-input-panel {
  flex: 1;
  border-right: 1px solid var(--semantics-dividers-color, #E0E3E8);
  min-width: 0;
}

.sb-scenario-panel {
  flex: 1;
  min-width: 0;
}

.sb-scroll {
  height: 100%;
  overflow-y: auto;
}

/* Sections */
.sb-section {
  padding: 12px 16px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sb-section-title {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 8px;
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sb-section-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--semantics-surfaces-color-secondary, #F0F1F3);
  color: var(--semantics-text-color-secondary, #666);
}

/* Date input */
.sb-label {
  display: block;
  font-weight: 600;
  font-size: 12px;
  margin-bottom: 4px;
  color: var(--semantics-text-color-secondary, #666);
}

.sb-date-input {
  padding: 6px 8px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 6px;
  font-size: 13px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  width: 160px;
}

.sb-date-input:focus {
  outline: none;
  border-color: #154273;
}

/* Parameters */
.sb-param-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.sb-param-label {
  font-size: 12px;
  font-weight: 600;
  min-width: 60px;
  color: var(--semantics-text-color-secondary, #666);
}

.sb-param-input {
  flex: 1;
  padding: 5px 8px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 6px;
  font-size: 13px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.sb-param-input:focus {
  outline: none;
  border-color: #154273;
}

/* Dependencies loading */
.sb-deps-loading {
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
}

.sb-deps-progress {
  font-size: 12px;
  color: var(--semantics-text-color-secondary, #666);
  font-style: italic;
}

.sb-deps-error {
  background: #fee;
  color: #c00;
  font-size: 13px;
}

/* Outputs */
.sb-output-row {
  margin-bottom: 8px;
}

.sb-output-check {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  cursor: pointer;
}

.sb-output-check input[type="checkbox"] {
  margin: 0;
}

.sb-output-expect {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
  padding-left: 22px;
}

.sb-expect-label {
  font-size: 11px;
  color: var(--semantics-text-color-secondary, #666);
}

.sb-radio {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 12px;
  cursor: pointer;
}

.sb-radio input[type="radio"] {
  margin: 0;
}

.sb-expect-input {
  padding: 3px 6px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 4px;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  width: 120px;
}

.sb-expect-input:focus {
  outline: none;
  border-color: #154273;
}

/* Gherkin preview */
.sb-gherkin-preview {
  background: #1e1e2e;
  min-height: 120px;
}

.sb-gherkin-code {
  margin: 0;
  padding: 16px;
  color: #cdd6f4;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

/* Execute bar */
.sb-execute-bar {
  padding: 8px 16px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sb-execute-btn {
  padding: 8px 20px;
  background: #154273;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.sb-execute-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.sb-execute-btn:hover:not(:disabled) {
  background: #1a5490;
}
</style>

<script setup>
import { ref, computed, watch } from 'vue';
import { useDependencies } from '../composables/useDependencies.js';
import { useDataSourceSchema } from '../composables/useDataSourceSchema.js';
import { useScenarios } from '../composables/useScenarios.js';
import { parseFeature } from '../gherkin/parser.js';
import { mapFeatureToForm, getEffectiveSetup } from '../gherkin/formMapper.js';
import DataSourceTable from './DataSourceTable.vue';

const props = defineProps({
  lawId: { type: String, required: true },
  lawYaml: { type: String, default: null },
  engine: { type: Object, default: null },
  ready: { type: Boolean, default: false },
  running: { type: Boolean, default: false },
});

const emit = defineEmits(['execute']);

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

// --- Scenario loading ---
const lawIdRef = computed(() => props.lawId);
const {
  scenarios: scenarioFiles,
  selectedScenario: selectedScenarioFile,
  featureText,
  loading: scenariosLoading,
  selectScenario: selectScenarioFile,
} = useScenarios(lawIdRef);

const selectedScenarioIndex = ref(0);
const formState = ref(null);

// Parse feature file when loaded
watch(featureText, (text) => {
  if (!text) {
    formState.value = null;
    selectedScenarioIndex.value = 0;
    return;
  }
  try {
    const parsed = parseFeature(text);
    formState.value = mapFeatureToForm(parsed);
    selectedScenarioIndex.value = 0;
    // Auto-populate from first scenario
    if (formState.value.scenarios.length > 0) {
      populateFromScenario(0);
    }
  } catch {
    formState.value = null;
  }
});

// --- Form state ---
const calculationDate = ref(new Date().toISOString().slice(0, 10));
const parameterValues = ref({});
const dataSourceRows = ref({});
const selectedOutputs = ref([]);
const expectations = ref({});

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

    // Load dependencies
    await loadAllDependencies(lawYaml, props.engine, fetchLawYaml);
    if (version !== watchVersion) return;

    // Build schema from main law + deps
    await buildSchema(lawYaml, loadedDeps.value, fetchLawYaml);
    if (version !== watchVersion) return;

    // Initialize parameter values (only if not already populated from scenario)
    if (Object.keys(parameterValues.value).length === 0) {
      const params = {};
      for (const p of lawParameters.value) {
        params[p.name] = '';
      }
      parameterValues.value = params;
    }

    // Initialize selected outputs with all outputs checked
    if (selectedOutputs.value.length === 0) {
      selectedOutputs.value = lawOutputs.value.map((o) => o.name);
    }
  },
  { immediate: true },
);

// --- Populate form from scenario ---
async function populateFromScenario(index) {
  if (!formState.value) return;
  selectedScenarioIndex.value = index;

  const setup = getEffectiveSetup(formState.value, index);
  if (!setup) return;

  const scenario = formState.value.scenarios[index];

  // Set calculation date
  if (setup.calculationDate) {
    calculationDate.value = setup.calculationDate;
  }

  // Set parameters
  const params = {};
  for (const p of setup.parameters) {
    params[p.name] = p.value;
  }
  // Merge with existing parameter names from schema
  for (const p of lawParameters.value) {
    if (!(p.name in params)) {
      params[p.name] = parameterValues.value[p.name] ?? '';
    }
  }
  parameterValues.value = params;

  // Store scenario data sources directly by their feature file source name.
  // These are registered with the engine at execution time using the exact
  // names the law YAML expects (e.g., "personal_data", "insurance", "box1").
  const newRows = {};
  for (const ds of setup.dataSources) {
    const rows = ds.rows.map((row, i) => {
      const obj = { _id: i };
      ds.headers.forEach((h, j) => { obj[h] = row[j] ?? ''; });
      return obj;
    });
    newRows[`_direct:${ds.sourceName}`] = {
      sourceName: ds.sourceName,
      keyField: ds.keyField,
      rows,
    };
  }
  dataSourceRows.value = newRows;

  // Set expectations from assertions
  const exp = {};
  for (const assertion of scenario.assertions || []) {
    if (assertion.outputName && assertion.value !== null && assertion.value !== undefined) {
      exp[assertion.outputName] = String(assertion.value);
    }
  }
  expectations.value = exp;

  // Select outputs referenced in assertions
  if (scenario.assertions?.length > 0) {
    const assertedOutputs = scenario.assertions
      .filter((a) => a.outputName)
      .map((a) => a.outputName);
    if (assertedOutputs.length > 0) {
      selectedOutputs.value = [...new Set([...assertedOutputs, ...selectedOutputs.value])];
    }
  }

  // Load dependencies specified in scenario
  if (props.engine && props.ready) {
    for (const depId of setup.dependencies) {
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
}

function onScenarioFileSelect(event) {
  const filename = event.target.value;
  if (filename) selectScenarioFile(filename);
}

function onScenarioSelect(event) {
  const index = parseInt(event.target.value, 10);
  if (!isNaN(index)) populateFromScenario(index);
}

// --- Data source row getter/setter ---
function getRows(group) {
  const key = `${group.lawId}:${group.articleNumber}`;
  return dataSourceRows.value[key] || [];
}

function setRows(group, rows) {
  const key = `${group.lawId}:${group.articleNumber}`;
  dataSourceRows.value = { ...dataSourceRows.value, [key]: rows };
}

// --- Execute (emit to parent) ---
function handleExecute() {
  const outputName = selectedOutputs.value[0];
  if (!outputName) return;

  // Collect all data sources
  const dataSources = [];
  for (const group of dataSourceGroups.value) {
    const key = `${group.lawId}:${group.articleNumber}`;
    const rows = dataSourceRows.value[key] || [];
    if (rows.length === 0) continue;
    dataSources.push({
      sourceName: `${group.lawId}_art${group.articleNumber}`,
      keyField: group.keyField,
      rows,
    });
  }

  // Also include directly-keyed data sources from feature files
  for (const [key, value] of Object.entries(dataSourceRows.value)) {
    if (key.startsWith('_direct:') && value.rows?.length > 0) {
      dataSources.push({
        sourceName: value.sourceName,
        keyField: value.keyField,
        rows: value.rows,
      });
    }
  }

  emit('execute', {
    lawId: props.lawId,
    outputName,
    parameters: { ...parameterValues.value },
    calculationDate: calculationDate.value,
    dataSources,
    expectations: { ...expectations.value },
  });
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

// Count data sources with data (schema groups + direct scenario sources)
const filledSourceCount = computed(() => {
  let count = 0;
  for (const group of dataSourceGroups.value) {
    const key = `${group.lawId}:${group.articleNumber}`;
    if ((dataSourceRows.value[key] || []).length > 0) count++;
  }
  for (const [key, value] of Object.entries(dataSourceRows.value)) {
    if (key.startsWith('_direct:') && value.rows?.length > 0) count++;
  }
  return count;
});

const directSourceCount = computed(() => {
  let count = 0;
  for (const key of Object.keys(dataSourceRows.value)) {
    if (key.startsWith('_direct:')) count++;
  }
  return count;
});

const totalSourceCount = computed(() =>
  dataSourceGroups.value.length + directSourceCount.value,
);

const scenarioNames = computed(() => {
  if (!formState.value) return [];
  return formState.value.scenarios.map((s) => s.name);
});
</script>

<template>
  <div class="sb-container">
    <div class="sb-scroll">
      <!-- Scenario selector -->
      <div class="sb-section" v-if="scenarioFiles.length > 0 || scenariosLoading">
        <div class="sb-section-title">Scenario</div>
        <div v-if="scenariosLoading" class="sb-deps-progress">Scenario's laden...</div>
        <template v-else>
          <select
            v-if="scenarioFiles.length > 0"
            class="sb-select"
            :value="selectedScenarioFile"
            @change="onScenarioFileSelect"
          >
            <option v-for="sf in scenarioFiles" :key="sf.filename" :value="sf.filename">
              {{ sf.filename }}
            </option>
          </select>
          <select
            v-if="scenarioNames.length > 1"
            class="sb-select sb-select--scenario"
            :value="selectedScenarioIndex"
            @change="onScenarioSelect"
          >
            <option v-for="(name, i) in scenarioNames" :key="i" :value="i">
              {{ name }}
            </option>
          </select>
        </template>
      </div>

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
            :value="parameterValues[param.name] ?? ''"
            @input="parameterValues = { ...parameterValues, [param.name]: $event.target.value }"
            :placeholder="param.name"
          />
        </div>
      </div>

      <!-- Data sources -->
      <div v-if="dataSourceGroups.length > 0 || directSourceCount > 0" class="sb-section">
        <div class="sb-section-title">
          Gegevensbronnen
          <span class="sb-section-badge" v-if="!depsLoading">
            {{ filledSourceCount }}/{{ totalSourceCount }}
          </span>
        </div>

        <!-- Scenario-loaded data sources (from feature file) -->
        <div v-for="(value, key) in dataSourceRows" :key="key">
          <div v-if="key.startsWith('_direct:')" class="sb-scenario-source">
            <div class="sb-scenario-source-header">
              {{ value.sourceName }}
              <span class="sb-section-badge">{{ value.rows.length }} rij(en)</span>
            </div>
            <div class="sb-scenario-source-fields">
              <span v-for="field in Object.keys(value.rows[0] || {}).filter(k => k !== '_id')" :key="field" class="sb-scenario-field">
                {{ field }}
              </span>
            </div>
          </div>
        </div>

        <!-- Schema-derived data source tables (for manual entry) -->
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

      <!-- Execute button -->
      <div class="sb-execute-bar">
        <button
          class="sb-execute-btn"
          @click="handleExecute"
          :disabled="!ready || running || selectedOutputs.length === 0"
          type="button"
        >
          {{ running ? 'Bezig...' : 'Uitvoeren \u25B6' }}
        </button>
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

/* Select (scenario selector) */
.sb-select {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 6px;
  font-size: 13px;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
  background: white;
}

.sb-select:focus {
  outline: none;
  border-color: #154273;
}

.sb-select--scenario {
  margin-top: 6px;
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

/* Scenario data sources (loaded from feature file) */
.sb-scenario-source {
  padding: 8px 10px;
  margin-bottom: 6px;
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
  border-radius: 6px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sb-scenario-source-header {
  font-size: 12px;
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
  display: flex;
  align-items: center;
  gap: 6px;
}

.sb-scenario-source-fields {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 4px;
}

.sb-scenario-field {
  font-size: 10px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  padding: 1px 5px;
  background: white;
  border-radius: 3px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  color: var(--semantics-text-color-secondary, #666);
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

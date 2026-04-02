<script setup>
import { ref, computed, watch } from 'vue';
import { parseValue } from '../gherkin/steps.js';
import { formatValue, formatOutputValue, matchStatus as _matchStatus } from '../utils/outputFormat.js';
import DataSourceTable from './DataSourceTable.vue';

const props = defineProps({
  /** Scenario object from mapFeatureToForm() */
  scenario: { type: Object, required: true },
  /** Merged setup from getEffectiveSetup() */
  setup: { type: Object, required: true },
  /** WASM engine instance */
  engine: { type: Object, default: null },
  /** Engine ready state */
  ready: { type: Boolean, default: false },
  /** Law ID for execution */
  lawId: { type: String, required: true },
  /** Article mapping: { outputToArticle, inputToArticle, paramToArticle } */
  articleMap: { type: Object, default: null },
});

const emit = defineEmits(['show-details']);

// --- Form state (initialized from scenario setup) ---
const calculationDate = ref(props.setup.calculationDate || new Date().toISOString().slice(0, 10));

const parameterValues = ref(
  Object.fromEntries((props.setup.parameters || []).map((p) => [p.name, p.value ?? ''])),
);

// Convert scenario data sources to DataSourceTable format
function initDataSources() {
  return (props.setup.dataSources || []).map((ds) => ({
    sourceName: ds.sourceName,
    keyField: ds.keyField,
    fields: ds.headers.filter((h) => h !== ds.keyField).map((h) => ({ name: h, type: 'string' })),
    rows: ds.rows.map((row, i) => {
      const obj = { _id: i };
      ds.headers.forEach((h, j) => { obj[h] = row[j] ?? ''; });
      return obj;
    }),
  }));
}

const dataSources = ref(initDataSources());

// Expectations from scenario assertions
const expectations = ref(
  Object.fromEntries(
    (props.scenario.assertions || [])
      .filter((a) => a.outputName && a.value !== null && a.value !== undefined)
      .map((a) => [a.outputName, String(a.value)]),
  ),
);

// Output selection: default to outputs referenced in execution + assertions
const initOutputs = () => {
  const names = new Set();
  if (props.scenario.execution?.outputName) names.add(props.scenario.execution.outputName);
  for (const a of props.scenario.assertions || []) {
    if (a.outputName) names.add(a.outputName);
  }
  return [...names];
};
const selectedOutputs = ref(initOutputs());

// --- Execution state (per-scenario) ---
const result = ref(null);
const running = ref(false);
const error = ref(null);

// Re-init when scenario/setup changes
watch([() => props.setup, () => props.scenario], () => {
  calculationDate.value = props.setup.calculationDate || new Date().toISOString().slice(0, 10);
  parameterValues.value = Object.fromEntries(
    (props.setup.parameters || []).map((p) => [p.name, p.value ?? '']),
  );
  dataSources.value = initDataSources();
  expectations.value = Object.fromEntries(
    (props.scenario.assertions || [])
      .filter((a) => a.outputName && a.value !== null && a.value !== undefined)
      .map((a) => [a.outputName, String(a.value)]),
  );
  selectedOutputs.value = initOutputs();
  result.value = null;
  error.value = null;
}, { deep: true });

function execute() {
  if (!props.engine || !props.ready) return;

  const outputName = props.scenario.execution?.outputName || selectedOutputs.value[0];
  if (!outputName) return;

  running.value = true;
  result.value = null;
  error.value = null;

  try {
    const engine = props.engine;
    engine.clearDataSources();

    // Register data sources
    for (const ds of dataSources.value) {
      if (ds.rows.length === 0) continue;
      const typedRows = ds.rows.map((row) => {
        const typed = {};
        for (const [k, v] of Object.entries(row)) {
          if (k === '_id') continue;
          typed[k] = typeof v === 'string' ? parseValue(v) : v;
        }
        return typed;
      });
      engine.registerDataSource(ds.sourceName, ds.keyField, typedRows);
    }

    // Build parameters
    const params = {};
    for (const [k, v] of Object.entries(parameterValues.value)) {
      if (v !== '' && v !== null && v !== undefined) {
        params[k] = typeof v === 'string' ? parseValue(v) : v;
      }
    }

    const execResult = engine.executeWithTrace(
      props.lawId,
      outputName,
      params,
      calculationDate.value,
    );

    result.value = execResult;
  } catch (e) {
    if (e && typeof e === 'object' && e.error) {
      error.value = e.error;
    } else {
      const msg = typeof e === 'string' ? e : (e.message || String(e));
      error.value = msg;
    }
  } finally {
    running.value = false;
  }
}

/** Returns the current execution data for use by parent components */
function getExecutionData() {
  return {
    result: result.value,
    traceText: result.value?.trace_text || null,
    error: error.value,
    expectations: expectations.value,
  };
}

defineExpose({ execute, getExecutionData });

function updateDataSourceRows(index, rows) {
  const updated = [...dataSources.value];
  updated[index] = { ...updated[index], rows };
  dataSources.value = updated;
}

// --- Result formatting (delegates to shared utils) ---
function matchStatus(outputName, actualValue) {
  return _matchStatus(outputName, actualValue, expectations.value);
}

const hasExpectations = computed(() => Object.keys(expectations.value).length > 0);
</script>

<template>
  <div class="sf-form">
    <!-- Expected outputs at top -->
    <div v-if="hasExpectations" class="sf-expectations">
      <div
        v-for="(exp, name) in expectations"
        :key="name"
        class="sf-expectation"
        :class="result ? `sf-expectation--${matchStatus(name, result.outputs?.[name])}` : (error ? 'sf-expectation--failed' : '')"
      >
        <div class="sf-expectation-header">
          <span class="sf-expectation-name">{{ name }}</span>
          <span v-if="articleMap?.outputToArticle?.get(name)" class="sf-article-tag">
            Art. {{ articleMap.outputToArticle.get(name) }}
          </span>
        </div>
        <div class="sf-expectation-values">
          <span class="sf-expectation-expected">verwacht: {{ formatValue(exp) }}</span>
          <template v-if="result && result.outputs">
            <span class="sf-expectation-arrow">&rarr;</span>
            <span class="sf-expectation-actual">{{ formatOutputValue(result.outputs[name], name) }}</span>
          </template>
        </div>
      </div>
    </div>

    <!-- Error -->
    <div v-if="error && !running" class="sf-error">{{ error }}</div>

    <!-- Loading indicator -->
    <div v-if="running" class="sf-running">Uitvoeren...</div>

    <!-- Calculation date -->
    <div class="sf-row">
      <label class="sf-label">Datum</label>
      <input type="date" class="sf-date" v-model="calculationDate" />
    </div>

    <!-- Parameters -->
    <div v-for="(value, name) in parameterValues" :key="name" class="sf-row">
      <label class="sf-label">
        {{ name }}
        <span v-if="articleMap?.paramToArticle?.get(name)" class="sf-article-tag">
          Art. {{ articleMap.paramToArticle.get(name) }}
        </span>
      </label>
      <input
        class="sf-input"
        :value="value"
        @input="parameterValues = { ...parameterValues, [name]: $event.target.value }"
      />
    </div>

    <!-- Data sources -->
    <DataSourceTable
      v-for="(ds, i) in dataSources"
      :key="ds.sourceName"
      :title="ds.sourceName"
      :key-field="ds.keyField"
      :fields="ds.fields"
      :model-value="ds.rows"
      :default-expanded="true"
      @update:model-value="updateDataSourceRows(i, $event)"
    />

    <!-- Details button -->
    <div class="sf-actions-row">
      <button
        class="sf-details-btn"
        @click="emit('show-details')"
        :disabled="!result && !error"
        type="button"
      >
        Details &#x25B6;
      </button>
    </div>
  </div>
</template>

<style scoped>
.sf-form {
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

/* Expected outputs */
.sf-expectations {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 8px;
}

.sf-expectation {
  padding: 6px 10px;
  border-radius: 6px;
  border-left: 3px solid var(--semantics-dividers-color, #ccc);
  background: var(--semantics-surfaces-color-secondary, #f5f5f5);
  font-size: 12px;
}

.sf-expectation--passed {
  background: #e8f5e9;
  border-left-color: #2e7d32;
}

.sf-expectation--failed {
  background: #ffebee;
  border-left-color: #c62828;
}

.sf-expectation-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 2px;
}

.sf-expectation-name {
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sf-expectation-values {
  display: flex;
  align-items: center;
  gap: 6px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 12px;
  color: var(--semantics-text-color-secondary, #555);
}

.sf-expectation-arrow {
  color: #999;
}

.sf-expectation-actual {
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
}

/* Article tag */
.sf-article-tag {
  font-size: 10px;
  font-weight: 600;
  color: #666;
  background: #eee;
  padding: 1px 5px;
  border-radius: 3px;
}

.sf-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
}

.sf-label {
  font-size: 12px;
  font-weight: 600;
  min-width: 50px;
  color: var(--semantics-text-color-secondary, #666);
  display: flex;
  align-items: center;
  gap: 4px;
}

.sf-date {
  padding: 4px 6px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 4px;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.sf-input {
  flex: 1;
  padding: 4px 6px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 4px;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.sf-date:focus, .sf-input:focus {
  outline: none;
  border-color: #154273;
}

/* Actions row */
.sf-actions-row {
  padding: 8px 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.sf-details-btn {
  padding: 5px 14px;
  background: var(--semantics-surfaces-color-secondary, #f0f0f0);
  color: var(--semantics-text-color-primary, #1C2029);
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 5px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.sf-details-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.sf-details-btn:hover:not(:disabled) {
  background: #e0e0e0;
}

.sf-running {
  font-size: 12px;
  color: var(--semantics-text-color-secondary, #666);
  font-style: italic;
  padding: 4px 0;
}

.sf-error {
  font-size: 12px;
  color: #c00;
  word-break: break-word;
  padding: 4px 0;
}
</style>

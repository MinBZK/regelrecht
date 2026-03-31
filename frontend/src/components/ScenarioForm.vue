<script setup>
import { ref, computed, watch } from 'vue';
import { parseValue } from '../gherkin/steps.js';
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
});

const emit = defineEmits(['executed']);

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

// Re-init when scenario changes
watch(() => props.setup, () => {
  calculationDate.value = props.setup.calculationDate || new Date().toISOString().slice(0, 10);
  parameterValues.value = Object.fromEntries(
    (props.setup.parameters || []).map((p) => [p.name, p.value ?? '']),
  );
  dataSources.value = initDataSources();
}, { deep: true });

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
    emit('executed', {
      result: execResult,
      traceText: execResult.trace_text || null,
    });
  } catch (e) {
    if (e && typeof e === 'object' && e.error) {
      error.value = e.error;
      emit('executed', { result: null, traceText: e.trace_text || null });
    } else {
      error.value = typeof e === 'string' ? e : (e.message || String(e));
    }
  } finally {
    running.value = false;
  }
}

function updateDataSourceRows(index, rows) {
  const updated = [...dataSources.value];
  updated[index] = { ...updated[index], rows };
  dataSources.value = updated;
}

// --- Result formatting ---
function formatValue(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  return String(value);
}

function formatOutputValue(value, name) {
  const raw = formatValue(value);
  if (typeof value === 'number' && Number.isInteger(value) &&
      (name.includes('hoogte') || name.includes('bedrag') || name.includes('premie'))) {
    return `${raw} (${(value / 100).toFixed(2)} euro)`;
  }
  return raw;
}

function matchStatus(outputName, actualValue) {
  if (!(outputName in expectations.value)) return 'neutral';
  const expected = expectations.value[outputName];
  if (expected === null || expected === undefined) return 'neutral';
  const actual = normalize(actualValue);
  const exp = normalize(expected);
  if (actual === exp) return 'passed';
  if (typeof actual === 'number' && typeof exp === 'number' && Math.abs(actual - exp) < 1e-9) return 'passed';
  return 'failed';
}

function normalize(v) {
  if (v === 'true' || v === true) return true;
  if (v === 'false' || v === false) return false;
  if (v === 'null' || v === null) return null;
  if (typeof v === 'string' && /^-?\d+(\.\d+)?$/.test(v)) return Number(v);
  return v;
}
</script>

<template>
  <div class="sf-form">
    <!-- Calculation date -->
    <div class="sf-row">
      <label class="sf-label">Datum</label>
      <input type="date" class="sf-date" v-model="calculationDate" />
    </div>

    <!-- Parameters -->
    <div v-for="(value, name) in parameterValues" :key="name" class="sf-row">
      <label class="sf-label">{{ name }}</label>
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

    <!-- Execute + Results -->
    <div class="sf-execute-row">
      <button
        class="sf-execute-btn"
        @click="execute"
        :disabled="!ready || running"
        type="button"
      >
        {{ running ? 'Bezig...' : 'Uitvoeren \u25B6' }}
      </button>

      <!-- Inline results -->
      <template v-if="result && !running">
        <div
          v-for="(value, name) in result.outputs"
          :key="name"
          class="sf-result"
          :class="`sf-result--${matchStatus(name, value)}`"
        >
          <span class="sf-result-icon">
            <template v-if="matchStatus(name, value) === 'passed'">&#x2713;</template>
            <template v-else-if="matchStatus(name, value) === 'failed'">&#x2717;</template>
            <template v-else>&#x25CF;</template>
          </span>
          <span class="sf-result-name">{{ name }}:</span>
          <span class="sf-result-value">{{ formatOutputValue(value, name) }}</span>
          <span v-if="matchStatus(name, value) === 'passed'" class="sf-badge sf-badge--pass">GESLAAGD</span>
          <span v-if="matchStatus(name, value) === 'failed'" class="sf-badge sf-badge--fail">MISLUKT</span>
        </div>
      </template>

      <div v-if="error && !running" class="sf-error">{{ error }}</div>
    </div>
  </div>
</template>

<style scoped>
.sf-form {
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
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

.sf-execute-row {
  padding: 8px 0;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.sf-execute-btn {
  padding: 5px 14px;
  background: #154273;
  color: white;
  border: none;
  border-radius: 5px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.sf-execute-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.sf-execute-btn:hover:not(:disabled) {
  background: #1a5490;
}

.sf-result {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.sf-result-icon {
  font-weight: bold;
}

.sf-result--passed .sf-result-icon { color: #060; }
.sf-result--failed .sf-result-icon { color: #c00; }
.sf-result--neutral .sf-result-icon { color: #666; }

.sf-result-name {
  font-weight: 600;
}

.sf-result-value {
  color: var(--semantics-text-color-secondary, #555);
}

.sf-badge {
  font-size: 9px;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 3px;
}

.sf-badge--pass { background: #efe; color: #060; }
.sf-badge--fail { background: #fee; color: #c00; }

.sf-error {
  font-size: 12px;
  color: #c00;
  word-break: break-word;
}
</style>

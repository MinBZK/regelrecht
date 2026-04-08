<script setup>
import { ref, computed, watch } from 'vue';
import { parseValue } from '../gherkin/steps.js';
import { formatValue, formatOutputValue, normalizeForCompare, matchStatus as _matchStatus } from '../utils/outputFormat.js';
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

const emit = defineEmits(['show-details', 'executed']);

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
const errorTraceText = ref(null);

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
  errorTraceText.value = null;
}, { deep: true });

function execute() {
  if (!props.engine || !props.ready) return;

  const outputName = props.scenario.execution?.outputName || selectedOutputs.value[0];
  if (!outputName) return;

  running.value = true;
  result.value = null;
  error.value = null;
  errorTraceText.value = null;

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
      errorTraceText.value = e.trace_text || null;
    } else {
      const msg = typeof e === 'string' ? e : (e.message || String(e));
      error.value = msg;
    }
  } finally {
    running.value = false;
    emit('executed', getExecutionData());
  }
}

/** Returns the current execution data for use by parent components */
function getExecutionData() {
  return {
    result: result.value,
    traceText: result.value?.trace_text || errorTraceText.value || null,
    error: error.value,
    expectations: expectations.value,
  };
}

defineExpose({ execute, getExecutionData });

// --- Auto-re-execute when input values change ---
let executeTimer = null;
watch(
  [parameterValues, calculationDate, dataSources],
  () => {
    if (!props.engine || !props.ready) return;
    clearTimeout(executeTimer);
    executeTimer = setTimeout(() => execute(), 300);
  },
  { deep: true },
);

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
    <ndd-list v-if="hasExpectations" variant="simple">
      <ndd-list-item
        v-for="(exp, name) in expectations"
        :key="name"
        size="sm"
        class="sf-expectation"
        :class="result ? `sf-expectation--${matchStatus(name, result.outputs?.[name])}` : (error ? 'sf-expectation--failed' : '')"
      >
        <ndd-text-cell size="sm" :text="articleMap?.outputToArticle?.get(name) ? `${name} (Art. ${articleMap.outputToArticle.get(name)})` : name"></ndd-text-cell>
        <ndd-text-cell size="sm" class="sf-expectation-value" :text="`verwacht: ${formatValue(normalizeForCompare(exp))}`"></ndd-text-cell>
        <ndd-text-cell v-if="result && result.outputs" size="sm" class="sf-expectation-actual" :text="`→ ${formatOutputValue(result.outputs[name], name)}`"></ndd-text-cell>
      </ndd-list-item>
    </ndd-list>

    <!-- Error -->
    <div v-if="error && !running" class="sf-error">{{ error }}</div>

    <!-- Loading indicator -->
    <div v-if="running" class="sf-running">Uitvoeren...</div>

    <!-- Input: date + parameters -->
    <ndd-list variant="simple" class="sf-input-list">
      <ndd-list-item size="sm">
        <ndd-text-cell size="sm" text="Datum"></ndd-text-cell>
        <ndd-cell>
          <input type="date" class="sf-date" v-model="calculationDate" />
        </ndd-cell>
      </ndd-list-item>
      <ndd-list-item v-for="(value, name) in parameterValues" :key="name" size="sm">
        <ndd-text-cell size="sm" :text="articleMap?.paramToArticle?.get(name) ? `${name} (Art. ${articleMap.paramToArticle.get(name)})` : name"></ndd-text-cell>
        <ndd-cell>
          <input
            class="sf-input"
            :value="value"
            @input="parameterValues = { ...parameterValues, [name]: $event.target.value }"
          />
        </ndd-cell>
      </ndd-list-item>
    </ndd-list>

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
      <ndd-button
        :disabled="!result && !error || undefined"
        @click="emit('show-details')"
        text="Details" end-icon="chevron-right"
      ></ndd-button>
    </div>
  </div>
</template>

<style scoped>
.sf-form {
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
}

/* Expected outputs */
.sf-expectation {
  border-left: 3px solid transparent;
}

.sf-expectation--passed {
  border-left-color: #2e7d32;
}

.sf-expectation--failed {
  border-left-color: #c62828;
}

.sf-expectation-value {
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: var(--semantics-text-color-secondary, #555);
}

.sf-expectation-actual {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
}

/* Input list */
.sf-input-list ndd-text-cell {
  width: 80px;
  min-width: 80px;
  flex-shrink: 0;
}

.sf-input-list ndd-cell {
  flex: 1;
  min-width: 0;
}

.sf-date {
  padding: 4px 6px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 4px;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.sf-input {
  width: 100%;
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

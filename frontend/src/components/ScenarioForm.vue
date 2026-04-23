<script setup>
import { ref, computed, watch, onBeforeUnmount } from 'vue';
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

const emit = defineEmits(['show-details', 'executed', 'change']);

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
    // Always emit so the parent's result panel reflects the latest state,
    // including the error path: getExecutionData() returns the error and
    // any partial trace, which the parent renders instead of stale data
    // from a previous successful run.
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

/** Returns the current form input values for syncing back to formState.
 *  All collections are returned as fresh shallow copies so a caller cannot
 *  accidentally mutate the component's reactive form state. */
function getFormValues() {
  return {
    parameterValues: { ...parameterValues.value },
    calculationDate: calculationDate.value,
    dataSources: [...dataSources.value],
  };
}

defineExpose({ execute, getExecutionData, getFormValues });

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

onBeforeUnmount(() => {
  clearTimeout(executeTimer);
});

function updateDataSourceRows(index, rows) {
  const updated = [...dataSources.value];
  updated[index] = { ...updated[index], rows };
  dataSources.value = updated;
  emit('change');
}

// --- Result formatting (delegates to shared utils) ---
function matchStatus(outputName, actualValue) {
  return _matchStatus(outputName, actualValue, expectations.value);
}

const hasExpectations = computed(() => Object.keys(expectations.value).length > 0);

const overallStatus = computed(() => {
  if (!result.value) return null;
  const keys = Object.keys(expectations.value);
  if (keys.length === 0) return null;
  for (const name of keys) {
    if (matchStatus(name, result.value.outputs?.[name]) === 'failed') return 'failed';
  }
  return 'passed';
});
</script>

<template>
  <div class="sf-form">
    <!-- Expected outputs at top -->
    <template v-if="hasExpectations">
      <nldd-title size="5" class="sf-section-title"><span>Verwachte uitkomsten</span></nldd-title>
      <div
        class="sf-expectations-block"
        :class="result ? `sf-expectations-block--${overallStatus}` : (error ? 'sf-expectations-block--failed' : '')"
      >
        <div
          v-for="(exp, name) in expectations"
          :key="name"
          class="sf-expectation-item"
        >
          <span class="sf-expectation-name">{{ name }}</span>
          <span v-if="articleMap?.outputToArticle?.get(name)" class="sf-article-tag">Art. {{ articleMap.outputToArticle.get(name) }}</span>
          <span class="sf-expectation-detail">
            <span>{{ formatValue(normalizeForCompare(exp)) }}</span>
            <template v-if="result && result.outputs">
              <span class="sf-expectation-arrow">&rarr;</span>
              <span>{{ formatOutputValue(result.outputs[name], name) }}</span>
            </template>
          </span>
        </div>
      </div>
    </template>

    <!-- Error -->
    <div v-if="error && !running" class="sf-error">{{ error }}</div>

    <!-- Loading indicator -->
    <div v-if="running" class="sf-running">Uitvoeren...</div>

    <!-- Input: date + parameters -->
    <nldd-title size="5" class="sf-section-title"><span>Invoer</span></nldd-title>
    <nldd-list variant="box" class="sf-input-list">
      <nldd-list-item size="md">
        <nldd-text-cell text="Datum" max-width="140"></nldd-text-cell>
        <nldd-cell>
          <nldd-text-field size="md" type="date" :value="calculationDate" @input="calculationDate = $event.target?.value ?? $event.detail?.value ?? calculationDate; emit('change')"></nldd-text-field>
        </nldd-cell>
      </nldd-list-item>
      <nldd-list-item v-for="(value, name) in parameterValues" :key="name" size="md">
        <nldd-text-cell :text="articleMap?.paramToArticle?.get(name) ? `${name} (Art. ${articleMap.paramToArticle.get(name)})` : name" max-width="140"></nldd-text-cell>
        <nldd-cell>
          <nldd-text-field
            size="md"
            :value="value"
            :placeholder="name"
            @input="parameterValues = { ...parameterValues, [name]: $event.target?.value ?? $event.detail?.value ?? '' }; emit('change')"
          ></nldd-text-field>
        </nldd-cell>
      </nldd-list-item>
    </nldd-list>

    <!-- Data sources -->
    <DataSourceTable
      v-for="(ds, i) in dataSources"
      :key="ds.sourceName"
      :title="ds.sourceName"
      :key-field="ds.keyField"
      :fields="ds.fields"
      :model-value="ds.rows"
      :default-expanded="false"
      @update:model-value="updateDataSourceRows(i, $event)"
    />

    <!-- Details button -->
    <div class="sf-actions-row">
      <nldd-button
        :disabled="!result && !error || undefined"
        @click="emit('show-details')"
        text="Toon resultaat" end-icon="chevron-right"
      ></nldd-button>
    </div>
  </div>
</template>

<style scoped>
.sf-form {
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
}

/* Section titles — extra top margin separates each block visually.
 * The first section title doesn't need top margin (handled by :first-child). */
.sf-section-title {
  margin-top: 16px;
  margin-bottom: 4px;
}
.sf-section-title:first-child {
  margin-top: 0;
}

/* Expected outputs — single continuous block with left border */
.sf-expectations-block {
  border-left: 3px solid transparent;
  padding: 8px 10px;
  border-radius: 4px;
  background: var(--semantics-surfaces-tinted-background-color, #f5f5f5);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.sf-expectations-block--passed {
  border-left-color: #2e7d32;
}

.sf-expectations-block--failed {
  border-left-color: #c62828;
}

.sf-expectation-item {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px;
  font-size: 13px;
}

.sf-expectation-name {
  font-weight: 400;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sf-article-tag {
  font-size: 10px;
  font-weight: 600;
  color: #666;
  background: rgba(0, 0, 0, 0.06);
  padding: 1px 5px;
  border-radius: 3px;
}

.sf-expectation-detail {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 13px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sf-expectation-arrow {
  flex-shrink: 0;
  color: var(--semantics-text-color-secondary, #666);
}

/* DataSourceTable blocks get the same top spacing as section titles */
.sf-form :deep(.ds-block) {
  margin-top: 16px;
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

<style>
/* Unscoped: nldd web components need global selectors */
.sf-input-list nldd-text-cell {
  width: 140px;
  min-width: 140px;
  flex-shrink: 0;
}

.sf-input-list nldd-cell {
  flex: 1;
  min-width: 0;
}

.sf-input-list nldd-text-field {
  width: 100%;
}
</style>

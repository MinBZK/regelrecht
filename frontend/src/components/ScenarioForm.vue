<script setup>
import { ref, computed, watch, onBeforeUnmount } from 'vue';
import { parseValue } from '../gherkin/steps.js';
import { formatValue, formatOutputValue, normalizeForCompare, matchStatus as _matchStatus, humanize } from '../utils/outputFormat.js';
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

const emit = defineEmits(['show-details', 'executed', 'change', 'drill-change']);

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

// Drill-in navigation: null = scenario overview, otherwise the index of the
// data source whose table is shown one level deeper in the sheet. The back
// affordance lives in the sheet's top-title-bar (a single level deep, so the
// ActionSheet-style breadcrumb rows would be overkill), so the parent needs
// to know the drilled source name and be able to pop back out.
const selectedSource = ref(null);

function clearDrill() {
  selectedSource.value = null;
}

// Data-source names render human-readable AND sentence-cased ("personal_data"
// → "Personal data"). humanize() only de-snakes; it doesn't capitalise.
function sourceLabel(name) {
  const h = humanize(name);
  return h ? h.charAt(0).toUpperCase() + h.slice(1) : h;
}

watch(selectedSource, (idx) => {
  emit('drill-change', idx == null ? null : (dataSources.value[idx]?.sourceName ?? null));
});

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
  selectedSource.value = null;
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
  // Return the data so callers can synchronously read the post-execution
  // result without relying on Vue reactivity timing.
  return getExecutionData();
}

/** Returns the current execution data for use by parent components */
function getExecutionData() {
  return {
    result: result.value,
    traceText: result.value?.trace_text || errorTraceText.value || null,
    error: error.value,
    expectations: expectations.value,
    // Expose the scenario's entry output so the graph view can mark the
    // starting leaf (see useTraceStepping.startNodeIds).
    outputName: props.scenario.execution?.outputName || selectedOutputs.value[0] || null,
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

defineExpose({ execute, getExecutionData, getFormValues, clearDrill });

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
</script>

<template>
  <div>
    <!-- Scenario overview -->
    <template v-if="selectedSource === null">
      <!-- Expected outputs -->
      <template v-if="hasExpectations">
        <nldd-title size="5"><h2>Verwachte uitkomsten</h2></nldd-title>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-list variant="box">
          <nldd-list-item v-for="(exp, name) in expectations" :key="name" size="md">
            <nldd-text-cell size="md" :text="humanize(name)"></nldd-text-cell>
            <nldd-text-cell
              size="md"
              horizontal-alignment="right"
              :text="humanize(formatValue(normalizeForCompare(exp)))"
            ></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-button
          :disabled="!result && !error || undefined"
          @click="emit('show-details')"
          text="Resultaat"
        ></nldd-button>
      </template>

      <!-- Error -->
      <div v-if="error && !running" class="sf-error">{{ error }}</div>

      <!-- Loading indicator -->
      <div v-if="running" class="sf-running">Uitvoeren...</div>

      <!-- Input: date + parameters -->
      <nldd-spacer v-if="hasExpectations" size="16"></nldd-spacer>
      <nldd-title size="5"><h2>Invoer</h2></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-list variant="box" class="sf-input-list">
        <nldd-list-item size="md">
          <nldd-text-cell text="Datum" max-width="280px"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell>
            <nldd-text-field size="md" type="date" :value="calculationDate" @input="calculationDate = $event.target?.value ?? $event.detail?.value ?? calculationDate; emit('change')"></nldd-text-field>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-for="(value, name) in parameterValues" :key="name" size="md">
          <nldd-text-cell :text="articleMap?.paramToArticle?.get(name) ? `${name} (Art. ${articleMap.paramToArticle.get(name)})` : name" max-width="280px"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
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

      <!-- Data sources: a row per source, drill in one level deeper -->
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="5"><h2>Gegevensbronnen</h2></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item
          v-for="(ds, i) in dataSources"
          :key="ds.sourceName"
          size="md"
          type="button"
          :data-testid="`ds-row-${i}`"
          @click="selectedSource = i"
        >
          <nldd-text-cell :text="sourceLabel(ds.sourceName)"></nldd-text-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-text-cell horizontal-alignment="right" :text="ds.rows.length ? String(ds.rows.length) : ''"></nldd-text-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- One level deeper: a single data source's table. Back to the scenario
         overview is the sheet's top-title-bar back button (driven by the
         parent via clearDrill / drill-change). -->
    <template v-else>
      <DataSourceTable
        :key="dataSources[selectedSource].sourceName"
        :title="sourceLabel(dataSources[selectedSource].sourceName)"
        :subtitle="scenario.name"
        :key-field="dataSources[selectedSource].keyField"
        :fields="dataSources[selectedSource].fields"
        :model-value="dataSources[selectedSource].rows"
        :drilled-in="true"
        anchor-id="ds-drill-anchor"
        @update:model-value="updateDataSourceRows(selectedSource, $event)"
      />
    </template>

  </div>
</template>

<style scoped>
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
  min-width: 140px;
  flex: 1;
}

.sf-input-list nldd-cell {
  flex: 1;
  min-width: 0;
  max-width: 280px;
}

.sf-input-list nldd-text-field {
  width: 100%;
}
</style>

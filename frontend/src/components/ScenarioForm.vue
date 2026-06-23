<script setup>
import { ref, computed, watch, onBeforeUnmount, useId } from 'vue';
import { parseValue } from '../gherkin/steps.js';
import { formatValue, formatOutputValue, normalizeForCompare, matchStatus as _matchStatus, humanize } from '../utils/outputFormat.js';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';

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
  /** Datatype mapping from buildTypeMap(): name -> { type, unit } */
  typeMap: { type: Object, default: null },
  /** External data-source field types from buildExternalFieldTypeMap(): name -> { type, unit } */
  externalFieldTypeMap: { type: Object, default: null },
});

// Resolve a parameter's declared datatype/unit; default to a plain text field
// for params not found in the map (background-only params, articles without
// machine_readable).
function paramMeta(name) {
  return props.typeMap?.get(name) ?? { type: 'string', unit: null };
}

// Resolve an external data-source column's datatype/unit from the dependency
// graph; default to a plain text field for columns not found in the map.
function typeField(name) {
  const meta = props.externalFieldTypeMap?.get(name);
  return { name, type: meta?.type ?? 'string', unit: meta?.unit ?? null };
}

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
    fields: ds.headers.filter((h) => h !== ds.keyField).map((h) => typeField(h)),
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

// Re-initialise every local input/result ref from the (read-only) props.
// All edits live in these refs and are only written back to formState on
// save, so re-initialising here fully discards unsaved edits. Used both
// when the scenario/setup props change and when the parent discards edits
// (cancel / click-away) without replacing formState — see ScenarioBuilder.
function discardEdits() {
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
  // Wiping the local result is safe even on a cancel-with-edits: the
  // builder keeps the last run in its scenarioResults map, and
  // onShowDetails() falls back to that when getExecutionData() returns
  // no fresh result (hasFresh === false). So the result sheet keeps
  // showing the previous outcome instead of going blank.
  result.value = null;
  error.value = null;
  errorTraceText.value = null;
}

// Re-init when scenario/setup changes
watch([() => props.setup, () => props.scenario], discardEdits, { deep: true });

// Column types come from the dependency graph, which loads asynchronously after
// the panel mounts. Re-type the data-source columns in place when the map
// arrives — without rebuilding rows, so unsaved cell edits survive.
watch(() => props.externalFieldTypeMap, () => {
  for (const ds of dataSources.value) {
    ds.fields = ds.fields.map((f) => typeField(f.name));
  }
});

// CONTRACT: execute() must stay fully synchronous. The WASM engine runs
// in-process with no I/O, so `result`/`error`/`running` are all settled
// (running reset in finally) before this returns, and callers read them
// back immediately from the return value or getExecutionData() — e.g.
// ScenarioBuilder.reExecute() / onSaveAndShow() and the auto-execute
// loop. Introducing any await/timer/microtask here would make those
// callers observe `running: true` and stale data. If async execution is
// ever needed, return a Promise and update every caller to await it.
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
    running: running.value,
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

defineExpose({ execute, getExecutionData, getFormValues, clearDrill, discardEdits });

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

// Minimal field-level validation: a missing date, or an empty input that
// caused an execution error, is marked invalid so the user sees which
// field to fill (the raw engine message still shows as context). No
// auto-revert — the input stays. We deliberately do NOT map engine error
// strings onto fields (incl. substring-matching the error against a param
// name): that produced confusing cross-field invalid states (changing bsn
// flagged loon_uit_dienstbetrekking) and was explicitly reverted. The
// broad "any error + this field empty" mark is the accepted trade — it
// can over-mark a legitimately-blank optional param, but it never
// mis-points at an unrelated field, which was the worse failure.
const dateInvalid = computed(() => !calculationDate.value);
// Unique id so the inline message can be aria-associated with the field
// (ScenarioForm is mounted once per scenario, so a static id would clash).
const dateErrorId = useId();
</script>

<template>
  <div class="sf-root">
    <!-- Scenario overview -->
    <template v-if="selectedSource === null">
      <!-- Expected outputs -->
      <template v-if="hasExpectations">
        <nldd-title size="5"><h2>Verwachte uitkomsten</h2></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-list variant="box">
          <nldd-list-item v-for="(exp, name) in expectations" :key="name" size="md">
            <nldd-text-cell size="md" :text="humanize(name)"></nldd-text-cell>
            <nldd-text-cell
              size="md"
              horizontal-alignment="right"
              :text="humanize(formatValue(normalizeForCompare(exp)))"
            ></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="md">
            <nldd-cell width="full">
              <nldd-button
                size="md"
                width="full"
                @click="emit('show-details')"
                text="Bekijk resultaat"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
      </template>

      <!-- Error -->
      <nldd-banner v-if="error && !running" variant="critical" :text="error"></nldd-banner>

      <!-- Loading indicator -->
      <div v-if="running" class="sf-running">Uitvoeren...</div>

      <!-- Input: date + parameters -->
      <nldd-spacer v-if="hasExpectations" size="16"></nldd-spacer>
      <nldd-title size="5"><h2>Invoer</h2></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item size="md">
          <nldd-text-cell text="Datum" min-width="120px" max-width="200px"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell width="full" min-width="120px">
            <ScenarioParameterInput
              type="date"
              name="Datum"
              :value="calculationDate"
              :invalid="dateInvalid"
              :error-message-ids="dateErrorId"
              @update="calculationDate = $event; emit('change')"
            />
            <span v-if="dateInvalid" :id="dateErrorId" class="sf-error">Datum is verplicht</span>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-for="(value, name) in parameterValues" :key="name" size="md">
          <nldd-text-cell :text="name" :supporting-text="articleMap?.paramToArticle?.get(name) ? `Artikel ${articleMap.paramToArticle.get(name)}` : undefined" min-width="120px" max-width="200px"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell width="full" min-width="120px">
            <ScenarioParameterInput
              :type="paramMeta(name).type"
              :unit="paramMeta(name).unit"
              :name="name"
              :value="value"
              :invalid="!!error && (value === '' || value == null)"
              @update="parameterValues = { ...parameterValues, [name]: $event }; emit('change')"
            />
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>

      <!-- Data sources: a row per source, drill in one level deeper -->
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="5"><h2>Bronnen</h2></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="box" arrow-navigation>
        <nldd-list-item
          v-for="(ds, i) in dataSources"
          :key="ds.sourceName"
          size="md"
          button
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
        :key-field="dataSources[selectedSource].keyField"
        :fields="dataSources[selectedSource].fields"
        :model-value="dataSources[selectedSource].rows"
        :drilled-in="true"
        @update:model-value="updateDataSourceRows(selectedSource, $event)"
      />
    </template>

  </div>
</template>

<style scoped>
/* Single component root required for v-show, but it must not generate a
 * box — otherwise it blocks the enclosing simple-section's nldd flex
 * layout (flex-grow / centering of an empty data source's dialog). */
.sf-root {
  display: contents;
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

<script setup>
import { ref, computed } from 'vue';
import { getEffectiveSetup } from '../gherkin/formMapper.js';
import DataSourceTable from './DataSourceTable.vue';

const props = defineProps({
  formState: { type: Object, required: true },
  results: { type: Array, default: null },
  readonly: { type: Boolean, default: true },
});

// Track which scenario cards are expanded (all expanded by default)
const expandedScenarios = ref(new Set(
  props.formState.scenarios.map((_, i) => i),
));

function toggleScenario(index) {
  const s = new Set(expandedScenarios.value);
  if (s.has(index)) {
    s.delete(index);
  } else {
    s.add(index);
  }
  expandedScenarios.value = s;
}

function scenarioResult(index) {
  if (!props.results || !props.results[index]) return null;
  return props.results[index];
}

function scenarioStatus(index) {
  const result = scenarioResult(index);
  if (!result) return 'neutral';
  return result.status;
}

// Map assertion result from step results
function assertionResult(scenarioIndex, assertionIndex) {
  const result = scenarioResult(scenarioIndex);
  if (!result) return null;

  const scenario = props.formState.scenarios[scenarioIndex];
  const effective = getEffectiveSetup(props.formState, scenarioIndex);

  let setupStepCount = 0;
  if (effective.calculationDate) setupStepCount++;
  setupStepCount += effective.dependencies.length;
  setupStepCount += effective.parameters.length;
  setupStepCount += effective.dataSources.length;
  if (scenario.execution) setupStepCount++;

  const bgUnmatched = props.formState.background?.unmatchedSteps?.length || 0;
  setupStepCount += bgUnmatched;
  setupStepCount += scenario.unmatchedSteps?.length || 0;

  const stepIndex = setupStepCount + assertionIndex;
  if (result.steps && result.steps[stepIndex]) {
    return result.steps[stepIndex];
  }
  return null;
}

function formatAssertionLabel(assertion) {
  switch (assertion.assertionType) {
    case 'succeeds': return 'Uitvoering slaagt';
    case 'fails': return 'Uitvoering mislukt';
    case 'failsWith': return `Fout: "${assertion.value}"`;
    case 'boolean': return `${assertion.outputName} = ${assertion.value ? 'ja' : 'nee'}`;
    case 'equals': return `${assertion.outputName} = ${assertion.value}`;
    case 'equalsString': return `${assertion.outputName} = "${assertion.value}"`;
    case 'null': return `${assertion.outputName} = null`;
    case 'contains': return `${assertion.outputName} bevat "${assertion.value}"`;
    default: return `${assertion.assertionType}`;
  }
}

function stepIcon(status) {
  if (!status) return '\u25CB';
  switch (status) {
    case 'passed': return '\u2713';
    case 'failed': return '\u2717';
    case 'skipped': return '\u2014';
    case 'undefined': return '?';
    default: return '\u25CB';
  }
}

// Convert data source from formMapper format to DataSourceTable format
function dataSourceToTableProps(ds) {
  const fields = ds.headers.map((h) => ({ name: h, type: 'string' }));
  const rows = ds.rows.map((row, i) => {
    const obj = { _id: i };
    ds.headers.forEach((h, j) => {
      obj[h] = row[j] ?? '';
    });
    return obj;
  });
  return { fields, rows, keyField: ds.keyField };
}
</script>

<template>
  <div class="sv-container">
    <!-- Background (compact inline) -->
    <div v-if="formState.background" class="sv-bg">
      <span v-if="formState.background.calculationDate" class="sv-bg-date">{{ formState.background.calculationDate }}</span>
      <span
        v-for="dep in formState.background.dependencies"
        :key="dep"
        class="sv-dep-pill"
      >{{ dep }}</span>
    </div>

    <!-- Scenarios -->
    <div
      v-for="(scenario, si) in formState.scenarios"
      :key="si"
      class="sv-scenario"
      :class="{
        'sv-scenario--passed': scenarioStatus(si) === 'passed',
        'sv-scenario--failed': scenarioStatus(si) === 'failed',
      }"
    >
      <!-- Scenario header -->
      <button class="sv-scenario-header" @click="toggleScenario(si)" type="button">
        <span class="sv-toggle">{{ expandedScenarios.has(si) ? '\u25BE' : '\u25B8' }}</span>
        <span class="sv-scenario-name">{{ scenario.name }}</span>
        <span
          v-if="scenarioResult(si)"
          class="sv-badge"
          :class="`sv-badge--${scenarioStatus(si)}`"
        >{{ scenarioStatus(si) === 'passed' ? 'OK' : 'FAIL' }}</span>
      </button>

      <!-- Scenario body -->
      <div v-if="expandedScenarios.has(si)" class="sv-body">
        <!-- Compact setup: date + params on one line each -->
        <div class="sv-setup">
          <div
            v-if="scenario.setup.calculationDate || formState.background?.calculationDate"
            class="sv-kv"
          >
            <span class="sv-k">Datum</span>
            <span class="sv-v">{{ scenario.setup.calculationDate || formState.background?.calculationDate }}</span>
          </div>
          <div v-for="param in scenario.setup.parameters" :key="param.name" class="sv-kv">
            <span class="sv-k">{{ param.name }}</span>
            <span class="sv-v">{{ param.value }}</span>
          </div>
          <div v-if="scenario.setup.dependencies.length > 0" class="sv-kv">
            <span class="sv-k">Deps</span>
            <span class="sv-deps-inline">
              <span v-for="dep in scenario.setup.dependencies" :key="dep" class="sv-dep-pill">{{ dep }}</span>
            </span>
          </div>
        </div>

        <!-- Data sources (collapsed by default for compactness) -->
        <div v-if="scenario.setup.dataSources.length > 0" class="sv-ds-section">
          <DataSourceTable
            v-for="(ds, di) in scenario.setup.dataSources"
            :key="di"
            :title="ds.sourceName"
            :key-field="dataSourceToTableProps(ds).keyField"
            :fields="dataSourceToTableProps(ds).fields"
            :model-value="dataSourceToTableProps(ds).rows"
            :default-expanded="false"
            :readonly="readonly"
          />
        </div>

        <!-- Execution -->
        <div v-if="scenario.execution" class="sv-exec">
          Evalueer <code>{{ scenario.execution.outputName }}</code> van <code>{{ scenario.execution.lawId }}</code>
        </div>

        <!-- Assertions -->
        <div v-if="scenario.assertions.length > 0" class="sv-assertions">
          <div
            v-for="(assertion, ai) in scenario.assertions"
            :key="ai"
            class="sv-assert"
            :class="assertionResult(si, ai) ? `sv-assert--${assertionResult(si, ai).status}` : ''"
          >
            <span class="sv-assert-icon">{{ stepIcon(assertionResult(si, ai)?.status) }}</span>
            <span class="sv-assert-text">{{ formatAssertionLabel(assertion) }}</span>
            <span v-if="assertionResult(si, ai)?.status === 'passed'" class="sv-badge sv-badge--passed">OK</span>
            <span v-if="assertionResult(si, ai)?.status === 'failed'" class="sv-badge sv-badge--failed">FAIL</span>
          </div>
          <div
            v-for="(assertion, ai) in scenario.assertions"
            :key="`err-${ai}`"
          >
            <div v-if="assertionResult(si, ai)?.error" class="sv-assert-error">
              {{ assertionResult(si, ai).error }}
            </div>
          </div>
        </div>

        <!-- Unmatched steps -->
        <div v-if="scenario.unmatchedSteps.length > 0" class="sv-unmatched-section">
          <div v-for="(step, i) in scenario.unmatchedSteps" :key="i" class="sv-unmatched">
            <span class="sv-unmatched-kw">{{ step.keyword }}</span> {{ step.text }}
          </div>
        </div>

        <!-- Engine trace (step-by-step execution log) -->
        <div v-if="scenarioResult(si)" class="sv-trace">
          <button class="sv-trace-toggle" @click="$event.target.closest('.sv-trace').classList.toggle('sv-trace--open')" type="button">
            Engine trace ({{ scenarioResult(si).steps.length }} stappen)
          </button>
          <div class="sv-trace-body">
            <div
              v-for="(step, i) in scenarioResult(si).steps"
              :key="i"
              class="sv-trace-step"
              :class="`sv-trace-step--${step.status}`"
            >
              <span class="sv-trace-icon">{{ stepIcon(step.status) }}</span>
              <span class="sv-trace-text">{{ step.text }}</span>
              <div v-if="step.error" class="sv-trace-err">{{ step.error }}</div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Empty state -->
    <div v-if="formState.scenarios.length === 0" class="sv-empty">
      Geen scenario's gevonden.
    </div>
  </div>
</template>

<style scoped>
.sv-container {
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  font-size: 12px;
}

/* Background bar */
.sv-bg {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  padding: 6px 10px;
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sv-bg-date {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 11px;
  font-weight: 600;
  margin-right: 4px;
}

/* Dependency pills */
.sv-dep-pill {
  display: inline-block;
  padding: 1px 6px;
  font-size: 10px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  background: #e8eef5;
  color: #154273;
  border-radius: 3px;
  white-space: nowrap;
}

/* Scenario card */
.sv-scenario {
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sv-scenario--passed { background: #fcfff8; }
.sv-scenario--failed { background: #fffcf8; }

.sv-scenario-header {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 6px 10px;
  background: none;
  border: none;
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  text-align: left;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sv-scenario-header:hover {
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
}

.sv-toggle {
  flex-shrink: 0;
  width: 10px;
  font-size: 10px;
  color: #999;
}

.sv-scenario-name { flex: 1; }

/* Badges */
.sv-badge {
  font-size: 9px;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 3px;
}

.sv-badge--passed { background: #efe; color: #060; }
.sv-badge--failed { background: #fee; color: #c00; }

/* Body */
.sv-body {
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

/* Setup key-value pairs */
.sv-setup {
  padding: 4px 10px;
}

.sv-kv {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 1px 0;
}

.sv-k {
  font-size: 11px;
  font-weight: 600;
  color: #888;
  min-width: 50px;
  flex-shrink: 0;
}

.sv-v {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 11px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sv-deps-inline {
  display: flex;
  flex-wrap: wrap;
  gap: 3px;
}

/* Data sources */
.sv-ds-section {
  padding: 2px 10px 4px;
}

/* Execution */
.sv-exec {
  padding: 4px 10px;
  font-size: 11px;
  color: #555;
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sv-exec code {
  font-family: 'SF Mono', 'Fira Code', monospace;
  background: var(--semantics-surfaces-color-secondary, #F0F1F3);
  padding: 0 3px;
  border-radius: 2px;
  font-size: 11px;
}

/* Assertions */
.sv-assertions {
  padding: 4px 10px;
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sv-assert {
  display: flex;
  align-items: baseline;
  gap: 5px;
  padding: 1px 0;
}

.sv-assert-icon {
  flex-shrink: 0;
  width: 12px;
  text-align: center;
  font-weight: bold;
  font-size: 11px;
}

.sv-assert--passed .sv-assert-icon { color: #060; }
.sv-assert--failed .sv-assert-icon { color: #c00; }
.sv-assert--skipped .sv-assert-icon { color: #999; }

.sv-assert-text {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 11px;
}

.sv-assert-error {
  padding: 2px 0 2px 17px;
  font-size: 10px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: #c00;
  word-break: break-word;
}

/* Unmatched steps */
.sv-unmatched-section {
  padding: 4px 10px;
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sv-unmatched {
  padding: 1px 0;
  font-size: 11px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: #888;
}

.sv-unmatched-kw {
  font-weight: 700;
  color: #154273;
}

/* Engine trace — collapsed by default */
.sv-trace {
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sv-trace-toggle {
  display: block;
  width: 100%;
  padding: 4px 10px;
  font-size: 10px;
  font-weight: 600;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  color: #888;
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
  border: none;
  cursor: pointer;
  text-align: left;
}

.sv-trace-toggle:hover {
  color: #154273;
}

.sv-trace-body {
  display: none;
  padding: 4px 10px 6px;
  background: #fafbfc;
}

.sv-trace--open .sv-trace-body {
  display: block;
}

.sv-trace-step {
  display: flex;
  align-items: baseline;
  gap: 5px;
  padding: 1px 0;
  font-size: 10px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.sv-trace-icon {
  flex-shrink: 0;
  width: 10px;
  text-align: center;
  font-weight: bold;
}

.sv-trace-step--passed .sv-trace-icon { color: #060; }
.sv-trace-step--failed .sv-trace-icon { color: #c00; }
.sv-trace-step--skipped .sv-trace-icon { color: #999; }
.sv-trace-step--undefined .sv-trace-icon { color: #f80; }

.sv-trace-step--skipped { color: #999; }

.sv-trace-err {
  margin-left: 15px;
  color: #c00;
  font-size: 10px;
  word-break: break-word;
}

/* Empty state */
.sv-empty {
  padding: 16px;
  text-align: center;
  font-size: 12px;
  color: #999;
  font-style: italic;
}
</style>

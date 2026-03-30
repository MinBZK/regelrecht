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

  // Find the assertion steps in the results (they're the Then steps)
  // Count assertion steps to find the right one
  const scenario = props.formState.scenarios[scenarioIndex];
  const effective = getEffectiveSetup(props.formState, scenarioIndex);

  // Count setup steps: date + deps + params + data sources + execution
  let setupStepCount = 0;
  if (effective.calculationDate) setupStepCount++;
  setupStepCount += effective.dependencies.length;
  setupStepCount += effective.parameters.length;
  setupStepCount += effective.dataSources.length;
  if (scenario.execution) setupStepCount++;

  // Unmatched steps in background and scenario
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
    case 'succeeds':
      return 'Uitvoering slaagt';
    case 'fails':
      return 'Uitvoering mislukt';
    case 'failsWith':
      return `Uitvoering mislukt met "${assertion.value}"`;
    case 'boolean':
      return `${assertion.outputName} = ${assertion.value ? 'ja' : 'nee'}`;
    case 'equals':
      return `${assertion.outputName} = ${assertion.value}`;
    case 'equalsString':
      return `${assertion.outputName} = "${assertion.value}"`;
    case 'null':
      return `${assertion.outputName} = null`;
    case 'contains':
      return `${assertion.outputName} bevat "${assertion.value}"`;
    default:
      return `${assertion.assertionType}`;
  }
}

function assertionIcon(stepResult) {
  if (!stepResult) return '\u25CB'; // empty circle
  switch (stepResult.status) {
    case 'passed': return '\u2713';
    case 'failed': return '\u2717';
    case 'skipped': return '\u2014';
    default: return '?';
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
    <!-- Background info (if present) -->
    <div v-if="formState.background" class="sv-background">
      <div class="sv-bg-label">Achtergrond</div>
      <div v-if="formState.background.calculationDate" class="sv-bg-item">
        <span class="sv-bg-icon">\uD83D\uDCC5</span>
        <span class="sv-bg-text">{{ formState.background.calculationDate }}</span>
      </div>
      <div v-if="formState.background.dependencies.length > 0" class="sv-deps">
        <span
          v-for="dep in formState.background.dependencies"
          :key="dep"
          class="sv-dep-pill"
        >{{ dep }}</span>
      </div>
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
        <span class="sv-scenario-toggle">{{ expandedScenarios.has(si) ? '\u25BE' : '\u25B8' }}</span>
        <span class="sv-scenario-name">{{ scenario.name }}</span>
        <span
          v-if="scenarioResult(si)"
          class="sv-scenario-badge"
          :class="`sv-scenario-badge--${scenarioStatus(si)}`"
        >
          {{ scenarioStatus(si) === 'passed' ? 'GESLAAGD' : 'MISLUKT' }}
        </span>
      </button>

      <!-- Scenario body -->
      <div v-if="expandedScenarios.has(si)" class="sv-scenario-body">
        <!-- Setup section -->
        <div class="sv-section">
          <div class="sv-section-title">Invoer</div>

          <!-- Calculation date (scenario-level override or from background) -->
          <div
            v-if="scenario.setup.calculationDate || formState.background?.calculationDate"
            class="sv-field"
          >
            <span class="sv-field-label">Berekeningsdatum</span>
            <span class="sv-field-value">{{ scenario.setup.calculationDate || formState.background?.calculationDate }}</span>
          </div>

          <!-- Extra dependencies (scenario-level) -->
          <div v-if="scenario.setup.dependencies.length > 0" class="sv-field">
            <span class="sv-field-label">Extra afhankelijkheden</span>
            <div class="sv-deps">
              <span
                v-for="dep in scenario.setup.dependencies"
                :key="dep"
                class="sv-dep-pill"
              >{{ dep }}</span>
            </div>
          </div>

          <!-- Parameters -->
          <div
            v-for="param in scenario.setup.parameters"
            :key="param.name"
            class="sv-field"
          >
            <span class="sv-field-label">{{ param.name }}</span>
            <span class="sv-field-value">{{ param.value }}</span>
          </div>
        </div>

        <!-- Data sources -->
        <div v-if="scenario.setup.dataSources.length > 0" class="sv-section">
          <div class="sv-section-title">Gegevensbronnen</div>
          <DataSourceTable
            v-for="(ds, di) in scenario.setup.dataSources"
            :key="di"
            :title="ds.sourceName"
            :key-field="dataSourceToTableProps(ds).keyField"
            :fields="dataSourceToTableProps(ds).fields"
            :model-value="dataSourceToTableProps(ds).rows"
            :default-expanded="true"
            :readonly="readonly"
          />
        </div>

        <!-- Execution -->
        <div v-if="scenario.execution" class="sv-section">
          <div class="sv-section-title">Uitvoering</div>
          <div class="sv-execution">
            Evalueer <code>{{ scenario.execution.outputName }}</code> van <code>{{ scenario.execution.lawId }}</code>
          </div>
        </div>

        <!-- Assertions -->
        <div v-if="scenario.assertions.length > 0" class="sv-section">
          <div class="sv-section-title">Verwachte resultaten</div>
          <div
            v-for="(assertion, ai) in scenario.assertions"
            :key="ai"
            class="sv-assertion"
            :class="assertionResult(si, ai) ? `sv-assertion--${assertionResult(si, ai).status}` : ''"
          >
            <span class="sv-assertion-icon">{{ assertionIcon(assertionResult(si, ai)) }}</span>
            <span class="sv-assertion-label">{{ formatAssertionLabel(assertion) }}</span>
            <span
              v-if="assertionResult(si, ai)?.status === 'passed'"
              class="sv-assertion-badge sv-assertion-badge--pass"
            >GESLAAGD</span>
            <span
              v-if="assertionResult(si, ai)?.status === 'failed'"
              class="sv-assertion-badge sv-assertion-badge--fail"
            >MISLUKT</span>
            <div
              v-if="assertionResult(si, ai)?.error"
              class="sv-assertion-error"
            >{{ assertionResult(si, ai).error }}</div>
          </div>
        </div>

        <!-- Unmatched steps -->
        <div v-if="scenario.unmatchedSteps.length > 0" class="sv-section">
          <div class="sv-section-title">Overige stappen</div>
          <div
            v-for="(step, i) in scenario.unmatchedSteps"
            :key="i"
            class="sv-unmatched"
          >
            <span class="sv-unmatched-keyword">{{ step.keyword }}</span>
            {{ step.text }}
          </div>
        </div>
      </div>
    </div>

    <!-- Empty state -->
    <div v-if="formState.scenarios.length === 0" class="sv-empty">
      Geen scenario's gevonden in dit bestand.
    </div>
  </div>
</template>

<style scoped>
.sv-container {
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
  padding: 8px 0;
}

/* Background */
.sv-background {
  padding: 10px 16px;
  margin: 0 8px 8px;
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
  border-radius: 8px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sv-bg-label {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--semantics-text-color-secondary, #666);
  margin-bottom: 6px;
}

.sv-bg-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  margin-bottom: 4px;
}

.sv-bg-icon {
  font-size: 14px;
}

.sv-bg-text {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 13px;
}

/* Dependencies */
.sv-deps {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 4px;
}

.sv-dep-pill {
  display: inline-block;
  padding: 2px 8px;
  font-size: 11px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  background: #e8eef5;
  color: #154273;
  border-radius: 4px;
  white-space: nowrap;
}

/* Scenario card */
.sv-scenario {
  margin: 0 8px 8px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 8px;
  overflow: hidden;
}

.sv-scenario--passed {
  border-color: #afa;
}

.sv-scenario--failed {
  border-color: #faa;
}

.sv-scenario-header {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 10px 12px;
  background: white;
  border: none;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
  text-align: left;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sv-scenario-header:hover {
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
}

.sv-scenario-toggle {
  flex-shrink: 0;
  width: 12px;
  font-size: 11px;
  color: var(--semantics-text-color-secondary, #666);
}

.sv-scenario-name {
  flex: 1;
}

.sv-scenario-badge {
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 4px;
}

.sv-scenario-badge--passed {
  background: #efe;
  color: #060;
}

.sv-scenario-badge--failed {
  background: #fee;
  color: #c00;
}

/* Scenario body */
.sv-scenario-body {
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

/* Sections */
.sv-section {
  padding: 10px 14px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.sv-section:last-child {
  border-bottom: none;
}

.sv-section-title {
  font-weight: 600;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  color: var(--semantics-text-color-secondary, #666);
  margin-bottom: 8px;
}

/* Fields */
.sv-field {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 4px;
  font-size: 13px;
}

.sv-field-label {
  font-weight: 600;
  font-size: 12px;
  color: var(--semantics-text-color-secondary, #666);
  min-width: 100px;
  flex-shrink: 0;
}

.sv-field-value {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 13px;
  color: var(--semantics-text-color-primary, #1C2029);
}

/* Execution */
.sv-execution {
  font-size: 13px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sv-execution code {
  font-family: 'SF Mono', 'Fira Code', monospace;
  background: var(--semantics-surfaces-color-secondary, #F0F1F3);
  padding: 1px 5px;
  border-radius: 3px;
  font-size: 12px;
}

/* Assertions */
.sv-assertion {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 6px;
  padding: 4px 0;
  font-size: 13px;
}

.sv-assertion-icon {
  flex-shrink: 0;
  width: 16px;
  text-align: center;
  font-weight: bold;
  font-size: 13px;
}

.sv-assertion--passed .sv-assertion-icon { color: #060; }
.sv-assertion--failed .sv-assertion-icon { color: #c00; }
.sv-assertion--skipped .sv-assertion-icon { color: #999; }

.sv-assertion-label {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 12px;
}

.sv-assertion-badge {
  margin-left: auto;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 3px;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.sv-assertion-badge--pass {
  background: #efe;
  color: #060;
}

.sv-assertion-badge--fail {
  background: #fee;
  color: #c00;
}

.sv-assertion-error {
  width: 100%;
  margin-left: 22px;
  margin-top: 2px;
  font-size: 11px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: #c00;
  word-break: break-word;
}

/* Unmatched steps */
.sv-unmatched {
  padding: 3px 0;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: var(--semantics-text-color-secondary, #666);
}

.sv-unmatched-keyword {
  font-weight: 700;
  color: #154273;
}

/* Empty state */
.sv-empty {
  padding: 24px 16px;
  text-align: center;
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #999);
  font-style: italic;
}
</style>

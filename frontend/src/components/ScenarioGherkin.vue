<script setup>
import { ref, computed } from 'vue';
import { useScenarios } from '../composables/useScenarios.js';
import { parseFeature } from '../gherkin/parser.js';
import { runFeature } from '../gherkin/runner.js';

const props = defineProps({
  lawId: { type: String, required: true },
  engine: { type: Object, default: null },
  ready: { type: Boolean, default: false },
  loadDependency: { type: Function, required: true },
});

const lawIdRef = computed(() => props.lawId);

const {
  scenarios,
  selectedScenario,
  featureText,
  loading: scenariosLoading,
  selectScenario,
} = useScenarios(lawIdRef);

const results = ref(null);
const running = ref(false);
const runError = ref(null);

function onScenarioSelect(event) {
  const filename = event.target.value;
  if (filename) selectScenario(filename);
}

async function runScenarios() {
  if (!props.ready || !props.engine) return;

  running.value = true;
  runError.value = null;
  results.value = null;

  try {
    const parsed = parseFeature(featureText.value);
    results.value = await runFeature(parsed, props.engine, {
      loadDependency: props.loadDependency,
    });
  } catch (e) {
    runError.value = e.message || String(e);
  } finally {
    running.value = false;
  }
}

const summary = computed(() => {
  if (!results.value) return null;
  const passed = results.value.filter((r) => r.status === 'passed').length;
  const total = results.value.length;
  return { passed, total, allPassed: passed === total };
});

function stepIcon(status) {
  switch (status) {
    case 'passed': return '\u2713';
    case 'failed': return '\u2717';
    case 'skipped': return '\u2014';
    case 'undefined': return '?';
    default: return '';
  }
}
</script>

<template>
  <div class="scenario-gherkin">
    <!-- Controls -->
    <div class="scenario-controls">
      <select
        class="scenario-select"
        :value="selectedScenario || ''"
        @change="onScenarioSelect"
        :disabled="scenariosLoading"
      >
        <option value="" disabled>
          {{ scenariosLoading ? 'Laden...' : scenarios.length === 0 ? 'Geen scenarios' : 'Selecteer scenario' }}
        </option>
        <option
          v-for="s in scenarios"
          :key="s.filename"
          :value="s.filename"
        >
          {{ s.filename }}
        </option>
      </select>

      <button
        class="scenario-run-btn"
        @click="runScenarios"
        :disabled="!ready || running || !featureText"
      >
        {{ running ? 'Bezig...' : 'Run \u25B6' }}
      </button>
    </div>

    <!-- Feature editor -->
    <div class="scenario-editor-wrap">
      <textarea
        v-model="featureText"
        class="scenario-editor"
        spellcheck="false"
        placeholder="Plak of schrijf een .feature scenario hier..."
      ></textarea>
    </div>

    <!-- Run error -->
    <div v-if="runError" class="scenario-error">{{ runError }}</div>

    <!-- Results -->
    <div v-if="results" class="scenario-results">
      <div v-if="summary" class="scenario-summary" :class="{ 'scenario-summary--pass': summary.allPassed }">
        {{ summary.passed }}/{{ summary.total }} scenarios geslaagd
      </div>

      <div
        v-for="(scenario, si) in results"
        :key="si"
        class="scenario-result"
      >
        <div class="scenario-result-header" :class="`scenario-result--${scenario.status}`">
          <span class="scenario-result-icon">&#x25CF;</span>
          {{ scenario.name }}
          <span class="scenario-result-badge">{{ scenario.status.toUpperCase() }}</span>
        </div>

        <div class="scenario-steps">
          <div
            v-for="(step, i) in scenario.steps"
            :key="i"
            class="scenario-step"
            :class="`scenario-step--${step.status}`"
          >
            <span class="scenario-step-icon">{{ stepIcon(step.status) }}</span>
            <span class="scenario-step-text">{{ step.text }}</span>
            <div v-if="step.error" class="scenario-step-error">{{ step.error }}</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.scenario-gherkin {
  display: flex;
  flex-direction: column;
  height: 100%;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.scenario-controls {
  display: flex;
  gap: 8px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.scenario-select {
  flex: 1;
  padding: 6px 10px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 6px;
  font-size: 13px;
  background: white;
}

.scenario-run-btn {
  padding: 6px 16px;
  background: #154273;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.scenario-run-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.scenario-run-btn:hover:not(:disabled) {
  background: #1a5490;
}

.scenario-editor-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.scenario-editor {
  flex: 1;
  width: 100%;
  min-height: 200px;
  background: #1e1e2e;
  color: #cdd6f4;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 13px;
  line-height: 1.6;
  padding: 16px;
  border: none;
  outline: none;
  resize: none;
  tab-size: 2;
  white-space: pre;
  overflow: auto;
}

.scenario-error {
  padding: 12px 16px;
  background: #fee;
  color: #c00;
  font-size: 13px;
}

.scenario-results {
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
  overflow-y: auto;
  max-height: 50%;
}

.scenario-summary {
  padding: 10px 16px;
  font-weight: 600;
  font-size: 14px;
  background: #fee;
  color: #c00;
}
.scenario-summary--pass {
  background: #efe;
  color: #060;
}

.scenario-result {
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.scenario-result-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  font-weight: 600;
  font-size: 13px;
}
.scenario-result--passed .scenario-result-icon { color: #060; }
.scenario-result--failed .scenario-result-icon { color: #c00; }

.scenario-result-badge {
  margin-left: auto;
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 4px;
}
.scenario-result--passed .scenario-result-badge {
  background: #efe;
  color: #060;
}
.scenario-result--failed .scenario-result-badge {
  background: #fee;
  color: #c00;
}

.scenario-steps {
  padding: 0 16px 8px 16px;
}

.scenario-step {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 2px 0;
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.scenario-step-icon {
  flex-shrink: 0;
  width: 14px;
  text-align: center;
  font-weight: bold;
}
.scenario-step--passed .scenario-step-icon { color: #060; }
.scenario-step--failed .scenario-step-icon { color: #c00; }
.scenario-step--skipped .scenario-step-icon { color: #999; }
.scenario-step--undefined .scenario-step-icon { color: #f80; }

.scenario-step--skipped .scenario-step-text { color: #999; }

.scenario-step-error {
  margin-left: 22px;
  color: #c00;
  font-size: 11px;
  word-break: break-word;
}
</style>

<script setup>
import { computed } from 'vue';
import { formatValue, formatOutputValue, normalizeForCompare, matchStatus as _matchStatus } from '../utils/outputFormat.js';

const props = defineProps({
  /** Execution result with outputs */
  result: { type: Object, default: null },
  /** Pre-rendered box-drawing trace text */
  traceText: { type: String, default: null },
  /** Expected output values: { outputName: expectedValue } */
  expectations: { type: Object, default: () => ({}) },
  /** Error message if execution failed */
  error: { type: String, default: null },
  /** Name of the scenario being displayed */
  scenarioName: { type: String, default: '' },
});

function matchStatus(outputName, actualValue) {
  return _matchStatus(outputName, actualValue, props.expectations);
}

const hasContent = computed(() =>
  props.result || props.traceText || props.error,
);

const overallStatus = computed(() => {
  if (!props.result) return null;
  const keys = Object.keys(props.expectations);
  if (keys.length === 0) return null;
  for (const name of keys) {
    if (matchStatus(name, props.result.outputs?.[name]) === 'failed') return 'failed';
  }
  return 'passed';
});
</script>

<template>
  <!-- Empty state -->
  <ndd-simple-section v-if="!hasContent" align="center">
    <ndd-inline-dialog text="Klik op &quot;Details&quot; bij een scenario om de trace te bekijken."></ndd-inline-dialog>
  </ndd-simple-section>

  <!-- Error state -->
  <ndd-simple-section v-else-if="error && !result" align="center">
    <ndd-inline-dialog variant="alert" text="Fout bij uitvoering" :supporting-text="error"></ndd-inline-dialog>
  </ndd-simple-section>

  <template v-if="result">
    <!-- Scenario title -->
    <ndd-simple-section v-if="scenarioName">
      <ndd-title size="4"><span>{{ scenarioName }}</span></ndd-title>
    </ndd-simple-section>

    <!-- Output summary — only outputs with expectations -->
    <ndd-simple-section v-if="Object.keys(expectations).length">
      <ndd-title size="5" class="etv-section-title"><span>Verwachte uitkomsten</span></ndd-title>
      <div class="etv-expectations-block" :class="overallStatus ? `etv-expectations-block--${overallStatus}` : ''">
        <div
          v-for="name in Object.keys(expectations)"
          :key="name"
          class="etv-expectation-item"
        >
          <span class="etv-expectation-name">{{ name }}</span>
          <span class="etv-expectation-detail">
            <span>{{ formatValue(normalizeForCompare(expectations[name])) }}</span>
            <span class="etv-expectation-arrow">&rarr;</span>
            <span>{{ formatOutputValue(result.outputs?.[name], name) }}</span>
            <span
              v-if="matchStatus(name, result.outputs?.[name]) === 'passed'"
              class="etv-badge etv-badge--pass"
            >GESLAAGD</span>
            <span
              v-if="matchStatus(name, result.outputs?.[name]) === 'failed'"
              class="etv-badge etv-badge--fail"
            >MISLUKT</span>
          </span>
        </div>
      </div>
    </ndd-simple-section>

    <!-- Trace text -->
    <ndd-simple-section v-if="traceText">
      <ndd-title size="5" class="etv-section-title"><span>Execution trace</span></ndd-title>
      <pre class="etv-trace-text">{{ traceText }}</pre>
    </ndd-simple-section>
  </template>

  <!-- Partial trace on error -->
  <ndd-simple-section v-if="error && traceText && !result">
    <ndd-title size="5" class="etv-section-title"><span>Partial trace (tot fout)</span></ndd-title>
    <pre class="etv-trace-text">{{ traceText }}</pre>
  </ndd-simple-section>
</template>

<style scoped>
.etv-section-title {
  margin-bottom: 4px;
}

/* Single continuous block with left border */
.etv-expectations-block {
  border-left: 3px solid transparent;
  padding: 8px 10px;
  border-radius: 4px;
  background: var(--semantics-surfaces-tinted-background-color, #f5f5f5);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.etv-expectations-block--passed {
  border-left-color: #2e7d32;
}

.etv-expectations-block--failed {
  border-left-color: #c62828;
}

.etv-expectation-item {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px;
  font-size: 13px;
}

.etv-expectation-name {
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
}

.etv-expectation-detail {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 13px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.etv-expectation-arrow {
  flex-shrink: 0;
  color: var(--semantics-text-color-secondary, #666);
}

.etv-badge {
  flex-shrink: 0;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 3px;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
}

.etv-badge--pass { background: #efe; color: #060; }
.etv-badge--fail { background: #fee; color: #c00; }

.etv-trace-text {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 11px;
  line-height: 1.5;
  padding: 8px;
  background: #1e1e2e;
  color: #cdd6f4;
  border-radius: 6px;
  overflow-x: auto;
  white-space: pre;
  margin: 0;
}
</style>

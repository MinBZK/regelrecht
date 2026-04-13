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
    <ndd-simple-section>
      <ndd-title size="5" class="etv-section-title"><span>Verwachte uitkomsten</span></ndd-title>
      <div class="etv-outputs">
        <div
          v-for="name in Object.keys(expectations)"
          :key="name"
          class="etv-output-row"
          :class="`etv-output-row--${matchStatus(name, result.outputs[name])}`"
        >
          <div class="etv-output-name">{{ name }}</div>
          <div class="etv-output-detail">
            <span class="etv-output-expected">{{ formatValue(normalizeForCompare(expectations[name])) }}</span>
            <span class="etv-output-arrow">&rarr;</span>
            <span class="etv-output-actual">{{ formatOutputValue(result.outputs[name], name) }}</span>
            <span
              v-if="matchStatus(name, result.outputs[name]) === 'passed'"
              class="etv-badge etv-badge--pass"
            >GESLAAGD</span>
            <span
              v-if="matchStatus(name, result.outputs[name]) === 'failed'"
              class="etv-badge etv-badge--fail"
            >MISLUKT</span>
          </div>
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
  <ndd-simple-section v-if="error && traceText">
    <ndd-title size="5" class="etv-section-title"><span>Partial trace (tot fout)</span></ndd-title>
    <pre class="etv-trace-text">{{ traceText }}</pre>
  </ndd-simple-section>
</template>

<style scoped>
.etv-section-title {
  margin-bottom: 4px;
}

.etv-outputs {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.etv-output-row {
  border-left: 3px solid transparent;
  padding: 6px 10px;
  border-radius: 4px;
  background: var(--semantics-surface-color-secondary, #f5f5f5);
}

.etv-output-row--passed {
  border-left-color: #2e7d32;
}

.etv-output-row--failed {
  border-left-color: #c62828;
}

.etv-output-name {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 2px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.etv-output-detail {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.etv-output-expected,
.etv-output-actual {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 12px;
}

.etv-output-arrow {
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

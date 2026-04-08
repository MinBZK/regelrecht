<script setup>
import { computed } from 'vue';
import { formatValue, formatOutputValue, matchStatus as _matchStatus } from '../utils/outputFormat.js';

const props = defineProps({
  /** Execution result with outputs */
  result: { type: Object, default: null },
  /** Pre-rendered box-drawing trace text */
  traceText: { type: String, default: null },
  /** Expected output values: { outputName: expectedValue } */
  expectations: { type: Object, default: () => ({}) },
  /** Error message if execution failed */
  error: { type: String, default: null },
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
    <!-- Output summary -->
    <ndd-simple-section>
      <div class="etv-section-title">Resultaat</div>
      <div
        v-for="(value, name) in result.outputs"
        :key="name"
        class="etv-output"
        :class="`etv-output--${matchStatus(name, value)}`"
      >
        <span class="etv-output-icon">
          <template v-if="matchStatus(name, value) === 'passed'">&#x2713;</template>
          <template v-else-if="matchStatus(name, value) === 'failed'">&#x2717;</template>
          <template v-else>&#x25CF;</template>
        </span>
        <span class="etv-output-name">{{ name }}:</span>
        <span class="etv-output-value">{{ formatOutputValue(value, name) }}</span>
        <span
          v-if="matchStatus(name, value) === 'passed'"
          class="etv-badge etv-badge--pass"
        >GESLAAGD</span>
        <span
          v-if="matchStatus(name, value) === 'failed'"
          class="etv-badge etv-badge--fail"
        >MISLUKT (verwacht: {{ formatValue(expectations[name]) }})</span>
      </div>
    </ndd-simple-section>

    <!-- Trace text -->
    <ndd-simple-section v-if="traceText">
      <div class="etv-section-title">Execution trace</div>
      <pre class="etv-trace-text">{{ traceText }}</pre>
    </ndd-simple-section>
  </template>

  <!-- Partial trace on error -->
  <ndd-simple-section v-if="error && traceText">
    <div class="etv-section-title">Partial trace (tot fout)</div>
    <pre class="etv-trace-text">{{ traceText }}</pre>
  </ndd-simple-section>
</template>

<style scoped>
.etv-section-title {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 8px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.etv-output {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 4px 0;
  font-size: 13px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.etv-output-icon {
  flex-shrink: 0;
  width: 14px;
  text-align: center;
  font-weight: bold;
}

.etv-output--passed .etv-output-icon { color: #060; }
.etv-output--failed .etv-output-icon { color: #c00; }
.etv-output--neutral .etv-output-icon { color: #666; }

.etv-output-name {
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
}

.etv-output-value {
  color: var(--semantics-text-color-secondary, #555);
}

.etv-badge {
  margin-left: auto;
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

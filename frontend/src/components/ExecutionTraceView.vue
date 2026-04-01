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
  <div class="etv-container">
    <div class="etv-scroll">
      <!-- Empty state -->
      <div v-if="!hasContent" class="etv-empty">
        <div class="etv-empty-text">Klik op "Uitvoeren" om de wet uit te voeren en de trace te bekijken.</div>
      </div>

      <!-- Error state -->
      <div v-else-if="error && !result" class="etv-error">
        <div class="etv-error-title">Fout bij uitvoering</div>
        <div class="etv-error-message">{{ error }}</div>
      </div>

      <template v-if="result">
        <!-- Output summary -->
        <div class="etv-section etv-outputs">
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
        </div>

        <!-- Trace text -->
        <div v-if="traceText" class="etv-section">
          <div class="etv-section-title">Execution trace</div>
          <pre class="etv-trace-text">{{ traceText }}</pre>
        </div>
      </template>

      <!-- Partial trace on error -->
      <div v-if="error && traceText" class="etv-section">
        <div class="etv-section-title">Partial trace (tot fout)</div>
        <pre class="etv-trace-text">{{ traceText }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.etv-container {
  height: 100%;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.etv-scroll {
  height: 100%;
  overflow-y: auto;
}

.etv-empty {
  padding: 32px 16px;
  text-align: center;
}

.etv-empty-text {
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #666);
}

.etv-running {
  padding: 12px 16px;
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #666);
  font-style: italic;
}

.etv-error {
  padding: 12px 16px;
}

.etv-error-title {
  font-weight: 600;
  font-size: 13px;
  color: #c00;
  margin-bottom: 4px;
}

.etv-error-message {
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: #c00;
  word-break: break-word;
  white-space: pre-wrap;
}

.etv-section {
  padding: 12px 16px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

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
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
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

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
    <!-- Output summary — styled to match ScenarioForm's "Verwachte uitkomsten" -->
    <ndd-simple-section>
      <ndd-title size="5" class="etv-section-title"><span>Verwachte uitkomsten</span></ndd-title>
      <ndd-list variant="box" class="etv-outputs-list">
        <ndd-list-item
          v-for="(value, name) in result.outputs"
          :key="name"
          size="md"
          class="etv-output-item"
          :class="`etv-output-item--${matchStatus(name, value)}`"
        >
          <ndd-text-cell :text="name" max-width="140"></ndd-text-cell>
          <ndd-cell>
            <div class="etv-output-values">
              <template v-if="name in expectations">
                <ndd-text-field size="md" :value="formatValue(normalizeForCompare(expectations[name]))" readonly></ndd-text-field>
                <span class="etv-output-arrow">&rarr;</span>
              </template>
              <ndd-text-field size="md" :value="formatOutputValue(value, name)" readonly></ndd-text-field>
              <span
                v-if="matchStatus(name, value) === 'passed'"
                class="etv-badge etv-badge--pass"
              >GESLAAGD</span>
              <span
                v-if="matchStatus(name, value) === 'failed'"
                class="etv-badge etv-badge--fail"
              >MISLUKT</span>
            </div>
          </ndd-cell>
        </ndd-list-item>
      </ndd-list>
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

.etv-output-item {
  border-left: 3px solid transparent;
}

.etv-output-item--passed {
  border-left-color: #2e7d32;
}

.etv-output-item--failed {
  border-left-color: #c62828;
}

.etv-output-values {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
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
  /* Use a monospace font where box-drawing pipes (│├└─) connect
   * vertically into continuous lines. line-height: 1.0 ensures no
   * gap between rows; letter-spacing: 0 prevents horizontal gaps. */
  font-family: 'Cascadia Mono', 'JetBrains Mono', 'Fira Code', 'SF Mono', 'Consolas', monospace;
  font-size: 12px;
  line-height: 1.0;
  letter-spacing: 0;
  padding: 12px;
  background: #1e1e2e;
  color: #cdd6f4;
  border-radius: 6px;
  overflow-x: auto;
  white-space: pre;
  margin: 0;
}
</style>

<style>
/* Unscoped: ndd web components need global selectors */
.etv-outputs-list ndd-text-cell {
  width: 140px;
  min-width: 140px;
  flex-shrink: 0;
}

.etv-outputs-list ndd-cell {
  flex: 1;
  min-width: 0;
}

.etv-outputs-list ndd-text-field {
  flex: 1;
  min-width: 0;
}
</style>

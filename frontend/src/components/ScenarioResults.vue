<script setup>
defineProps({
  /** Execution result object from engine.execute() */
  result: { type: Object, default: null },
  /** Expected output values for comparison: { outputName: expectedValue } */
  expectations: { type: Object, default: () => ({}) },
  /** Error message if execution failed */
  error: { type: String, default: null },
  /** Whether execution is running */
  running: { type: Boolean, default: false },
});

function formatValue(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  return String(value);
}

function formatOutputValue(value, name) {
  const raw = formatValue(value);
  // Check if this looks like eurocent (large number for a monetary output)
  if (typeof value === 'number' && isLikelyEurocent(name, value)) {
    return `${raw} (${(value / 100).toFixed(2)} euro)`;
  }
  return raw;
}

function isLikelyEurocent(name, value) {
  // Heuristic: integer amounts with monetary-sounding names are likely eurocent
  return (
    Number.isInteger(value) &&
    (name.includes('hoogte') || name.includes('bedrag') || name.includes('premie'))
  );
}

function matchStatus(outputName, actualValue, expectations) {
  if (!(outputName in expectations)) return 'neutral';
  const expected = expectations[outputName];
  if (expected === null || expected === undefined) return 'neutral';

  // Normalize for comparison
  const actual = normalizeForCompare(actualValue);
  const exp = normalizeForCompare(expected);

  if (primitiveEqual(actual, exp)) return 'passed';
  return 'failed';
}

function primitiveEqual(a, b) {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === 'number') return Math.abs(a - b) < 1e-9;
  return false;
}

function normalizeForCompare(value) {
  if (value === 'true' || value === true) return true;
  if (value === 'false' || value === false) return false;
  if (value === 'null' || value === null) return null;
  if (typeof value === 'string' && /^-?\d+(\.\d+)?$/.test(value)) {
    return Number(value);
  }
  return value;
}
</script>

<template>
  <div class="sr-container">
    <!-- Running state -->
    <div v-if="running" class="sr-running">
      Uitvoeren...
    </div>

    <!-- Error state -->
    <div v-else-if="error" class="sr-error">
      <div class="sr-error-title">Fout bij uitvoering</div>
      <div class="sr-error-message">{{ error }}</div>
    </div>

    <!-- Results -->
    <div v-else-if="result" class="sr-results">
      <div class="sr-title">Resultaat</div>

      <div
        v-for="(value, name) in result.outputs"
        :key="name"
        class="sr-output"
        :class="`sr-output--${matchStatus(name, value, expectations)}`"
      >
        <span class="sr-output-icon">
          <template v-if="matchStatus(name, value, expectations) === 'passed'">&#x2713;</template>
          <template v-else-if="matchStatus(name, value, expectations) === 'failed'">&#x2717;</template>
          <template v-else>&#x25CF;</template>
        </span>
        <span class="sr-output-name">{{ name }}:</span>
        <span class="sr-output-value">{{ formatOutputValue(value, name) }}</span>
        <span
          v-if="matchStatus(name, value, expectations) === 'passed'"
          class="sr-output-badge sr-output-badge--pass"
        >
          GESLAAGD
        </span>
        <span
          v-if="matchStatus(name, value, expectations) === 'failed'"
          class="sr-output-badge sr-output-badge--fail"
        >
          MISLUKT (verwacht: {{ formatValue(expectations[name]) }})
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sr-container {
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
}

.sr-running {
  padding: 12px 16px;
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #666);
  font-style: italic;
}

.sr-error {
  padding: 12px 16px;
}

.sr-error-title {
  font-weight: 600;
  font-size: 13px;
  color: #c00;
  margin-bottom: 4px;
}

.sr-error-message {
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: #c00;
  word-break: break-word;
  white-space: pre-wrap;
}

.sr-results {
  padding: 12px 16px;
}

.sr-title {
  font-weight: 600;
  font-size: 14px;
  margin-bottom: 8px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sr-output {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 4px 0;
  font-size: 13px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.sr-output-icon {
  flex-shrink: 0;
  width: 14px;
  text-align: center;
  font-weight: bold;
}

.sr-output--passed .sr-output-icon { color: #060; }
.sr-output--failed .sr-output-icon { color: #c00; }
.sr-output--neutral .sr-output-icon { color: #666; }

.sr-output-name {
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sr-output-value {
  color: var(--semantics-text-color-secondary, #555);
}

.sr-output-badge {
  margin-left: auto;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 3px;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
}

.sr-output-badge--pass {
  background: #efe;
  color: #060;
}

.sr-output-badge--fail {
  background: #fee;
  color: #c00;
}
</style>

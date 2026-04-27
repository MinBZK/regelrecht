<script setup>
import { computed } from 'vue';
import { formatValue, formatOutputValue, formatOutputValueParts, normalizeForCompare, matchStatus as _matchStatus } from '../utils/outputFormat.js';

function humanize(name) {
  if (typeof name !== 'string') return name;
  const spaced = name.replace(/_/g, ' ');
  return /[A-Z]/.test(spaced) && spaced === spaced.toUpperCase() ? spaced.toLowerCase() : spaced;
}

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
  <nldd-simple-section v-if="!hasContent" align="center">
    <nldd-inline-dialog text="Klik op &quot;Details&quot; bij een scenario om de trace te bekijken."></nldd-inline-dialog>
  </nldd-simple-section>

  <nldd-simple-section v-else-if="error && !result && !traceText" align="center">
    <nldd-inline-dialog variant="alert" text="Fout bij uitvoering" :supporting-text="error"></nldd-inline-dialog>
  </nldd-simple-section>

  <nldd-simple-section v-else>
    <template v-if="result && scenarioName">
      <nldd-title size="4"><span>{{ scenarioName }}</span></nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <template v-if="result && Object.keys(expectations).length">
      <nldd-title size="5" class="etv-section-title"><span>Verwachte uitkomsten</span></nldd-title>
      <nldd-spacer size="4"></nldd-spacer>
      <nldd-list variant="simple">
        <nldd-list-item size="md">
          <nldd-text-cell size="md" color="secondary" text=""></nldd-text-cell>
          <nldd-text-cell
            size="md"
            color="secondary"
            horizontal-alignment="right"
            width="100px"
            text="Verwacht"
          ></nldd-text-cell>
          <nldd-text-cell
            size="md"
            color="secondary"
            horizontal-alignment="right"
            width="100px"
            text="Uitkomst"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell size="md" color="secondary" horizontal-alignment="right" width="80px" text="Status"></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item v-for="name in Object.keys(expectations)" :key="name" size="md">
          <nldd-text-cell size="md" :text="humanize(name)"></nldd-text-cell>
          <nldd-text-cell
            size="md"
            horizontal-alignment="right"
            width="100px"
            :text="humanize(formatValue(normalizeForCompare(expectations[name])))"
          ></nldd-text-cell>
          <nldd-text-cell
            size="md"
            horizontal-alignment="right"
            width="100px"
            :text="humanize(formatOutputValueParts(result.outputs?.[name], name).text)"
            :supporting-text="formatOutputValueParts(result.outputs?.[name], name).supportingText"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            size="md"
            horizontal-alignment="right"
            width="80px"
            :text="matchStatus(name, result.outputs?.[name]) === 'passed'
              ? 'Geslaagd'
              : matchStatus(name, result.outputs?.[name]) === 'failed'
                ? 'Mislukt'
                : '—'"
          ></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <template v-if="result && traceText">
      <nldd-title size="5" class="etv-section-title"><span>Execution trace</span></nldd-title>
      <nldd-spacer size="4"></nldd-spacer>
      <pre class="etv-trace-text">{{ traceText }}</pre>
    </template>

    <template v-if="error && traceText && !result">
      <nldd-title size="5" class="etv-section-title"><span>Partial trace (tot fout)</span></nldd-title>
      <nldd-spacer size="4"></nldd-spacer>
      <pre class="etv-trace-text">{{ traceText }}</pre>
    </template>
  </nldd-simple-section>
</template>

<style scoped>
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

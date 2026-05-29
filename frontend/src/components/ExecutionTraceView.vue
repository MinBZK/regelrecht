<script setup>
import { computed } from 'vue';
import { formatValue, formatOutputValue, formatOutputValueParts, normalizeForCompare, matchStatus as _matchStatus, humanize } from '../utils/outputFormat.js';

const props = defineProps({
  /** Execution result with outputs */
  result: { type: Object, default: null },
  /** Pre-rendered box-drawing trace text */
  traceText: { type: String, default: null },
  /** Expected output values: { outputName: expectedValue } */
  expectations: { type: Object, default: () => ({}) },
  /** Error message if execution failed */
  error: { type: String, default: null },
  /** Scenario is currently executing */
  running: { type: Boolean, default: false },
  /** Whether a re-run action is available */
  canReload: { type: Boolean, default: false },
});

const emit = defineEmits(['reload']);

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
  <nldd-inline-dialog v-if="running" text="Bezig met uitvoeren…"></nldd-inline-dialog>

  <template v-else-if="error && !result && !traceText">
    <nldd-inline-dialog variant="alert" text="Fout bij uitvoering" :supporting-text="error"></nldd-inline-dialog>
    <template v-if="canReload">
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-button size="md" text="Opnieuw uitvoeren" @click="emit('reload')"></nldd-button>
    </template>
  </template>

  <template v-else-if="!hasContent">
    <nldd-inline-dialog text="Nog geen resultaat voor dit scenario."></nldd-inline-dialog>
    <template v-if="canReload">
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-button size="md" text="Opnieuw uitvoeren" @click="emit('reload')"></nldd-button>
    </template>
  </template>

  <template v-else>
    <template v-if="result && Object.keys(expectations).length">
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
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-code-viewer>{{ traceText }}</nldd-code-viewer>
    </template>

    <template v-if="error && traceText && !result">
      <nldd-title size="5" class="etv-section-title"><span>Partial trace (tot fout)</span></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-code-viewer>{{ traceText }}</nldd-code-viewer>
      <template v-if="canReload">
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-button size="md" text="Opnieuw uitvoeren" @click="emit('reload')"></nldd-button>
      </template>
    </template>
  </template>
</template>

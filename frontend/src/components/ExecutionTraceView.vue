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

/* The trace is a box-drawing tree, so every line's meaning sits in its column:
 * wrapping restarts a continuation at column 0 and it reads as if it lives at
 * depth 0, cutting straight through the tree gutters. So no `wrap` — the long
 * lines have to scroll sideways instead.
 *
 * But `nldd-code-viewer` grows to its full content height (3167px on
 * wet_op_de_zorgtoeslag art. 2), which parks its horizontal scrollbar thousands
 * of pixels below the fold, inside the sheet's own scroller. Unfindable — the
 * reason the `wrap` went on in the first place (#1101).
 *
 * The component owns no height API, so we bound it from here. CodeMirror won't
 * take a height through the host (it lays the scroller out itself), so the rule
 * has to land inside the shadow root: `.cm-editor` needs a definite height and
 * `.cm-scroller` an overflow, per CodeMirror's own fixed-height recipe. That
 * gives the block its own two scrollbars at the edges of a visible box, with
 * the column alignment intact.
 *
 * Remove this once `nldd-code-viewer` grows a height/rows property of its own
 * (nldd-code-editor already has `rows`); then this becomes an attribute.
 */
const TRACE_SCROLL_BOX_CSS = `
  .code-viewer { display: flex; flex-direction: column; min-height: 0; }
  .cm-editor { height: 100%; min-height: 0; }
  .cm-scroller { overflow: auto; }
`;

function applyTraceScrollBox(el) {
  const root = el?.shadowRoot;
  if (!root) return false;
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(TRACE_SCROLL_BOX_CSS);
  // Appended last so it wins over the component's own styles at equal
  // specificity. Lit keeps its static styles in the same list, so replacing the
  // array wholesale would strip them.
  root.adoptedStyleSheets = [...root.adoptedStyleSheets, sheet];
  return true;
}

const vTraceScrollBox = {
  async mounted(el) {
    // The element only has a shadow root once its definition is registered and
    // it has upgraded. It normally is by now (the design system is imported at
    // startup), so this awaits nothing; but under a lazy registration the block
    // would silently stay unbounded, and a 3000px trace is the exact failure
    // we're fixing.
    if (applyTraceScrollBox(el)) return;
    await customElements.whenDefined('nldd-code-viewer');
    applyTraceScrollBox(el);
  },
};
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
      <nldd-code-viewer v-trace-scroll-box class="etv-trace">{{ traceText }}</nldd-code-viewer>
    </template>

    <template v-if="error && traceText && !result">
      <nldd-title size="5" class="etv-section-title"><span>Partial trace (tot fout)</span></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-code-viewer v-trace-scroll-box class="etv-trace">{{ traceText }}</nldd-code-viewer>
      <template v-if="canReload">
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-button size="md" text="Opnieuw uitvoeren" @click="emit('reload')"></nldd-button>
      </template>
    </template>
  </template>
</template>

<style scoped>
/* The ceiling the shadow-root rules in v-trace-scroll-box scroll against. A
   short trace still renders at its own height; only a long one gets capped,
   and then its scrollbars sit at the edges of a box that fits on screen. */
.etv-trace {
  max-height: 60vh;
}
</style>

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
 * But `nldd-code-viewer` grows to its full content height — on
 * wet_op_de_zorgtoeslag art. 2 the e2e measures a `scrollHeight` of 3114px of
 * trace content — which parks its horizontal scrollbar thousands of pixels
 * below the fold, inside the sheet's own scroller. Unfindable — the reason the
 * `wrap` went on in the first place (#1101).
 *
 * Bounding it takes nothing but the `max-height` in `.etv-trace` below: the
 * component's own CodeMirror theme already ships `.cm-editor { height: 100% }`
 * and `.cm-scroller { overflow: auto }`, so a capped host is exactly
 * CodeMirror's fixed-height recipe. Both scrollbars then sit at the edges of a
 * box that starts *and ends* inside the viewport on the reference fixture, with
 * the column alignment intact — see `.etv-trace` below for the measurement that
 * fixes the cap, and for the one case where a short sheet-scroll is still
 * needed. No shadow-root styling needed.
 *
 * What the component does not do is notice that it now scrolls *vertically*.
 * `_updateScrollable()` measures only the horizontal axis, so on a trace whose
 * lines all fit the width it leaves the scroll region without
 * tabindex/role/aria-label: measured on such a trace the accessibility tree has
 * no region for it at all, so a screen reader announces nothing to enter, and
 * only browsers that make overflowing scrollers focusable by themselves put it
 * in the tab order (WCAG 2.1.1). The height cap is what opened that gap, so
 * closing it is ours: `v-trace-scroll-box` marks the region from here.
 *
 * Remove the directive once `nldd-code-viewer` marks its scroll region on both
 * axes; drop `.etv-trace` once it grows a height/rows property of its own
 * (nldd-code-editor already has `rows`), which would make this an attribute.
 */
const TRACE_REGION_LABEL = 'Execution trace';

/**
 * Keep the viewer's scroll region focusable and labelled for as long as it
 * overflows on *either* axis, and return a teardown.
 *
 * The design system marks the region itself, but only on horizontal overflow,
 * and it recomputes on a rAF-debounced ResizeObserver, on every slot change and
 * on several property changes — stripping the attributes again each time it
 * finds no horizontal overflow. A one-shot set would therefore be undone, so we
 * watch and re-apply.
 *
 * Re-applying is asynchronous, and that costs something: our MutationObserver
 * callback only runs a microtask after the design system has already called
 * `removeAttribute('tabindex')`. In that window the scroller is not focusable,
 * so if the user's keyboard focus was *inside* it the HTML focus-fixup rule has
 * moved focus to `<body>` — re-adding the attribute cannot bring it back. A
 * design-system recompute (new trace text after a re-run, its ResizeObserver, a
 * variant/background/no-copy change) can therefore drop focus out of the trace.
 * That is inherent to marking the region from the app side; it goes away when
 * `nldd-code-viewer` measures both axes itself. Known cost, not a bug to hunt.
 *
 * Two observers, because both triggers exist: content that
 * starts overflowing produces no attribute mutation at all (nothing was there to
 * remove), and a detach/reattach re-mount builds a brand-new `.cm-scroller`.
 * `.code-viewer` is the component's own render root and survives that re-mount,
 * so it is the stable thing to observe.
 *
 * Add-only on purpose: removal stays the design system's call, which keeps the
 * two from fighting, and re-setting an attribute that is already there is a
 * no-op — so the MutationObserver settles instead of looping. The label is the
 * one exception: it is written whenever it differs, because on a trace that
 * *also* overflows horizontally the design system gets there first and labels
 * the region with its own generic default. `translations` (below) makes that
 * default our label too, so in practice both write the same string and there is
 * nothing to correct.
 */
function watchScrollRegion(el, label) {
  const root = el.shadowRoot;
  const block = root?.querySelector('.code-viewer');
  if (!block) return null;

  let scroller = null;

  function markRegion() {
    // A 0-width measure (mid-reflow, or before first layout) is never a
    // trustworthy overflow signal — the design system bails on it for the same
    // reason. A later observer tick re-measures once the width is real.
    if (!scroller || scroller.clientWidth === 0) return;
    const overflows = scroller.scrollHeight > scroller.clientHeight
      || scroller.scrollWidth > scroller.clientWidth;
    if (!overflows) return;
    if (!scroller.hasAttribute('tabindex')) scroller.setAttribute('tabindex', '0');
    if (!scroller.hasAttribute('role')) scroller.setAttribute('role', 'region');
    if (scroller.getAttribute('aria-label') !== label) scroller.setAttribute('aria-label', label);
  }

  const resizeObserver = new ResizeObserver(markRegion);

  function bindScroller() {
    const next = root.querySelector('.cm-scroller');
    if (next === scroller) return;
    if (scroller) resizeObserver.unobserve(scroller);
    scroller = next;
    if (scroller) resizeObserver.observe(scroller);
  }

  const attributeObserver = new MutationObserver(() => {
    bindScroller();
    markRegion();
  });
  attributeObserver.observe(block, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ['tabindex', 'role', 'aria-label'],
  });

  bindScroller();
  markRegion();

  return () => {
    attributeObserver.disconnect();
    resizeObserver.disconnect();
  };
}

async function markTraceScrollRegion(el, state) {
  // The element only has a shadow root once its definition is registered and it
  // has upgraded; CodeMirror only exists after Lit's first render. The design
  // system is imported at startup, so this normally awaits a microtask.
  if (!el.shadowRoot) await customElements.whenDefined('nldd-code-viewer');
  await el.updateComplete;
  if (state.stopped) return;
  const stop = watchScrollRegion(el, TRACE_REGION_LABEL);
  if (!stop) throw new Error('nldd-code-viewer exposes no .code-viewer to observe');
  if (state.stopped) stop();
  else state.stop = stop;
}

/** Per-element observer state, so `unmounted` can stop what `mounted` started. */
const scrollRegionWatchers = new WeakMap();

const vTraceScrollBox = {
  mounted(el) {
    // `translations` is the component's own knob for the scroll region's label,
    // so the region reads as this block rather than as generic "Code" when the
    // component labels it itself. Set before its first render, so its first
    // measure already uses it.
    el.translations = { 'components.code-viewer.region-label': TRACE_REGION_LABEL };
    const state = { stopped: false, stop: null };
    scrollRegionWatchers.set(el, state);
    // Not an `async mounted`: a rejection there is an unhandled rejection, and
    // the failure mode is silent either way — the trace block would keep
    // scrolling under the mouse while staying unreachable by keyboard.
    markTraceScrollRegion(el, state).catch((error) => {
      console.warn(
        'ExecutionTraceView: kon het scrollgebied van de execution trace niet '
        + 'markeren. Het blok blijft met de muis scrollbaar, maar is niet met '
        + 'het toetsenbord te bereiken of aan te kondigen.',
        error,
      );
    });
  },
  unmounted(el) {
    const state = scrollRegionWatchers.get(el);
    if (!state) return;
    state.stopped = true;
    state.stop?.();
    scrollRegionWatchers.delete(el);
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
/* The whole scroll box. `nldd-code-viewer` has no height API. CodeMirror's own
   `.cm-editor { height: 100% }` does not resolve against the host but against
   `.code-viewer` — that is the element `mountEditor` hands to
   `new EditorView({ parent })`. The cap still reaches it, one step further out:
   `:host { display: flex }` makes a *row* container, so `.code-viewer` stretches
   to the host's height on the cross axis — that default `align-items: stretch`
   is what carries the cap down, not the `flex-grow: 1` the design system sets
   (that one sizes the width). Clamping the host therefore makes
   `.code-viewer`'s height definite and the editor's 100% has something to
   resolve against. So what would break this is `align-items` or a
   `flex-direction: column` appearing on `:host`, not a change to `flex-grow`.
   (Measured: host 288px → scroller clientHeight 256px, the difference being
   `.code-viewer`'s 2×16px block padding.)

   40vh, not more, because the block does not start at the top of the screen. On
   the reference fixture (wet_op_de_zorgtoeslag art. 2, 1280×720) it starts
   356px down — top bar, expectations list, section heading — so 40vh = 288px
   puts its bottom edge at 644px, some 76px above the fold: both scrollbars are
   on screen without scrolling the sheet first. 60vh would end at 788px, i.e.
   below the fold, which is the #1101 symptom in miniature.

   Those 76px of headroom are worth exactly one extra expectation row (measured:
   45px each). So the honest claim is not "always fits": on a scenario with a
   longer expectations list the block starts lower, and reaching its bottom edge
   still takes one short sheet-scroll. Bounded, though — tens of pixels, not the
   ~2700px of #1101. A short trace renders at its own height untouched; only a
   long one gets capped. */
.etv-trace {
  max-height: 40vh;
}
</style>

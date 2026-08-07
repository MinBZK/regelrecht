/**
 * usePrototypeView — the machinery the three prototypes share.
 *
 * Each prototype only differs in *where it puts things*. Everything around that
 * is identical and lives here:
 *
 *  - the pan/zoom transform (`usePanZoom`) over the shared world box;
 *  - the zoom → level mapping and the cross-fade between two levels
 *    (`useSemanticZoom`);
 *  - a per-(level × edge-filter) cache of the pure layout result, plus a
 *    warm-up that computes the two deeper levels right after first paint so
 *    scrolling into them does not stall;
 *  - the timings the comparison needs: how long each level took to lay out,
 *    and a rolling frame time while you scroll (criterion 10).
 */
import { computed, ref, shallowRef, watch } from 'vue';
import { usePanZoom } from './usePanZoom.js';
import { useSemanticZoom } from './useSemanticZoom.js';
import { LEVELS, buildIndex } from '../lib/archIndex.js';

/**
 * @param {import('vue').Ref<object>} modelRef
 * @param {(model:object, level:string, opts:object) => object} layoutFn
 * @param {import('vue').Ref<Set<string>>} enabledKindsRef
 */
export function usePrototypeView(modelRef, layoutFn, enabledKindsRef) {
  const panzoom = usePanZoom();
  const { blend, level } = useSemanticZoom(panzoom.zoom);

  const cache = new Map(); // `${level}|${kinds}` -> layout
  const timings = ref({}); // level -> ms of the last layout run
  const busy = ref(false);

  let index = null;
  let indexModel = null;

  const kindKey = computed(() => [...enabledKindsRef.value].sort().join(','));

  function layoutFor(lvl) {
    const model = modelRef.value;
    if (!model || !lvl) return null;
    if (indexModel !== model) {
      index = buildIndex(model);
      indexModel = model;
    }
    const key = `${lvl}|${kindKey.value}`;
    const hit = cache.get(key);
    if (hit) return hit;
    const t0 = performance.now();
    const result = layoutFn(model, lvl, { index, enabledKinds: enabledKindsRef.value });
    timings.value = { ...timings.value, [lvl]: Math.round(performance.now() - t0) };
    cache.set(key, result);
    return result;
  }

  // Changing the model or the edge filters invalidates everything.
  watch([modelRef, kindKey], () => {
    cache.clear();
    timings.value = {};
    warm();
  });

  /**
   * Compute the deeper levels off the critical path. They are pure functions,
   * so this is only about *when* the cost is paid: paying it during a wheel
   * event is what would make the level switch feel like a stall.
   */
  function warm() {
    const schedule =
      typeof requestIdleCallback === 'function'
        ? (fn) => requestIdleCallback(fn, { timeout: 2000 })
        : (fn) => setTimeout(fn, 60);
    busy.value = true;
    const queue = LEVELS.slice();
    const step = () => {
      const next = queue.shift();
      if (!next) {
        busy.value = false;
        return;
      }
      layoutFor(next);
      schedule(step);
    };
    schedule(step);
  }

  const baseLayout = shallowRef(null);
  const nextLayout = shallowRef(null);
  watch(
    [blend, modelRef, kindKey],
    () => {
      baseLayout.value = layoutFor(blend.value.base);
      nextLayout.value = blend.value.next ? layoutFor(blend.value.next) : null;
    },
    { immediate: true },
  );

  // --- Frame timing (criterion 10) -----------------------------------------
  const frameMs = ref(0);
  const samples = [];
  function recordFrame(ms) {
    samples.push(ms);
    if (samples.length > 30) samples.shift();
    // Median, so one garbage-collection spike does not dominate the readout.
    const sorted = samples.slice().sort((a, b) => a - b);
    frameMs.value = Math.round(sorted[sorted.length >> 1] * 10) / 10;
  }

  // The level you are actually reading — the one carrying most of the opacity,
  // not the one being faded out. Read-outs and hit-testing follow this one.
  const activeLayout = computed(() => {
    const { next, t } = blend.value;
    return next && t >= 0.5 ? nextLayout.value : baseLayout.value;
  });
  const stats = computed(() => activeLayout.value?.stats || null);

  return {
    panzoom,
    blend,
    level,
    baseLayout,
    nextLayout,
    activeLayout,
    timings,
    busy,
    stats,
    frameMs,
    recordFrame,
    warm,
    layoutFor,
  };
}

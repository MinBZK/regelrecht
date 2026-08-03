/**
 * useSemanticZoom — the detail level is a pure function of the zoom factor.
 *
 * The three prototypes have no expand/collapse. Scrolling is the only way to
 * change what you see: zoom out far enough and you look at the 20 containers,
 * zoom in and the 803 components fade in on top of them, zoom in further and
 * the 1605 code nodes appear.
 *
 * The zoom factor `k` is expressed **relative to "the whole model fits the
 * viewport"** (k = 1). That keeps the thresholds meaningful on any screen size:
 * "you see components once you have zoomed in about 1.6×", not "once the scale
 * is 0.0007". All three prototypes normalise their layout into the same world
 * box (see `lib/normalize.js`), so k means the same thing everywhere.
 *
 * Everything tunable lives in the two constants below, deliberately together
 * and apart from any rendering code (acceptance criterion 7).
 */
import { computed } from 'vue';
import { LEVELS } from '../lib/archIndex.js';

/**
 * Zoom factor at which each level takes over from the one above it, in
 * multiples of "fits the viewport". `container` needs no entry: it is what you
 * see below the first threshold.
 *
 * The ratio 20 / 803 / 1605 is why `code` sits so far out: it has ~20× the node
 * count of the level above it, so it must only appear once you are properly
 * zoomed in on one corner of the model.
 */
export const LEVEL_ZOOM_THRESHOLDS = Object.freeze({
  component: 1.25,
  code: 4.5,
});

/**
 * Half-width of the cross-fade band around a threshold, as a fraction of the
 * threshold. 0.25 means the fade runs from T/1.25 to T*1.25 — a couple of
 * mouse-wheel notches wide, which is enough to read as a dissolve rather than
 * a jump (acceptance criterion 8).
 */
export const LEVEL_FADE_BAND = 0.25;

/** Hard limits on the zoom factor, so you can neither lose nor bury the model. */
export const ZOOM_LIMITS = Object.freeze({ min: 0.4, max: 40 });

function smoothstep(t) {
  const x = Math.min(1, Math.max(0, t));
  return x * x * (3 - 2 * x);
}

/**
 * Where a zoom factor sits between two levels.
 *
 * @param {number} zoom  zoom factor, 1 = the whole model fits
 * @returns {{ base: string, next: string|null, t: number }}
 *   `base` is the level being faded out, `next` the one being faded in (null
 *   outside a transition band), and `t` ∈ [0,1] how far the transition is.
 *   Render `base` at alpha 1−t and `next` at alpha t.
 */
export function levelBlend(zoom) {
  const k = Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
  for (let i = 1; i < LEVELS.length; i += 1) {
    const level = LEVELS[i];
    const threshold = LEVEL_ZOOM_THRESHOLDS[level];
    const lo = threshold / (1 + LEVEL_FADE_BAND);
    const hi = threshold * (1 + LEVEL_FADE_BAND);
    if (k < lo) return { base: LEVELS[i - 1], next: null, t: 0 };
    if (k <= hi) {
      const t = smoothstep((k - lo) / (hi - lo));
      return { base: LEVELS[i - 1], next: level, t };
    }
  }
  return { base: LEVELS[LEVELS.length - 1], next: null, t: 0 };
}

/**
 * The single level a zoom factor reads as — the one carrying most of the
 * opacity. Used for labels, statistics and hit-testing, where a blend makes no
 * sense.
 */
export function levelForZoom(zoom) {
  const { base, next, t } = levelBlend(zoom);
  return next && t >= 0.5 ? next : base;
}

/** The zoom factor at which a level starts to appear (its fade-in lower edge). */
export function zoomForLevel(level) {
  const threshold = LEVEL_ZOOM_THRESHOLDS[level];
  if (threshold === undefined) return ZOOM_LIMITS.min;
  return threshold;
}

/** Reactive wrapper around the pure functions above. */
export function useSemanticZoom(zoomRef) {
  const blend = computed(() => levelBlend(zoomRef.value));
  const level = computed(() => levelForZoom(zoomRef.value));
  return { blend, level };
}

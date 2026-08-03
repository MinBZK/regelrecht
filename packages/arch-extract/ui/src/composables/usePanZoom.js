/**
 * usePanZoom — pointer-anchored pan/zoom over the shared world box.
 *
 * The three prototypes draw on a canvas rather than through Vue Flow, so they
 * need their own view transform. It is deliberately the same one for all three,
 * because two acceptance criteria live in it:
 *
 *  - **the point under the cursor stays put while zooming** (criterion 9). The
 *    wheel handler converts the cursor to world coordinates *before* changing
 *    the scale and re-derives the translation afterwards, so the world point
 *    under the pointer is invariant. Because every layout is normalised into
 *    the same world box, that also holds *across* a level change: switching
 *    from components to code does not touch the transform at all.
 *  - **the zoom factor is comparable** (criterion 7): `zoom` is reported
 *    relative to the fit-to-viewport scale, so `zoom === 1` means "the whole
 *    model is in view" on any screen.
 */
import { computed, ref } from 'vue';
import { ZOOM_LIMITS } from './useSemanticZoom.js';

/** Wheel sensitivity: one notch (deltaY ≈ 120) is ~1.18×. */
const WHEEL_INTENSITY = 0.0014;

/** See `fit()`: how much of the viewport stays empty at zoom = 1. */
const FIT_PADDING = 0.22;

export function usePanZoom() {
  const scale = ref(1); // world unit → screen px
  const tx = ref(0);
  const ty = ref(0);
  const fitScale = ref(1); // the scale at which the whole model fits

  /** Zoom factor relative to "everything fits". */
  const zoom = computed(() => scale.value / (fitScale.value || 1));

  function toWorld(px, py) {
    return { x: (px - tx.value) / scale.value, y: (py - ty.value) / scale.value };
  }

  function toScreen(wx, wy) {
    return { x: wx * scale.value + tx.value, y: wy * scale.value + ty.value };
  }

  /**
   * Frame `bounds` in a `width`×`height` viewport and make that zoom = 1.
   *
   * The generous default padding is deliberate: it buys the headroom the
   * semantic zoom needs. At zoom = 1 the model fills ~78% of the viewport, so
   * you can still zoom past the container→component threshold (1.25×, see
   * useSemanticZoom.js) with the whole thing on screen. Without that headroom
   * the finer levels could only ever be looked at through a keyhole.
   */
  function fit(bounds, width, height, padding = FIT_PADDING) {
    if (!width || !height) return;
    const w = Math.max(1e-6, bounds.maxX - bounds.minX);
    const h = Math.max(1e-6, bounds.maxY - bounds.minY);
    const s = Math.min(width / w, height / h) * (1 - padding);
    fitScale.value = s;
    scale.value = s;
    tx.value = width / 2 - ((bounds.minX + bounds.maxX) / 2) * s;
    ty.value = height / 2 - ((bounds.minY + bounds.maxY) / 2) * s;
  }

  /** Re-fit after a resize without losing the current zoom factor. */
  function refit(bounds, width, height, padding = FIT_PADDING) {
    const keep = zoom.value;
    fit(bounds, width, height, padding);
    if (Number.isFinite(keep) && keep > 0 && Math.abs(keep - 1) > 1e-6) {
      zoomAt(width / 2, height / 2, keep);
    }
  }

  /** Multiply the scale by `factor`, keeping the screen point (px, py) fixed. */
  function zoomAt(px, py, factor) {
    const world = toWorld(px, py);
    const min = fitScale.value * ZOOM_LIMITS.min;
    const max = fitScale.value * ZOOM_LIMITS.max;
    const next = Math.min(max, Math.max(min, scale.value * factor));
    scale.value = next;
    tx.value = px - world.x * next;
    ty.value = py - world.y * next;
  }

  /** Wheel → zoom, anchored on the cursor. */
  function wheelZoom(px, py, deltaY) {
    zoomAt(px, py, Math.exp(-deltaY * WHEEL_INTENSITY));
  }

  function panBy(dx, dy) {
    tx.value += dx;
    ty.value += dy;
  }

  /** Centre the view on a world point without changing the zoom. */
  function centreOn(wx, wy, width, height) {
    tx.value = width / 2 - wx * scale.value;
    ty.value = height / 2 - wy * scale.value;
  }

  return { scale, tx, ty, fitScale, zoom, toWorld, toScreen, fit, refit, zoomAt, wheelZoom, panBy, centreOn };
}

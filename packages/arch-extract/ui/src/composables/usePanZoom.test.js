import { describe, it, expect } from 'vitest';
import { usePanZoom } from './usePanZoom.js';
import { ZOOM_LIMITS } from './useSemanticZoom.js';

const BOUNDS = { minX: -500, minY: -500, maxX: 500, maxY: 500 };
const W = 1600;
const H = 900;

describe('usePanZoom', () => {
  it('makes "the whole model fits" zoom factor 1', () => {
    const pz = usePanZoom();
    pz.fit(BOUNDS, W, H);
    expect(pz.zoom.value).toBeCloseTo(1, 6);
    // The centre of the bounds lands in the centre of the viewport.
    const c = pz.toScreen(0, 0);
    expect(c.x).toBeCloseTo(W / 2, 6);
    expect(c.y).toBeCloseTo(H / 2, 6);
  });

  it('leaves room to zoom past the first level threshold with everything on screen', () => {
    const pz = usePanZoom();
    pz.fit(BOUNDS, W, H);
    // At zoom 1 the model occupies less than the viewport, so the fine levels
    // are reachable without cropping. (See the fit padding in usePanZoom.js.)
    const height = (BOUNDS.maxY - BOUNDS.minY) * pz.scale.value;
    expect(height).toBeLessThan(H * 0.85);
  });

  it('keeps the point under the cursor fixed while zooming (criterion 9)', () => {
    const pz = usePanZoom();
    pz.fit(BOUNDS, W, H);

    for (const [px, py] of [
      [100, 80],
      [W / 2, H / 2],
      [W - 40, H - 30],
    ]) {
      const before = pz.toWorld(px, py);
      pz.wheelZoom(px, py, -120);
      pz.wheelZoom(px, py, -120);
      pz.wheelZoom(px, py, -120);
      const after = pz.toWorld(px, py);
      expect(after.x).toBeCloseTo(before.x, 6);
      expect(after.y).toBeCloseTo(before.y, 6);
    }
  });

  it('holds that invariant across a level change, because every level shares one world box', () => {
    const pz = usePanZoom();
    pz.fit(BOUNDS, W, H);
    const px = 900;
    const py = 300;
    const before = pz.toWorld(px, py);
    // Scroll far enough to cross both thresholds. Nothing in the transform
    // depends on the level, so the world point under the cursor cannot move.
    for (let i = 0; i < 14; i += 1) pz.wheelZoom(px, py, -120);
    const after = pz.toWorld(px, py);
    expect(after.x).toBeCloseTo(before.x, 6);
    expect(after.y).toBeCloseTo(before.y, 6);
    expect(pz.zoom.value).toBeGreaterThan(4.5);
  });

  it('clamps the zoom factor to its limits', () => {
    const pz = usePanZoom();
    pz.fit(BOUNDS, W, H);
    for (let i = 0; i < 200; i += 1) pz.wheelZoom(10, 10, -120);
    expect(pz.zoom.value).toBeCloseTo(ZOOM_LIMITS.max, 6);
    for (let i = 0; i < 400; i += 1) pz.wheelZoom(10, 10, 120);
    expect(pz.zoom.value).toBeCloseTo(ZOOM_LIMITS.min, 6);
  });

  it('panning moves the world under the viewport, not the zoom factor', () => {
    const pz = usePanZoom();
    pz.fit(BOUNDS, W, H);
    const zoom = pz.zoom.value;
    const before = pz.toWorld(400, 400);
    pz.panBy(120, -60);
    const after = pz.toWorld(400, 400);
    expect(pz.zoom.value).toBe(zoom);
    expect(after.x).toBeCloseTo(before.x - 120 / pz.scale.value, 6);
    expect(after.y).toBeCloseTo(before.y + 60 / pz.scale.value, 6);
  });
});

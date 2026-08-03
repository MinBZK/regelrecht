import { describe, it, expect } from 'vitest';
import {
  LEVEL_FADE_BAND,
  LEVEL_ZOOM_THRESHOLDS,
  levelBlend,
  levelForZoom,
} from './useSemanticZoom.js';

const T_COMPONENT = LEVEL_ZOOM_THRESHOLDS.component;
const T_CODE = LEVEL_ZOOM_THRESHOLDS.code;
const below = (t) => t / (1 + LEVEL_FADE_BAND) / 1.05;
const above = (t) => t * (1 + LEVEL_FADE_BAND) * 1.05;

describe('levelForZoom', () => {
  it('walks container → component → code as you zoom in', () => {
    expect(levelForZoom(below(T_COMPONENT))).toBe('container');
    expect(levelForZoom(above(T_COMPONENT))).toBe('component');
    expect(levelForZoom(below(T_CODE))).toBe('component');
    expect(levelForZoom(above(T_CODE))).toBe('code');
  });

  it('is monotonic — zooming in never goes back to a coarser level', () => {
    const order = ['container', 'component', 'code'];
    let seen = 0;
    for (let k = 0.4; k < 40; k *= 1.05) {
      const rank = order.indexOf(levelForZoom(k));
      expect(rank).toBeGreaterThanOrEqual(seen);
      seen = rank;
    }
    expect(seen).toBe(2);
  });

  it('falls back to the coarsest level for a nonsensical zoom', () => {
    expect(levelForZoom(0)).toBe('container');
    expect(levelForZoom(NaN)).toBe('container');
    expect(levelForZoom(-3)).toBe('container');
  });
});

describe('levelBlend', () => {
  it('has no transition outside the fade bands', () => {
    expect(levelBlend(below(T_COMPONENT))).toEqual({ base: 'container', next: null, t: 0 });
    expect(levelBlend(above(T_CODE))).toEqual({ base: 'code', next: null, t: 0 });
  });

  it('cross-fades between two levels around a threshold instead of jumping', () => {
    const at = levelBlend(T_COMPONENT);
    expect(at.base).toBe('container');
    expect(at.next).toBe('component');
    expect(at.t).toBeGreaterThan(0.2);
    expect(at.t).toBeLessThan(0.8);
  });

  it('ramps t continuously from 0 to 1 across the band', () => {
    const lo = T_COMPONENT / (1 + LEVEL_FADE_BAND);
    const hi = T_COMPONENT * (1 + LEVEL_FADE_BAND);
    let prev = -1;
    for (let i = 0; i <= 20; i += 1) {
      const { t } = levelBlend(lo + ((hi - lo) * i) / 20);
      expect(t).toBeGreaterThanOrEqual(prev);
      prev = t;
    }
    expect(prev).toBeCloseTo(1, 5);
  });
});

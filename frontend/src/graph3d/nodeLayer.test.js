import { describe, it, expect } from 'vitest';
import { encodePickId, decodePickId, weightScale, lodForCount } from './nodeLayer.js';

describe('GPU pick ids', () => {
  it('round-trips every index it can encode', () => {
    for (const i of [0, 1, 255, 256, 65535, 65536, 1 << 20, (1 << 24) - 2]) {
      const [r, g, b] = encodePickId(i);
      expect(decodePickId(Math.round(r * 255), Math.round(g * 255), Math.round(b * 255))).toBe(i);
    }
  });

  it('reads black as the background, not as node zero', () => {
    expect(decodePickId(0, 0, 0)).toBe(-1);
    expect(encodePickId(0)).toEqual([0, 0, 1 / 255]);
  });
});

describe('weightScale', () => {
  it('caps the ratio between the biggest and smallest node at 4:1', () => {
    expect(weightScale(1, 1, 1000)).toBeCloseTo(1, 5);
    expect(weightScale(1000, 1, 1000)).toBeCloseTo(4, 5);
    const mid = weightScale(30, 1, 1000);
    expect(mid).toBeGreaterThan(1);
    expect(mid).toBeLessThan(4);
  });

  it('is logarithmic, so a tenfold weight is not a tenfold radius', () => {
    const a = weightScale(10, 1, 10000);
    const b = weightScale(100, 1, 10000);
    expect(b / a).toBeLessThan(2);
  });

  it('degrades to a single size when every weight is equal', () => {
    expect(weightScale(5, 5, 5)).toBe(1);
  });

  it('clamps out-of-range weights instead of producing giants', () => {
    expect(weightScale(1e9, 1, 100)).toBe(4);
    expect(weightScale(-5, 1, 100)).toBeGreaterThanOrEqual(1);
  });
});

describe('lodForCount', () => {
  it('drops geometry detail as the corpus grows', () => {
    expect(lodForCount(4138)).toBe('high');
    expect(lodForCount(25000)).toBe('mid');
    expect(lodForCount(150000)).toBe('low');
  });
});

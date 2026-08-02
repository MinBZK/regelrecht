import { describe, it, expect } from 'vitest';
import { edt1d, edt2d, sdfFromAlpha } from './sdf.js';

describe('edt1d', () => {
  it('gives the squared distance to the nearest zero', () => {
    const n = 5;
    const f = new Float64Array([1e20, 1e20, 0, 1e20, 1e20]);
    const d = new Float64Array(n);
    edt1d(f, d, new Int32Array(n), new Float64Array(n + 1), n);
    expect(Array.from(d)).toEqual([4, 1, 0, 1, 4]);
  });
});

describe('edt2d', () => {
  it('is symmetric around a single seed pixel', () => {
    const w = 5;
    const h = 5;
    const grid = new Float64Array(w * h).fill(1e20);
    grid[2 * w + 2] = 0;
    edt2d(grid, w, h);
    expect(grid[2 * w + 2]).toBe(0);
    expect(grid[2 * w + 1]).toBe(1);
    expect(grid[1 * w + 1]).toBe(2);
    expect(grid[0]).toBe(8);
  });
});

describe('sdfFromAlpha', () => {
  it('puts the glyph edge at the midpoint of the range', () => {
    const w = 16;
    const h = 16;
    const alpha = new Uint8Array(w * h);
    // Solid 8x8 block in the middle.
    for (let y = 4; y < 12; y++) {
      for (let x = 4; x < 12; x++) alpha[y * w + x] = 255;
    }
    const sdf = sdfFromAlpha(alpha, w, h, 4);

    const inside = sdf[8 * w + 8];
    const outside = sdf[0];
    const justInside = sdf[4 * w + 8];
    const justOutside = sdf[3 * w + 8];

    expect(inside).toBeGreaterThan(200);
    expect(outside).toBeLessThan(50);
    // The transition straddles the 128 midpoint within a pixel.
    expect(justInside).toBeGreaterThanOrEqual(128);
    expect(justOutside).toBeLessThan(128);
    expect(justInside - justOutside).toBeLessThan(80);
  });

  it('stays at the floor for an empty glyph', () => {
    const sdf = sdfFromAlpha(new Uint8Array(64), 8, 8, 4);
    expect(Math.max(...sdf)).toBe(0);
  });
});

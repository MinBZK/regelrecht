import { describe, it, expect } from 'vitest';
import {
  encodePickId,
  decodePickId,
  nearestNeighbourSpacing,
  weightScale,
  lodForCount,
  WEIGHT_SCALE_MAX,
  WEIGHT_SCALE_MIN,
} from './nodeLayer.js';

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
  it('stays inside the range the base size was calibrated for', () => {
    expect(weightScale(1, 1, 1000)).toBeCloseTo(WEIGHT_SCALE_MIN, 5);
    expect(weightScale(1000, 1, 1000)).toBeCloseTo(WEIGHT_SCALE_MAX, 5);
    const mid = weightScale(30, 1, 1000);
    expect(mid).toBeGreaterThan(WEIGHT_SCALE_MIN);
    expect(mid).toBeLessThan(WEIGHT_SCALE_MAX);
  });

  it('modulates around the base size instead of only multiplying it', () => {
    // The heaviest law may be bigger than the base size, the lightest has to be
    // smaller: a range that starts at 1 pushes every node past the spacing the
    // base size was fitted to, which is how the corpus became one solid body.
    expect(WEIGHT_SCALE_MIN).toBeLessThan(1);
    expect(WEIGHT_SCALE_MAX).toBeGreaterThan(1);
    // And under 3:1, so a heavyweight never swallows its neighbours.
    expect(WEIGHT_SCALE_MAX / WEIGHT_SCALE_MIN).toBeLessThan(3);
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
    expect(weightScale(1e9, 1, 100)).toBe(WEIGHT_SCALE_MAX);
    expect(weightScale(-5, 1, 100)).toBeGreaterThanOrEqual(WEIGHT_SCALE_MIN);
  });
});

describe('nearestNeighbourSpacing', () => {
  /** A cubic lattice with a known neighbour distance, in a given box. */
  function lattice(side, step, origin = 0) {
    const n = side ** 3;
    const p = new Float32Array(n * 3);
    let k = 0;
    for (let x = 0; x < side; x++) {
      for (let y = 0; y < side; y++) {
        for (let z = 0; z < side; z++) {
          p[k++] = origin + x * step;
          p[k++] = origin + y * step;
          p[k++] = origin + z * step;
        }
      }
    }
    return { positions: p, nodeCount: n };
  }

  it('finds the lattice step, whatever the units are', () => {
    const small = lattice(10, 1);
    expect(nearestNeighbourSpacing(small.positions, small.nodeCount)).toBeCloseTo(1, 5);
    const large = lattice(10, 1000);
    expect(nearestNeighbourSpacing(large.positions, large.nodeCount)).toBeCloseTo(1000, 2);
  });

  it('ignores a stray far outside the corpus, which the bounding box cannot', () => {
    // This is the whole point of the measurement. One framework law thrown ten
    // corpus-widths out doubles the bounding radius and therefore doubles the
    // node size under the old rule, while the nodes it has to fit between have
    // not moved a millimetre.
    const { positions, nodeCount } = lattice(10, 1);
    const withStray = new Float32Array((nodeCount + 1) * 3);
    withStray.set(positions);
    withStray[nodeCount * 3] = 500;
    withStray[nodeCount * 3 + 1] = 500;
    withStray[nodeCount * 3 + 2] = 500;
    expect(nearestNeighbourSpacing(withStray, nodeCount + 1)).toBeCloseTo(1, 5);
  });

  it('follows the dense part when the layout has two densities', () => {
    // A dense core with a sparse halo: the size has to fit the core, because
    // that is where nodes touch.
    const dense = lattice(12, 1);
    const sparse = lattice(6, 20, 200);
    const total = dense.nodeCount + sparse.nodeCount;
    const p = new Float32Array(total * 3);
    p.set(dense.positions);
    p.set(sparse.positions, dense.nodeCount * 3);
    const spacing = nearestNeighbourSpacing(p, total);
    expect(spacing).toBeGreaterThan(0.5);
    expect(spacing).toBeLessThan(2);
  });

  it('says nothing rather than something wrong about a degenerate layout', () => {
    expect(nearestNeighbourSpacing(new Float32Array(3), 1)).toBe(0);
    expect(nearestNeighbourSpacing(new Float32Array(9), 3)).toBe(0); // all on one point
  });
});

describe('lodForCount', () => {
  it('drops geometry detail as the corpus grows', () => {
    expect(lodForCount(4138)).toBe('high');
    expect(lodForCount(25000)).toBe('mid');
    expect(lodForCount(150000)).toBe('low');
  });
});

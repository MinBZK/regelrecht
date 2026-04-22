import { describe, it, expect } from 'vitest';
import { generatePopulation, histogram } from './population.js';

describe('generatePopulation', () => {
  it('produces the requested number of people', () => {
    const pop = generatePopulation({ size: 50, calculationDate: '2025-01-01' });
    expect(pop).toHaveLength(50);
  });

  it('is deterministic for the same seed', () => {
    const a = generatePopulation({ size: 10, seed: 7, calculationDate: '2025-01-01' });
    const b = generatePopulation({ size: 10, seed: 7, calculationDate: '2025-01-01' });
    expect(a).toEqual(b);
  });

  it('generates BSNs and plausible ages', () => {
    const pop = generatePopulation({ size: 100, calculationDate: '2025-06-01' });
    for (const p of pop) {
      expect(p.bsn).toMatch(/^\d{9}$/);
      expect(p.age).toBeGreaterThanOrEqual(18);
      expect(p.age).toBeLessThan(90);
      expect(p.inkomen).toBeGreaterThanOrEqual(0);
      expect(p.vermogen).toBeGreaterThanOrEqual(0);
    }
  });
});

describe('histogram', () => {
  it('handles empty input', () => {
    expect(histogram([]).buckets).toEqual([]);
  });

  it('sums counts to input length', () => {
    const h = histogram([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 5);
    const total = h.buckets.reduce((s, b) => s + b.count, 0);
    expect(total).toBe(10);
  });
});

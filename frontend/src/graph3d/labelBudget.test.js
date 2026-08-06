import { describe, it, expect } from 'vitest';
import { selectLabels, weightOrder } from './labelLayer.js';
import { layoutLabel } from './sdfAtlas.js';

describe('weightOrder', () => {
  it('sorts node indices by descending weight', () => {
    const order = weightOrder(new Float32Array([1, 9, 5, 7]));
    expect(Array.from(order)).toEqual([1, 3, 2, 0]);
  });
});

describe('selectLabels', () => {
  const order = Int32Array.from([4, 3, 2, 1, 0]);
  const all = () => true;

  it('never exceeds the budget', () => {
    expect(selectLabels(order, all, 2)).toEqual([4, 3]);
    expect(selectLabels(order, all, 99)).toHaveLength(5);
  });

  it('skips nodes the visibility test rejects', () => {
    const evenOnly = (i) => i % 2 === 0;
    expect(selectLabels(order, evenOnly, 3)).toEqual([4, 2, 0]);
  });

  it('pins the selection ahead of heavier neighbours', () => {
    expect(selectLabels(order, all, 2, [0])).toEqual([0, 4]);
  });

  it('does not label a pinned node twice', () => {
    expect(selectLabels(order, all, 3, [3, 3])).toEqual([3, 4, 2]);
  });

  it('honours a pin even when the node fails the visibility test', () => {
    // The selected node keeps its label when it scrolls out of frustum-based
    // visibility; otherwise the label of the thing you just clicked vanishes.
    expect(selectLabels(order, () => false, 3, [1])).toEqual([1]);
  });
});

describe('layoutLabel', () => {
  const glyphs = new Map([
    ['a', { advance: 0.5 }],
    ['b', { advance: 0.5 }],
    ['…', { advance: 0.4 }],
  ]);

  it('advances one glyph at a time', () => {
    const { quads, width } = layoutLabel('ab', glyphs);
    expect(quads.map((q) => q.x)).toEqual([0, 0.5]);
    expect(width).toBe(1);
  });

  it('truncates with an ellipsis past the maximum width', () => {
    const { quads } = layoutLabel('aaaa', glyphs, 1.4);
    expect(quads[quads.length - 1].glyph).toBe(glyphs.get('…'));
    expect(quads.length).toBeLessThan(5);
  });

  it('ignores characters that are not in the atlas', () => {
    const { quads } = layoutLabel('a☃b', glyphs);
    expect(quads).toHaveLength(2);
  });
});

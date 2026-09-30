// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { toRgb } from './cssColor.js';

/**
 * A fake 2d context that behaves like a canvas for the cases that matter:
 * it knows a few colours, ignores an unknown one (keeping the previous
 * fillStyle, as a real canvas does) and reports the painted pixel.
 */
function fakeCanvas(known) {
  const ctx = {
    _fill: '#000000',
    _pixel: [0, 0, 0, 0],
    set fillStyle(v) {
      if (v === 'transparent' || v in known) this._fill = v;
    },
    get fillStyle() {
      return this._fill;
    },
    clearRect() {
      this._pixel = [0, 0, 0, 0];
    },
    fillRect() {
      this._pixel = this._fill === 'transparent' ? [0, 0, 0, 0] : known[this._fill];
    },
    getImageData() {
      return { data: this._pixel };
    },
  };
  const create = document.createElement.bind(document);
  vi.spyOn(document, 'createElement').mockImplementation((tag) =>
    tag === 'canvas' ? { getContext: () => ctx } : create(tag),
  );
}

afterEach(() => vi.restoreAllMocks());

describe('toRgb', () => {
  it('turns an oklch colour into rgb for mermaid', () => {
    fakeCanvas({ 'oklch(1 0 0)': [255, 255, 255, 255], 'oklch(0.39 0.1 255)': [21, 66, 115, 255] });
    expect(toRgb('oklch(1 0 0)')).toBe('rgb(255, 255, 255)');
    expect(toRgb('oklch(0.39 0.1 255)')).toBe('rgb(21, 66, 115)');
  });

  it('does not return the previous colour for one the canvas rejects', () => {
    fakeCanvas({ 'oklch(1 0 0)': [255, 255, 255, 255] });
    expect(toRgb('oklch(1 0 0)')).toBe('rgb(255, 255, 255)');
    expect(toRgb('geen-kleur')).toBeNull();
  });

  it('returns null without a canvas, so the caller falls back to its hex', () => {
    const create = document.createElement.bind(document);
    vi.spyOn(document, 'createElement').mockImplementation((tag) => (tag === 'canvas' ? { getContext: () => null } : create(tag)));
    expect(toRgb('oklch(1 0 0)')).toBeNull();
    expect(toRgb('oklch(1 0 0)') || '#ffffff').toBe('#ffffff');
  });
});

import { describe, it, expect } from 'vitest';
import { readPalette, nodeColor } from './palette.js';

/** How far a colour sits from the background, summed over the channels. */
function distance(color, background) {
  const ch = (c) => [(c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff];
  const [ar, ag, ab] = ch(color);
  const [br, bg, bb] = ch(background);
  return Math.abs(ar - br) + Math.abs(ag - bg) + Math.abs(ab - bb);
}

describe('graph palette', () => {
  it('draws lines weaker than nodes', () => {
    // The corpus is grey by rule - grey is not one state but the whole map -
    // so the distinction between a thing and a connection has to come from
    // inside that grey. There are seven citations per law, and at equal
    // strength the lines win on count alone and the picture is one mass.
    const p = readPalette();
    expect(distance(p.edge, p.background)).toBeLessThan(distance(p.grey, p.background));
    // Nearly every edge is a citation, and that is the one drawn through the
    // per-edge colour buffer rather than the material, so it has to be weak too.
    expect(p.edgeTypes[0]).toBe(p.edge);
  });

  it('keeps the framework laws the strongest of the greys', () => {
    const p = readPalette();
    const framework = nodeColor(p, 0, 0, 1);
    const ordinary = nodeColor(p, 0, 0, 0);
    expect(distance(framework, p.background)).toBeGreaterThan(distance(ordinary, p.background));
    expect(distance(ordinary, p.background)).toBeGreaterThan(distance(p.edge, p.background));
  });
});

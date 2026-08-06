import { describe, it, expect } from 'vitest';
import { labelOrder } from './GraphScene.js';
import { isEnriched, STATUS_IDS } from './graphSchema.js';
import { nodeColor, readPalette } from './palette.js';

/**
 * The colour rule: grey is everything that has only been harvested, colour
 * means somebody has enriched this law. These tests pin that rule, because it
 * is a decision about what the map says and not a rendering detail.
 */
describe('enrichment colour rule', () => {
  const palette = readPalette(null);

  it('paints harvested laws grey, whatever cluster they are in', () => {
    const a = nodeColor(palette, 0, STATUS_IDS.harvested);
    const b = nodeColor(palette, 3, STATUS_IDS.harvested);
    expect(a).toBe(b);
    expect(a).toBe(palette.grey);
  });

  it('gives framework laws a darker grey, still no hue', () => {
    const framework = nodeColor(palette, 0, STATUS_IDS.harvested, 1);
    expect(framework).toBe(palette.greyDeep);
    expect(framework).not.toBe(palette.grey);
  });

  it('spends colour only once something has been enriched', () => {
    const enriched = nodeColor(palette, 0, STATUS_IDS.enriched);
    const validated = nodeColor(palette, 0, STATUS_IDS.validated);
    expect(enriched).not.toBe(palette.grey);
    expect(validated).not.toBe(palette.grey);
    expect(enriched).not.toBe(validated);
    // Two enriched laws in different clusters differ; that is the second
    // distinction, and it only exists inside the coloured set.
    expect(nodeColor(palette, 1, STATUS_IDS.enriched)).not.toBe(enriched);
  });

  it('marks a law the enricher is working on with the attention colour', () => {
    expect(nodeColor(palette, 2, STATUS_IDS.enriching)).toBe(palette.active);
  });

  it('counts every coloured status as interactive and grey as not', () => {
    expect(isEnriched(STATUS_IDS.harvested)).toBe(false);
    expect(isEnriched(STATUS_IDS.enriched)).toBe(true);
    expect(isEnriched(STATUS_IDS.validated)).toBe(true);
    expect(isEnriched(STATUS_IDS.enriching)).toBe(true);
  });
});

describe('labelOrder', () => {
  const graph = {
    nodeCount: 5,
    weight: Float32Array.from([100, 1, 50, 2, 3]),
    status: Uint8Array.from([0, 1, 0, 2, 0]),
  };

  it('labels enriched laws before heavier grey ones', () => {
    // Node 0 is by far the heaviest but grey; nodes 3 and 1 are enriched.
    expect(Array.from(labelOrder(graph))).toEqual([3, 1, 0, 2, 4]);
  });

  it('falls back to pure weight when nothing is enriched', () => {
    const grey = { ...graph, status: new Uint8Array(5) };
    expect(Array.from(labelOrder(grey))).toEqual([0, 2, 4, 3, 1]);
  });
});

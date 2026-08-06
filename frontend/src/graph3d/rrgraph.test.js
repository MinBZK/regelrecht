import { describe, it, expect } from 'vitest';
import { decodeRrgraph, readHeader, familyFor, enrichmentStatus } from './rrgraph.js';
import { KIND_IDS, STATUS_IDS } from './graphSchema.js';

/**
 * Build a payload by hand, exactly as `packages/graph/src/payload.rs`
 * describes it. Writing the encoder in the test rather than checking in a
 * fixture keeps the contract visible: if the format changes, this is where the
 * disagreement shows up.
 */
function encode(header, sections) {
  const enc = new TextEncoder();
  const meta = [];
  let offset = 0;
  const chunks = [];
  for (const [name, type, array] of sections) {
    const bytes = array.byteLength;
    meta.push({ name, type, len: array.length, offset, bytes });
    chunks.push(new Uint8Array(array.buffer, array.byteOffset, bytes));
    offset += bytes;
    const pad = (8 - (offset % 8)) % 8;
    if (pad) {
      chunks.push(new Uint8Array(pad));
      offset += pad;
    }
  }
  // The header's own length depends on `data_offset`, which depends on the
  // header's length. Two passes settle it; the padding to a multiple of eight
  // absorbs the last digit of difference.
  const headerStart = 12;
  let dataOffset = 0;
  let finalHeader = enc.encode(JSON.stringify({ ...header, sections: meta, data_offset: 0 }));
  for (let pass = 0; pass < 3; pass++) {
    dataOffset = headerStart + finalHeader.length;
    dataOffset += (8 - (dataOffset % 8)) % 8;
    finalHeader = enc.encode(JSON.stringify({ ...header, sections: meta, data_offset: dataOffset }));
    if (headerStart + finalHeader.length <= dataOffset) break;
  }
  const buf = new ArrayBuffer(dataOffset + offset);
  const bytes = new Uint8Array(buf);
  bytes.set(enc.encode('RRGRAPH'), 0);
  bytes[7] = 1;
  new DataView(buf).setUint32(8, finalHeader.length, true);
  bytes.set(finalHeader, headerStart);
  let cursor = dataOffset;
  for (const chunk of chunks) {
    bytes.set(chunk, cursor);
    cursor += chunk.length;
  }
  return buf;
}

// Two laws and one article of the first law. The article sits at (1, 0, 0)
// relative to its parent, and a self-reference hangs off law 0.
function samplePayload() {
  const header = {
    format: 'rrgraph',
    version: 1,
    snapshot_id: 'test',
    layout_version: 'spectral-fa2/1',
    node_count: 3,
    law_node_count: 2,
    edge_count: 3,
    law_edge_count: 1,
    kinds: ['law', 'article', 'external', 'expected'],
    edge_types: ['citation', 'source', 'delegation'],
    layers: ['GRONDWET', 'WET', 'AMVB'],
    clusters: 2,
    framework_cluster: 65535,
    enrichment_states: ['none', 'partial', 'full'],
    strings: ['wet_a', 'Wet A', 'wet_b', 'Wet B', 'wet_a:art1', 'Artikel 1'],
  };
  return encode(header, [
    ['node_pos', 'f32', Float32Array.from([10, 0, 0, 20, 0, 0, 1, 0, 0])],
    ['node_id', 'u32', Uint32Array.from([0, 2, 4])],
    ['node_label', 'u32', Uint32Array.from([1, 3, 5])],
    ['node_kind', 'u8', Uint8Array.from([0, 0, 1])],
    ['node_layer', 'u8', Uint8Array.from([1, 2, 1])],
    ['node_weight', 'f32', Float32Array.from([5, 1, 0])],
    ['node_cluster', 'u16', Uint16Array.from([1, 65535, 1])],
    ['node_parent', 'u32', Uint32Array.from([0xffffffff, 0xffffffff, 0])],
    ['node_flags', 'u8', Uint8Array.from([0, 1, 0])],
    ['edge_src', 'u32', Uint32Array.from([0, 0, 2])],
    ['edge_dst', 'u32', Uint32Array.from([1, 0, 1])],
    ['edge_type', 'u8', Uint8Array.from([1, 1, 2])],
    ['edge_count', 'u32', Uint32Array.from([3, 1, 1])],
    ['node_enrichment', 'u8', Uint8Array.from([1, 0, 2])],
    ['node_activity', 'u8', Uint8Array.from([0, 1, 0])],
    ['node_articles', 'u32', Uint32Array.from([10, 0, 0])],
    ['node_articles_enriched', 'u32', Uint32Array.from([6, 0, 0])],
  ]);
}

describe('readHeader', () => {
  it('rejects anything that is not a payload', () => {
    expect(() => readHeader(new ArrayBuffer(32))).toThrow(/magic/);
  });

  it('reads the metadata without touching the sections', () => {
    const header = readHeader(samplePayload());
    expect(header.format).toBe('rrgraph');
    expect(header.node_count).toBe(3);
    expect(header.law_node_count).toBe(2);
  });
});

describe('decodeRrgraph', () => {
  it('resolves the string table into ids and labels', () => {
    const g = decodeRrgraph(samplePayload());
    expect(g.ids).toEqual(['wet_a', 'wet_b', 'wet_a:art1']);
    expect(g.labels).toEqual(['Wet A', 'Wet B', 'Artikel 1']);
  });

  it('turns article coordinates into world coordinates', () => {
    const g = decodeRrgraph(samplePayload());
    // Article 1 of law A: stored as (1, 0, 0) relative to a law at (10, 0, 0).
    expect(Array.from(g.positions.slice(6, 9))).toEqual([11, 0, 0]);
    expect(Array.from(g.positions.slice(0, 3))).toEqual([10, 0, 0]);
  });

  it('drops self-references and counts them on the node', () => {
    const g = decodeRrgraph(samplePayload());
    expect(g.edgeCount).toBe(2);
    expect(g.selfRefs[0]).toBe(1);
    expect(Array.from(g.edgeSource)).toEqual([0, 2]);
  });

  it('keeps only the law block when asked, edges included', () => {
    const g = decodeRrgraph(samplePayload(), { lawLevelOnly: true });
    expect(g.nodeCount).toBe(2);
    // Only the law-level edge block is read, so the article edge stays out and
    // no endpoint can point past the loaded nodes.
    expect(g.edgeCount).toBe(1);
    expect(Array.from(g.edgeSource)).toEqual([0]);
    expect(Array.from(g.edgeTarget)).toEqual([1]);
  });

  it('marks framework laws and keeps their cluster out of the hue range', () => {
    const g = decodeRrgraph(samplePayload());
    expect(Array.from(g.framework)).toEqual([0, 1, 0]);
    expect(g.cluster[1]).toBe(0);
    expect(g.cluster[0]).toBe(1);
  });

  it('reads the enrichment status, with activity winning over it', () => {
    const g = decodeRrgraph(samplePayload());
    // Law A is partial, law B is untouched but has the enricher in it right
    // now, the article is fully modelled.
    expect(Array.from(g.status)).toEqual([
      STATUS_IDS.enriched,
      STATUS_IDS.enriching,
      STATUS_IDS.validated,
    ]);
    expect(g.articles[0]).toBe(10);
    expect(g.articlesEnriched[0]).toBe(6);
  });

  it('rejects a future payload version instead of misreading it', () => {
    const buf = samplePayload();
    const header = readHeader(buf);
    const bad = new TextEncoder().encode(JSON.stringify({ ...header, version: 2 }));
    const copy = new ArrayBuffer(buf.byteLength + bad.length);
    new Uint8Array(copy).set(new Uint8Array(buf));
    new Uint8Array(copy).set(bad, 12);
    new DataView(copy).setUint32(8, bad.length, true);
    expect(() => decodeRrgraph(copy)).toThrow(/payloadversie/);
  });
});

describe('familyFor', () => {
  it('gives articles, placeholders and layers their own silhouette', () => {
    expect(familyFor('article', 'WET')).toBe(KIND_IDS.artikel);
    expect(familyFor('expected', 'MINISTERIELE_REGELING')).toBe(KIND_IDS.beleidsregel);
    expect(familyFor('law', 'AMVB')).toBe(KIND_IDS.amvb);
    expect(familyFor('law', 'MINISTERIELE_REGELING')).toBe(KIND_IDS.ministeriele_regeling);
    expect(familyFor('law', 'WET')).toBe(KIND_IDS.law);
    expect(familyFor('law', 'ONBEKEND')).toBe(KIND_IDS.law);
  });
});

describe('enrichmentStatus', () => {
  it('keeps grey for untouched laws and colour for the rest', () => {
    expect(enrichmentStatus('none')).toBe(STATUS_IDS.harvested);
    expect(enrichmentStatus('partial')).toBe(STATUS_IDS.enriched);
    expect(enrichmentStatus('full')).toBe(STATUS_IDS.validated);
    // An unknown state from a newer builder stays grey rather than claiming
    // work that may not have happened.
    expect(enrichmentStatus('iets-nieuws')).toBe(STATUS_IDS.harvested);
    expect(enrichmentStatus(undefined)).toBe(STATUS_IDS.harvested);
  });
});

import { describe, it, expect } from 'vitest';
import {
  buildAlignment,
  spanToNodeSlices,
  cpToUtf16,
} from './useNotesHighlight.js';

// buildAlignment + spanToNodeSlices are the highest-risk pure functions in
// the markdown-highlight path (#646): they re-anchor resolver char-offsets
// (into the raw law text) onto the rendered DOM, which has list prefixes and
// some whitespace stripped. These tests pin the divergence cases.

/** Fake text node: identity-comparable, carries its string. */
function tn(text) {
  return { __t: text, textContent: text };
}

describe('buildAlignment', () => {
  it('maps 1:1 when DOM text equals raw text', () => {
    const raw = 'aanspraak op zorgtoeslag';
    const node = tn(raw);
    const { rawToDom } = buildAlignment(raw, [{ node, text: raw }]);
    expect(rawToDom).toHaveLength(raw.length);
    rawToDom.forEach((m, i) => {
      expect(m).toEqual({ node, offset: i });
    });
  });

  it('skips a numbered-list prefix that marked removed from the DOM', () => {
    // Raw lid text "1. Indien ..."; marked renders <li><p>Indien ...</p></li>,
    // so the DOM text node is "Indien ..." — the "1. " is raw-only.
    const raw = '1. Indien de normpremie';
    const domText = 'Indien de normpremie';
    const node = tn(domText);
    const { rawToDom } = buildAlignment(raw, [{ node, text: domText }]);
    // "1. " (offsets 0,1,2) has no DOM counterpart.
    expect(rawToDom[0]).toBeNull();
    expect(rawToDom[1]).toBeNull();
    expect(rawToDom[2]).toBeNull();
    // "Indien" starts at raw offset 3 -> DOM offset 0.
    expect(rawToDom[3]).toEqual({ node, offset: 0 });
    expect(rawToDom[raw.length - 1]).toEqual({
      node,
      offset: domText.length - 1,
    });
  });

  it('skips collapsed whitespace (raw double space -> single DOM space)', () => {
    const raw = 'a  b'; // two spaces in raw
    const domText = 'a b'; // HTML collapsed to one
    const node = tn(domText);
    const { rawToDom } = buildAlignment(raw, [{ node, text: domText }]);
    expect(rawToDom[0]).toEqual({ node, offset: 0 }); // a
    // One of the two raw spaces maps, the other is skipped (null).
    const spaceMaps = [rawToDom[1], rawToDom[2]].filter(Boolean);
    expect(spaceMaps).toHaveLength(1);
    expect(rawToDom[3]).toEqual({ node, offset: 2 }); // b
  });

  it('keeps a soft newline that survives as text in the DOM node', () => {
    // marked leaves a soft "\n" inside a paragraph as a literal newline in
    // the text node (verified against marked v18); it must map 1:1.
    const raw = 'dat\njaar';
    const node = tn(raw);
    const { rawToDom } = buildAlignment(raw, [{ node, text: raw }]);
    expect(rawToDom[3]).toEqual({ node, offset: 3 }); // the \n
    rawToDom.forEach((m, i) => expect(m).toEqual({ node, offset: i }));
  });

  it('does not flag desync for the expected raw-only transforms', () => {
    // List prefix + collapsed whitespace are expected; desynced stays false.
    const raw = '1. a  b';
    const node = tn('a b');
    const { desynced } = buildAlignment(raw, [{ node, text: 'a b' }]);
    expect(desynced).toBe(false);
  });

  it('tolerates a tiny DOM-only artefact (does not flag desync)', () => {
    // A stray entity/punctuation artefact (run <= 4) must not suppress an
    // otherwise-correct article: that would throw away every valid highlight
    // over one decoded character. Here DOM has one extra "Z" the raw lacks.
    const raw = 'abcdef';
    const node = tn('abcZdef');
    const { desynced } = buildAlignment(raw, [{ node, text: 'abcZdef' }]);
    expect(desynced).toBe(false);
  });

  it('flags desync on a contiguous block of DOM-only text', () => {
    // The real failure shape: inline markdown that ADDED visible DOM text
    // (e.g. a renderer-injected phrase, or markup the scan cannot model),
    // leaving a contiguous run with no raw counterpart after which every
    // offset is shifted. Run > 4 -> drop highlighting rather than smear.
    // This is scale-independent: it fires regardless of article length,
    // unlike the old ratio threshold which only caught tiny articles.
    const raw = 'het toetsingsinkomen van de verzekerde';
    const domText = 'het toetsingsinkomen ZIE NOOT 7 van de verzekerde';
    const node = tn(domText);
    const { desynced } = buildAlignment(raw, [{ node, text: domText }]);
    expect(desynced).toBe(true);
  });

  it('spans multiple DOM nodes (two list items)', () => {
    // Raw: "1. eerste\n\n2. tweede" -> two <li> text nodes "eerste","tweede".
    const raw = '1. eerste\n\n2. tweede';
    const n1 = tn('eerste');
    const n2 = tn('tweede');
    const { rawToDom } = buildAlignment(raw, [
      { node: n1, text: 'eerste' },
      { node: n2, text: 'tweede' },
    ]);
    // "eerste" is raw 3..9 -> n1 0..6
    expect(rawToDom[3]).toEqual({ node: n1, offset: 0 });
    expect(rawToDom[8]).toEqual({ node: n1, offset: 5 });
    // "tweede" -> n2; find first mapped char in the second half
    const second = rawToDom.slice(11).find(Boolean);
    expect(second.node).toBe(n2);
  });
});

describe('spanToNodeSlices', () => {
  const raw = '1. Indien de normpremie';
  const domText = 'Indien de normpremie';
  const node = tn(domText);

  it('clips a span that starts inside a stripped prefix to the first real char', () => {
    const { rawToDom } = buildAlignment(raw, [{ node, text: domText }]);
    const len = new Map([[node, Array.from(domText).length]]);
    // Span covers the prefix + "Indien" (raw 0..9).
    const slices = spanToNodeSlices(rawToDom, { start: 0, end: 9 }, len);
    expect(slices).toEqual([{ node, startCp: 0, endCp: 6 }]); // "Indien"
  });

  it('produces one slice per crossed text node', () => {
    const r = '1. een\n\n2. twee';
    const a = tn('een');
    const b = tn('twee');
    const { rawToDom } = buildAlignment(r, [
      { node: a, text: 'een' },
      { node: b, text: 'twee' },
    ]);
    const len = new Map([
      [a, 3],
      [b, 4],
    ]);
    const slices = spanToNodeSlices(rawToDom, { start: 3, end: 15 }, len);
    expect(slices.map((s) => s.node)).toEqual([a, b]);
    expect(slices[0]).toEqual({ node: a, startCp: 0, endCp: 3 });
    expect(slices[1].node).toBe(b);
  });

  it('returns [] for an unanchorable span', () => {
    const { rawToDom } = buildAlignment('1. ', [{ node: tn(''), text: '' }]);
    expect(spanToNodeSlices(rawToDom, { start: 0, end: 3 }, new Map())).toEqual(
      [],
    );
  });
});

describe('cpToUtf16', () => {
  it('is identity for BMP text', () => {
    expect(cpToUtf16('verzekerde', 4)).toBe(4);
  });

  it('counts an astral code point as two UTF-16 units', () => {
    // "🎉" is 1 code point, 2 UTF-16 units. Offset 2 (after "a🎉") -> 3.
    expect(cpToUtf16('a🎉b', 2)).toBe(3);
  });
});

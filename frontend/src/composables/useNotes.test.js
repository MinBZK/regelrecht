import { describe, it, expect } from 'vitest';
import { markRanges } from './useNotes.js';

// markRanges is the highest-risk pure function in the notes feature: it turns
// resolver char-offsets into rendered segments. These tests pin the partition
// invariant (every char emitted exactly once) and the documented overlap
// behaviour.

const noteA = { motivation: 'linking' };
const noteB = { motivation: 'commenting' };

/** Re-joining all segment texts must reproduce the input exactly. */
function assertPartition(text, segments) {
  expect(segments.map((s) => s.text).join('')).toBe(text);
}

describe('markRanges', () => {
  it('returns one plain segment when there are no notes', () => {
    const text = 'heeft de verzekerde aanspraak';
    const segs = markRanges(text, []);
    expect(segs).toEqual([{ text, note: null }]);
    assertPartition(text, segs);
  });

  it('wraps a single span and keeps the surrounding text', () => {
    const text = 'op een zorgtoeslag hier';
    // "zorgtoeslag" is chars 7..18
    const segs = markRanges(text, [
      { note: noteA, spans: [{ start: 7, end: 18 }] },
    ]);
    expect(segs).toEqual([
      { text: 'op een ', note: null },
      { text: 'zorgtoeslag', note: noteA },
      { text: ' hier', note: null },
    ]);
    assertPartition(text, segs);
  });

  it('handles a span at the very start and end', () => {
    const text = 'abcdef';
    expect(markRanges(text, [{ note: noteA, spans: [{ start: 0, end: 3 }] }])).toEqual([
      { text: 'abc', note: noteA },
      { text: 'def', note: null },
    ]);
    expect(markRanges(text, [{ note: noteA, spans: [{ start: 3, end: 6 }] }])).toEqual([
      { text: 'abc', note: null },
      { text: 'def', note: noteA },
    ]);
  });

  it('emits two adjacent non-overlapping marks with no gap', () => {
    const text = '0123456789';
    const segs = markRanges(text, [
      { note: noteA, spans: [{ start: 1, end: 3 }] },
      { note: noteB, spans: [{ start: 3, end: 5 }] },
    ]);
    expect(segs).toEqual([
      { text: '0', note: null },
      { text: '12', note: noteA },
      { text: '34', note: noteB },
      { text: '56789', note: null },
    ]);
    assertPartition(text, segs);
  });

  it('drops a later mark that overlaps an earlier one, keeping the partition intact', () => {
    const text = '0123456789';
    // B (2..6) overlaps A (1..4); A starts first so B is dropped.
    const segs = markRanges(text, [
      { note: noteB, spans: [{ start: 2, end: 6 }] },
      { note: noteA, spans: [{ start: 1, end: 4 }] },
    ]);
    expect(segs).toEqual([
      { text: '0', note: null },
      { text: '123', note: noteA },
      { text: '456789', note: null },
    ]);
    // Invariant holds even when a note is dropped.
    assertPartition(text, segs);
    expect(segs.some((s) => s.note === noteB)).toBe(false);
  });

  it('prefers the longer span on equal start (sort tiebreak)', () => {
    const text = '0123456789';
    const segs = markRanges(text, [
      { note: noteA, spans: [{ start: 1, end: 3 }] },
      { note: noteB, spans: [{ start: 1, end: 6 }] },
    ]);
    // Both start at 1; the longer (B, 1..6) wins, the shorter is dropped.
    expect(segs).toEqual([
      { text: '0', note: null },
      { text: '12345', note: noteB },
      { text: '6789', note: null },
    ]);
    assertPartition(text, segs);
  });

  it('treats offsets as code points, not UTF-16 units', () => {
    // "ë" is one code point; an astral emoji is one code point but two UTF-16
    // units. The resolver yields code-point offsets, so markRanges must slice
    // by code point. After "café🎉 " (c a f é 🎉 space = 6 code points) the
    // word "tekst" is chars 6..11.
    const text = 'café🎉 tekst hier';
    const segs = markRanges(text, [
      { note: noteA, spans: [{ start: 6, end: 11 }] },
    ]);
    expect(segs).toEqual([
      { text: 'café🎉 ', note: null },
      { text: 'tekst', note: noteA },
      { text: ' hier', note: null },
    ]);
    assertPartition(text, segs);
  });

  it('handles multiple notes across the text in document order', () => {
    const text = 'de verzekerde heeft aanspraak op zorgtoeslag';
    const segs = markRanges(text, [
      { note: noteA, spans: [{ start: 3, end: 13 }] }, // "verzekerde"
      { note: noteB, spans: [{ start: 33, end: 44 }] }, // "zorgtoeslag"
    ]);
    expect(segs.filter((s) => s.note === noteA)).toHaveLength(1);
    expect(segs.filter((s) => s.note === noteB)).toHaveLength(1);
    assertPartition(text, segs);
  });

  it('drops a zero-length span instead of emitting an empty mark', () => {
    const text = 'heeft de verzekerde aanspraak';
    const segs = markRanges(text, [
      { note: noteA, spans: [{ start: 9, end: 9 }] }, // degenerate span
      { note: noteB, spans: [{ start: 9, end: 19 }] }, // "verzekerde"
    ]);
    // No empty-text segment, no mark for the zero-length note.
    expect(segs.every((s) => s.text.length > 0)).toBe(true);
    expect(segs.some((s) => s.note === noteA)).toBe(false);
    expect(segs.filter((s) => s.note === noteB)).toHaveLength(1);
    assertPartition(text, segs);
  });
});

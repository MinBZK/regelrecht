/**
 * planSegments — overlap-aware render plan for note highlights.
 *
 * Replaces the flat-partition overlap drop that #647 inherited from
 * markRanges. Given each note's resolved spans (with the note's index in
 * notesForArticle), produces a disjoint ordered list of segments whose union
 * is exactly the union of all input spans. Each segment carries enough
 * information for the renderer to:
 *
 *   - paint the right backgrounds in default state (`visibleIdx`),
 *   - bridge a hovered note across regions where it is suppressed
 *     (`coveringIdx`),
 *   - choose which note's popover opens on hover (`primaryIdx`).
 *
 * Encapsulation rule: if note X strictly contains note Y (X.start <= Y.start
 * AND X.end >= Y.end AND at least one inequality is strict) and both cover
 * the same segment, X is suppressed in that segment so the inner note shows
 * cleanly. X stays in the `coveringIdx` so hovering it bridges back to its
 * full span. Identical spans both render — neither strictly contains the
 * other.
 *
 * Partial overlap (neither contains the other): both render layered.
 *
 * Boundary sweep: the segment endpoints are exactly the spans' own
 * endpoints, so containment in a segment [a, b) reduces to
 * `note.start <= a && note.end >= b`.
 *
 * @param {Array<{start:number, end:number, idx:number}>} items
 *        Flattened (note, span) pairs; idx is the note's index in
 *        notesForArticle. A note with multiple spans contributes multiple
 *        items with the same idx.
 * @returns {Array<{
 *   start:number,
 *   end:number,
 *   visibleIdx:number[],
 *   coveringIdx:number[],
 *   primaryIdx:number,
 * }>}
 *   Segments in ascending start order. Visible/covering arrays are
 *   deduplicated by idx (so a note with two spans crossing a segment counts
 *   once); idx is preserved in the order it first appears.
 */
export function planSegments(items) {
  const valid = items.filter((it) => it.end > it.start);
  if (valid.length === 0) return [];

  const points = new Set();
  for (const it of valid) {
    points.add(it.start);
    points.add(it.end);
  }
  const boundaries = [...points].sort((a, b) => a - b);

  const segments = [];
  for (let i = 0; i < boundaries.length - 1; i++) {
    const a = boundaries[i];
    const b = boundaries[i + 1];
    const covering = valid.filter((it) => it.start <= a && it.end >= b);
    if (covering.length === 0) continue;

    const visible = covering.filter(
      (x) => !covering.some((y) => y !== x && strictlyContains(x, y)),
    );
    // Strict containment is asymmetric (A ⊃ B ∧ B ⊃ A would require both
    // boundaries equal, which violates the "at least one strict" clause), so
    // a non-empty covering set always has at least one minimal element that
    // survives the filter. Surfacing the invariant as a throw, not a silent
    // continue, means a future change to strictlyContains that breaks it
    // fails loudly instead of producing empty render segments.
    if (visible.length === 0) {
      throw new Error('planSegments invariant: visible must be non-empty when covering is non-empty');
    }

    const primary = pickPrimary(visible);
    segments.push({
      start: a,
      end: b,
      visibleIdx: uniqueIdx(visible),
      coveringIdx: uniqueIdx(covering),
      primaryIdx: primary.idx,
    });
  }

  // Merge adjacent segments with identical visible/covering/primary so the
  // renderer wraps one <mark> for a run of equal-coverage chars instead of
  // one per boundary. Span endpoints from other notes outside the covering
  // set still create boundaries here; merging keeps the DOM tidy.
  return mergeAdjacent(segments);
}

function strictlyContains(outer, inner) {
  return (
    outer.start <= inner.start &&
    outer.end >= inner.end &&
    (outer.start < inner.start || outer.end > inner.end)
  );
}

function pickPrimary(visible) {
  // Earliest span.start wins (the note that "starts first" in document
  // order). On a tie, lowest idx — that is the order the note appears in
  // notesForArticle, which is the YAML order for committed notes.
  let best = visible[0];
  for (let i = 1; i < visible.length; i++) {
    const x = visible[i];
    if (x.start < best.start) best = x;
    else if (x.start === best.start && x.idx < best.idx) best = x;
  }
  return best;
}

function uniqueIdx(items) {
  const seen = new Set();
  const out = [];
  for (const it of items) {
    if (seen.has(it.idx)) continue;
    seen.add(it.idx);
    out.push(it.idx);
  }
  return out;
}

function sameSet(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function mergeAdjacent(segments) {
  if (segments.length < 2) return segments;
  const out = [segments[0]];
  for (let i = 1; i < segments.length; i++) {
    const prev = out[out.length - 1];
    const cur = segments[i];
    if (
      prev.end === cur.start &&
      prev.primaryIdx === cur.primaryIdx &&
      sameSet(prev.visibleIdx, cur.visibleIdx) &&
      sameSet(prev.coveringIdx, cur.coveringIdx)
    ) {
      prev.end = cur.end;
    } else {
      out.push(cur);
    }
  }
  return out;
}

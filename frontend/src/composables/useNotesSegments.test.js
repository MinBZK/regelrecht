import { describe, it, expect } from 'vitest';
import { planSegments } from './useNotesSegments.js';

// planSegments replaces the flat-partition overlap drop. These tests pin the
// boundary-sweep, the encapsulation suppression rule, and the primary
// tiebreak so the renderer can rely on the plan shape without re-checking.

describe('planSegments', () => {
  it('returns [] when there are no spans', () => {
    expect(planSegments([])).toEqual([]);
  });

  it('skips zero-length spans', () => {
    expect(
      planSegments([{ start: 5, end: 5, idx: 0 }]),
    ).toEqual([]);
  });

  it('passes a single span through as one segment', () => {
    expect(
      planSegments([{ start: 10, end: 20, idx: 0 }]),
    ).toEqual([
      {
        start: 10,
        end: 20,
        visibleIdx: [0],
        coveringIdx: [0],
        primaryIdx: 0,
      },
    ]);
  });

  it('emits non-overlapping spans as two independent segments', () => {
    expect(
      planSegments([
        { start: 10, end: 20, idx: 0 },
        { start: 30, end: 40, idx: 1 },
      ]),
    ).toEqual([
      { start: 10, end: 20, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
      { start: 30, end: 40, visibleIdx: [1], coveringIdx: [1], primaryIdx: 1 },
    ]);
  });

  it('splits a partial overlap into three segments, both visible in the middle', () => {
    // A:[10,20) and B:[15,25) - neither contains the other, both render in
    // the overlap. Primary in the overlap is A (earlier start).
    expect(
      planSegments([
        { start: 10, end: 20, idx: 0 },
        { start: 15, end: 25, idx: 1 },
      ]),
    ).toEqual([
      { start: 10, end: 15, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
      {
        start: 15,
        end: 20,
        visibleIdx: [0, 1],
        coveringIdx: [0, 1],
        primaryIdx: 0,
      },
      { start: 20, end: 25, visibleIdx: [1], coveringIdx: [1], primaryIdx: 1 },
    ]);
  });

  it('suppresses the outer note in the inner-only segment (encapsulation)', () => {
    // A:[10,50) encapsulates B:[20,30). In [20,30) only B is visible, but A
    // stays in coveringIdx so hovering A from [10,20) or [30,50) can bridge
    // back over the inner segment.
    expect(
      planSegments([
        { start: 10, end: 50, idx: 0 },
        { start: 20, end: 30, idx: 1 },
      ]),
    ).toEqual([
      { start: 10, end: 20, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
      {
        start: 20,
        end: 30,
        visibleIdx: [1],
        coveringIdx: [0, 1],
        primaryIdx: 1,
      },
      { start: 30, end: 50, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
    ]);
  });

  it('renders identical spans layered - neither strictly contains the other', () => {
    // Two notes on exactly the same span: both visible, primary by lowest idx.
    expect(
      planSegments([
        { start: 10, end: 20, idx: 0 },
        { start: 10, end: 20, idx: 1 },
      ]),
    ).toEqual([
      {
        start: 10,
        end: 20,
        visibleIdx: [0, 1],
        coveringIdx: [0, 1],
        primaryIdx: 0,
      },
    ]);
  });

  it('handles nested encapsulation (3 levels) with only the innermost visible at the core', () => {
    // A:[1,100) wraps B:[10,90) wraps C:[20,30). Each outer is suppressed
    // wherever an inner covers the same segment.
    const plan = planSegments([
      { start: 1, end: 100, idx: 0 },
      { start: 10, end: 90, idx: 1 },
      { start: 20, end: 30, idx: 2 },
    ]);
    expect(plan).toEqual([
      { start: 1, end: 10, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
      {
        start: 10,
        end: 20,
        visibleIdx: [1],
        coveringIdx: [0, 1],
        primaryIdx: 1,
      },
      {
        start: 20,
        end: 30,
        visibleIdx: [2],
        coveringIdx: [0, 1, 2],
        primaryIdx: 2,
      },
      {
        start: 30,
        end: 90,
        visibleIdx: [1],
        coveringIdx: [0, 1],
        primaryIdx: 1,
      },
      {
        start: 90,
        end: 100,
        visibleIdx: [0],
        coveringIdx: [0],
        primaryIdx: 0,
      },
    ]);
  });

  it('picks the earliest-start note as primary in an overlap', () => {
    // A:[10,40), B:[20,30) (inside A): primary in the inner segment is B,
    // because A is suppressed. In the outer A-only flanks, primary is A.
    const plan = planSegments([
      { start: 10, end: 40, idx: 5 },
      { start: 20, end: 30, idx: 9 },
    ]);
    expect(plan.map((s) => s.primaryIdx)).toEqual([5, 9, 5]);
  });

  it('breaks ties on equal start by lowest idx', () => {
    // Both span [10,20); both visible; primary should be the lower idx, not
    // the array order. Test order is reversed to make this matter.
    const plan = planSegments([
      { start: 10, end: 20, idx: 7 },
      { start: 10, end: 20, idx: 3 },
    ]);
    expect(plan).toHaveLength(1);
    expect(plan[0].primaryIdx).toBe(3);
  });

  it('deduplicates a note that contributes two spans to one segment', () => {
    // A single note (idx 0) has two spans that both cover [15,20) - could
    // happen if a TextQuoteSelector matches two adjacent occurrences. The
    // visibleIdx/coveringIdx lists must dedupe so the renderer paints one
    // background layer, not two of the same colour.
    const plan = planSegments([
      { start: 10, end: 20, idx: 0 },
      { start: 15, end: 25, idx: 0 },
    ]);
    // Boundaries 10, 15, 20, 25. Same note covers all three segments.
    // After merging, every adjacent segment has the same {visible, covering,
    // primary} so they collapse into one [10,25) segment.
    expect(plan).toEqual([
      { start: 10, end: 25, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
    ]);
  });

  it('merges adjacent segments with identical coverage', () => {
    // A:[10,40) and B:[20,30) inside A produce 3 segments, but if another
    // unrelated note D:[50,60) is added it should NOT introduce extra
    // boundaries in A's region - already true; this test guards against a
    // regression where stray boundaries got injected.
    const plan = planSegments([
      { start: 10, end: 40, idx: 0 },
      { start: 20, end: 30, idx: 1 },
      { start: 50, end: 60, idx: 2 },
    ]);
    expect(plan).toEqual([
      { start: 10, end: 20, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
      {
        start: 20,
        end: 30,
        visibleIdx: [1],
        coveringIdx: [0, 1],
        primaryIdx: 1,
      },
      { start: 30, end: 40, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
      { start: 50, end: 60, visibleIdx: [2], coveringIdx: [2], primaryIdx: 2 },
    ]);
  });

  it('handles a three-way partial+encapsulating mix', () => {
    // A:[10,30), B:[20,40), C:[25,35). A and B partially overlap; B
    // strictly contains C; C and A partially overlap. In [25,30) all three
    // cover, but B contains C so B is suppressed there → visible {A, C}.
    const plan = planSegments([
      { start: 10, end: 30, idx: 0 },
      { start: 20, end: 40, idx: 1 },
      { start: 25, end: 35, idx: 2 },
    ]);
    expect(plan).toEqual([
      { start: 10, end: 20, visibleIdx: [0], coveringIdx: [0], primaryIdx: 0 },
      {
        start: 20,
        end: 25,
        visibleIdx: [0, 1],
        coveringIdx: [0, 1],
        primaryIdx: 0,
      },
      {
        start: 25,
        end: 30,
        visibleIdx: [0, 2],
        coveringIdx: [0, 1, 2],
        primaryIdx: 0,
      },
      {
        start: 30,
        end: 35,
        visibleIdx: [2],
        coveringIdx: [1, 2],
        primaryIdx: 2,
      },
      {
        start: 35,
        end: 40,
        visibleIdx: [1],
        coveringIdx: [1],
        primaryIdx: 1,
      },
    ]);
  });
});

// @vitest-environment jsdom
//
// This file asserts the exact DOM structure produced by the marked +
// DOMPurify article pipeline (e.g. that "1. " renders as <ol><li>). happy-dom
// 20.x (our default test environment) has a NodeIterator bug that DOMPurify
// >= 3.4.8 trips over while scrubbing: it strips the <ol>/<ul> wrapper and
// keeps only the <li>, so `querySelector('ol li')` returns null. Verified in
// real Chromium, and under jsdom, that the same DOMPurify output keeps the
// list intact, so this is purely a happy-dom quirk, not a production bug. Pin
// this file to jsdom (a spec-faithful NodeIterator) until happy-dom fixes it.
//
// TODO(happy-dom NodeIterator): once a happy-dom release sanitizes DOMPurify's
// <ol>/<ul> output without stripping the wrapper, drop this docblock so the
// file returns to the default happy-dom environment. Re-check by temporarily
// removing the line and running this file's "ol li" assertions.
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import AnnotatedText from './AnnotatedText.vue';

// The authoring tests mount NoteCreator, which fetches the ambiguity
// vocabulary on first use. Stub fetch so no real request dangles.
beforeEach(() => {
  globalThis.fetch = vi
    .fn()
    .mockResolvedValue({ ok: true, text: async () => 'ambiguity: []\n' });
});

// The watcher awaits nextTick before applyHighlights, so the test must wait
// two ticks: one for the watcher to fire, one for its inner nextTick.

// Component-level tests for the DOM-bound behaviours the pure
// useNotesHighlight / useNotesSegments tests cannot cover: idempotent
// re-apply (clear before re-wrap) and the boundary-segment overlap render
// (partial overlap = layered, encapsulation = inner shown, outer suppressed
// in the inner's segment). The markdown render + offset alignment are
// covered by useNotesHighlight.test.js; the segment-planning rules
// themselves by useNotesSegments.test.js.

const nlddStubs = {
  'nldd-rich-text': { template: '<div><slot/></div>' },
  'nldd-popover': { template: '<div><slot/></div>' },
  'nldd-inline-dialog': { template: '<div/>' },
  'nldd-segmented-control': { template: '<div><slot/></div>' },
  'nldd-segmented-control-item': { template: '<div/>' },
  'nldd-text-field': { template: '<input/>' },
  'nldd-button': { template: '<button><slot/></button>' },
};

function mountWith(article, notesForArticle, extraProps = {}) {
  return mount(AnnotatedText, {
    props: { article, notesForArticle, ...extraProps },
    global: { stubs: nlddStubs },
  });
}

// "1. " is a numbered-list prefix marked strips from the DOM text; the
// resolver offsets are into the raw string including that prefix.
const ART = {
  number: '2',
  text: '1. een verzekerde heeft aanspraak op zorgtoeslag',
};
// raw offsets: "verzekerde" = 7..17, "zorgtoeslag" = 37..48
const noteVerzekerde = { motivation: 'commenting', creator: 'A' };
const noteZorgtoeslag = { motivation: 'linking', creator: 'B' };

describe('AnnotatedText markdown highlighting', () => {
  it('wraps resolved spans in <mark> over the markdown-rendered list', async () => {
    const wrapper = mountWith(ART, [
      { note: noteVerzekerde, spans: [{ start: 3, end: 17 }] },
    ]);
    await nextTick();
    await nextTick();
    const marks = wrapper.element.querySelectorAll('mark[data-note-idx]');
    expect(marks).toHaveLength(1);
    // The "1. " prefix is stripped; the mark covers "een verzekerde".
    expect(marks[0].textContent).toBe('een verzekerde');
    expect(marks[0].className).toContain('note-commenting');
  });

  it('is idempotent: re-applying does not double-wrap', async () => {
    const wrapper = mountWith(ART, [
      { note: noteVerzekerde, spans: [{ start: 7, end: 17 }] },
    ]);
    await nextTick();
    await nextTick();
    expect(
      wrapper.element.querySelectorAll('mark[data-note-idx]'),
    ).toHaveLength(1);

    // Change notes without changing the article text (the Fase-5 scenario:
    // html is stable, only notesForArticle changes). Must clear the old
    // mark, not stack a second one.
    await wrapper.setProps({
      notesForArticle: [
        { note: noteZorgtoeslag, spans: [{ start: 37, end: 48 }] },
      ],
    });
    await nextTick();
    await nextTick();
    const marks = wrapper.element.querySelectorAll('mark[data-note-idx]');
    expect(marks).toHaveLength(1);
    expect(marks[0].textContent).toBe('zorgtoeslag');
    // No leftover nested marks anywhere.
    expect(wrapper.element.querySelectorAll('mark mark')).toHaveLength(0);
  });

  it('splits a partial overlap into three marks; the middle one shows both notes layered', async () => {
    // A on "verzekerde" (raw 7..17), B on "kerde heeft a" (raw 12..25):
    // boundaries 7,12,17,25 -> three segments. The middle [12,17) has both
    // notes visible (layered backgrounds); flanks have one each. Primary in
    // the middle is the earlier-start note (A), so its popover opens on
    // hover.
    const wrapper = mountWith(ART, [
      { note: noteVerzekerde, spans: [{ start: 7, end: 17 }] },
      { note: noteZorgtoeslag, spans: [{ start: 12, end: 25 }] },
    ]);
    await nextTick();
    await nextTick();
    const marks = [
      ...wrapper.element.querySelectorAll('mark[data-primary-idx]'),
    ];
    expect(marks).toHaveLength(3);
    // [7,12) - only A.
    expect(marks[0].textContent).toBe('verze');
    expect(marks[0].dataset.noteIdx).toBe('0');
    expect(marks[0].dataset.primaryIdx).toBe('0');
    expect(marks[0].dataset.coverDepth).toBe('1');
    expect(marks[0].className).toContain('note-commenting');
    // [12,17) - both, primary is A (earlier start), bg is the layered
    // marker class (no class-background; inline backgroundImage stacks).
    // coverDepth=2 drives the top-edge underline that makes same-motivation
    // overlap legible without depending on a colour shift.
    expect(marks[1].textContent).toBe('kerde');
    expect(marks[1].dataset.noteIdx).toBe('0,1');
    expect(marks[1].dataset.primaryIdx).toBe('0');
    expect(marks[1].dataset.coverDepth).toBe('2');
    expect(marks[1].className).toContain('note-multi');
    expect(marks[1].style.backgroundImage).toContain('linear-gradient');
    // [17,25) - only B.
    expect(marks[2].textContent).toBe(' heeft a');
    expect(marks[2].dataset.noteIdx).toBe('1');
    expect(marks[2].dataset.primaryIdx).toBe('1');
    expect(marks[2].dataset.coverDepth).toBe('1');
  });

  it('encapsulation: outer is suppressed inside the inner segment (inner shown cleanly)', async () => {
    // Outer A on "een verzekerde heeft aanspraak " (raw 3..34) wraps inner
    // B on "verzekerde" (raw 7..17). The inner segment [7,17) must show
    // only B; the outer's segments [3,7) and [17,34) show only A. A stays
    // in coveringIdx of the inner segment so a hover-bridge can reach it.
    const wrapper = mountWith(ART, [
      { note: noteVerzekerde, spans: [{ start: 3, end: 34 }] }, // outer
      { note: noteZorgtoeslag, spans: [{ start: 7, end: 17 }] }, // inner
    ]);
    await nextTick();
    await nextTick();
    const marks = [
      ...wrapper.element.querySelectorAll('mark[data-primary-idx]'),
    ];
    expect(marks).toHaveLength(3);
    // Outer's left flank: A only.
    expect(marks[0].textContent).toBe('een ');
    expect(marks[0].dataset.noteIdx).toBe('0');
    expect(marks[0].dataset.coverIdx).toBe('0');
    expect(marks[0].dataset.coverDepth).toBe('1');
    // Inner: B shown, A suppressed but reachable via coverIdx. coverDepth=2
    // surfaces the nested boundary visually even if outer and inner share a
    // motivation (suppressed outer would otherwise erase the colour shift).
    expect(marks[1].textContent).toBe('verzekerde');
    expect(marks[1].dataset.noteIdx).toBe('1');
    expect(marks[1].dataset.coverIdx).toBe('0,1');
    expect(marks[1].dataset.primaryIdx).toBe('1');
    expect(marks[1].dataset.coverDepth).toBe('2');
    // Outer's right flank: A only.
    expect(marks[2].dataset.noteIdx).toBe('0');
    expect(marks[2].dataset.coverDepth).toBe('1');
  });

  it('caps cover-depth at 3 even when four+ notes share a span', async () => {
    // Four notes on identical spans - none strictly contains another, all
    // four are visible. coveringIdx.length is 4 but data-cover-depth must
    // clamp to '3' so the CSS rule set stays bounded.
    const wrapper = mountWith(ART, [
      { note: noteVerzekerde, spans: [{ start: 7, end: 17 }] },
      { note: noteZorgtoeslag, spans: [{ start: 7, end: 17 }] },
      { note: { motivation: 'questioning', creator: 'C' }, spans: [{ start: 7, end: 17 }] },
      { note: { motivation: 'tagging', creator: 'D' }, spans: [{ start: 7, end: 17 }] },
    ]);
    await nextTick();
    await nextTick();
    const marks = [
      ...wrapper.element.querySelectorAll('mark[data-primary-idx]'),
    ];
    expect(marks).toHaveLength(1);
    expect(marks[0].dataset.noteIdx).toBe('0,1,2,3');
    expect(marks[0].dataset.coverDepth).toBe('3');
  });

  it('hovering the outer in encapsulation bridges .note-hovered across the inner segment', async () => {
    // Same setup as the encapsulation test. Firing pointerover on the
    // outer's left flank must add .note-hovered to all three marks - the
    // inner segment too, even though it does not render the outer's
    // background by default - so the outer's full extent reads as one
    // continuous range.
    const wrapper = mountWith(ART, [
      { note: noteVerzekerde, spans: [{ start: 3, end: 34 }] },
      { note: noteZorgtoeslag, spans: [{ start: 7, end: 17 }] },
    ]);
    await nextTick();
    await nextTick();
    const marks = [
      ...wrapper.element.querySelectorAll('mark[data-primary-idx]'),
    ];
    marks[0].dispatchEvent(new Event('pointerover', { bubbles: true }));
    await nextTick();
    expect(marks[0].classList.contains('note-hovered')).toBe(true);
    expect(marks[1].classList.contains('note-hovered')).toBe(true);
    expect(marks[2].classList.contains('note-hovered')).toBe(true);
  });

  it('does not wrap the inter-<li> whitespace nodes when a span crosses leden', async () => {
    // marked emits "\n" text nodes between <li>s. A note spanning two leden
    // must not wrap those bare newlines in a focusable, styled <mark>
    // (invalid HTML + empty highlight blob between list items). Regression
    // guard for the cross-node slice bug.
    const twoLeden = {
      number: '2',
      text: '1. eerste lid hier\n\n2. tweede lid daar',
    };
    // Span from "eerste" (raw 3) through "tweede lid" - crosses the \n\n.
    const wrapper = mountWith(twoLeden, [
      { note: noteVerzekerde, spans: [{ start: 3, end: 34 }] },
    ]);
    await nextTick();
    await nextTick();
    const marks = [
      ...wrapper.element.querySelectorAll('mark[data-note-idx]'),
    ];
    expect(marks.length).toBeGreaterThan(0);
    // No mark may contain only whitespace.
    for (const m of marks) {
      expect(m.textContent.trim()).not.toBe('');
    }
    // No <mark> is a direct child of <ol> (would mean a wrapped \n node).
    expect(wrapper.element.querySelectorAll('ol > mark')).toHaveLength(0);
  });

  it('renders no marks (clean markdown) when there are no notes', async () => {
    const wrapper = mountWith(ART, []);
    await nextTick();
    await nextTick();
    expect(
      wrapper.element.querySelectorAll('mark[data-note-idx]'),
    ).toHaveLength(0);
    // The list rendered: an <ol><li> from "1. ".
    expect(wrapper.element.querySelector('ol li')).toBeTruthy();
  });

  it('renders no authoring UI in the default read-only mode', async () => {
    const wrapper = mountWith(ART, []);
    await nextTick();
    // canCreate defaults false: no NoteCreator, no floating button, no
    // selection anchor - the pane is purely a read view.
    expect(wrapper.find('[data-testid="create-note-btn"]').exists()).toBe(
      false,
    );
    expect(wrapper.find('.sel-anchor').exists()).toBe(false);
    expect(wrapper.findComponent({ name: 'NoteCreator' }).exists()).toBe(false);
  });

  it('mounts the authoring path without disturbing the highlight render', async () => {
    const wrapper = mountWith(ART, [{ note: noteVerzekerde, spans: [] }], {
      canCreate: true,
      lawId: 'wet_op_de_zorgtoeslag',
      engine: { resolveNote: () => ({ status: 'orphaned', matches: [] }) },
    });
    await nextTick();
    await nextTick();
    // NoteCreator is present (gated on canCreate) but its popover is closed
    // until a selection is made, and the read render is unaffected.
    expect(wrapper.findComponent({ name: 'NoteCreator' }).exists()).toBe(true);
    expect(wrapper.element.querySelector('ol li')).toBeTruthy();
  });

  it('tears down an open creator when the article changes', async () => {
    const wrapper = mountWith(ART, [], {
      canCreate: true,
      lawId: 'wet_op_de_zorgtoeslag',
      engine: { resolveNote: () => ({ status: 'found', matches: [] }) },
    });
    await nextTick();
    // Simulate a selection captured + creator opened against ART.
    wrapper.vm.pendingRange = { start: 3, end: 9 };
    wrapper.vm.openCreator();
    await nextTick();
    expect(wrapper.vm.creatorOpen).toBe(true);
    expect(wrapper.vm.selectionRange).toEqual({ start: 3, end: 9 });
    // Navigating to another article must reset the creation flow so
    // NoteCreator never builds a selector with stale offsets against the
    // new article's text (must-fix 2c).
    await wrapper.setProps({
      article: { number: '3', text: 'Een heel ander artikel hier.' },
    });
    await nextTick();
    expect(wrapper.vm.creatorOpen).toBe(false);
    expect(wrapper.vm.selectionRange).toBe(null);
  });
});

// nldd-popover uses the native HTML popover API: showPopover() puts the
// element in the top layer and steals focus. Opening it from a pointerover
// while the user is mid-drag therefore lands the drag's pointermove on the
// popover instead of the underlying text - selection cannot extend past the
// first mark it touches. These tests pin the drag-aware gate that closes
// the hover-popover path during a selection drag, while keeping hover and
// keyboard discoverability intact.
//
// The signal under test is `wrapper.vm.activeNote`, not a spy on the
// popover stub. openFor sets activeNote BEFORE calling pop.showPopover,
// so observing activeNote tells us whether the gate let openFor through.
// This sidesteps quirks in how Vue Test Utils stubs expose setup() returns
// via useTemplateRef + optional-chained calls.
describe('AnnotatedText popover suppression during drag-selection', () => {
  function fire(target, type, init = {}) {
    // MouseEvent matches the type names Vue listens to for pointer events
    // (pointerover/pointerdown/pointerup/pointercancel) and gives us
    // clientX/clientY/button without depending on PointerEvent constructor
    // support in happy-dom.
    target.dispatchEvent(new MouseEvent(type, { bubbles: true, ...init }));
  }

  // Attach to document.body so events dispatched on marks bubble up to the
  // document-level pointerdown/pointerup listeners the gate registers. The
  // default mount is detached and pointer events would never reach document.
  // Each wrapper goes on the `wrappers` list and is unmounted in afterEach
  // so its document-level listeners are removed before the next test; without
  // this, stale wrappers across tests would all see each other's pointer
  // events and a future regression test could observe spurious state changes.
  const wrappers = [];
  afterEach(() => {
    while (wrappers.length) wrappers.pop().unmount();
  });
  async function mountedWithMark() {
    const wrapper = mount(AnnotatedText, {
      props: {
        article: ART,
        notesForArticle: [
          { note: noteVerzekerde, spans: [{ start: 7, end: 17 }] },
        ],
      },
      attachTo: document.body,
      global: { stubs: nlddStubs },
    });
    wrappers.push(wrapper);
    await nextTick();
    await nextTick();
    return {
      wrapper,
      mark: wrapper.element.querySelector('mark[data-primary-idx]'),
    };
  }

  it('opens the popover on a hover without any prior pointerdown', async () => {
    const { wrapper, mark } = await mountedWithMark();
    fire(mark, 'pointerover');
    expect(wrapper.vm.activeNote).toBeTruthy();
  });

  it('does not open the popover when pointerover hits a mark mid-drag', async () => {
    const { wrapper, mark } = await mountedWithMark();
    fire(document, 'pointerdown', { button: 0, clientX: 10, clientY: 10 });
    fire(mark, 'pointerover');
    expect(wrapper.vm.activeNote).toBeNull();
  });

  it('does not open the popover on focusin during a drag', async () => {
    const { wrapper, mark } = await mountedWithMark();
    fire(document, 'pointerdown', { button: 0, clientX: 10, clientY: 10 });
    mark.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(wrapper.vm.activeNote).toBeNull();
  });

  it('opens the popover on focusin via keyboard (no prior pointerdown)', async () => {
    const { wrapper, mark } = await mountedWithMark();
    mark.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(wrapper.vm.activeNote).toBeTruthy();
  });

  it('does not pin the popover on pointerup when the pointer moved (real drag)', async () => {
    const { wrapper, mark } = await mountedWithMark();
    fire(document, 'pointerdown', { button: 0, clientX: 10, clientY: 10 });
    fire(mark, 'pointerup', { button: 0, clientX: 80, clientY: 50 });
    expect(wrapper.vm.activeNote).toBeNull();
  });

  it('pins the popover on pointerup when the pointer did not move (tap on mark)', async () => {
    const { wrapper, mark } = await mountedWithMark();
    fire(document, 'pointerdown', { button: 0, clientX: 20, clientY: 20 });
    fire(mark, 'pointerup', { button: 0, clientX: 21, clientY: 22 });
    expect(wrapper.vm.activeNote).toBeTruthy();
  });

  it('releases the drag flag on pointercancel so later hovers open again', async () => {
    const { wrapper, mark } = await mountedWithMark();
    fire(document, 'pointerdown', { button: 0, clientX: 10, clientY: 10 });
    fire(document, 'pointercancel');
    fire(mark, 'pointerover');
    expect(wrapper.vm.activeNote).toBeTruthy();
  });

  it('does not open the popover when a non-collapsed selection lives inside the rich-text root', async () => {
    const { wrapper, mark } = await mountedWithMark();
    // Stub getSelection to return a non-collapsed Selection anchored inside
    // the rich-text root - covers the post-mouseup path where the drag is
    // done but the selection is still standing.
    const fakeSel = {
      rangeCount: 1,
      isCollapsed: false,
      anchorNode: mark.firstChild ?? mark,
    };
    const orig = window.getSelection;
    window.getSelection = () => fakeSel;
    try {
      fire(mark, 'pointerover');
      expect(wrapper.vm.activeNote).toBeNull();
    } finally {
      window.getSelection = orig;
    }
  });

  it('ignores a secondary-button pointerup while a primary drag is in flight', async () => {
    // Right-button release mid-left-drag must not consume the left-drag's
    // start coords or clear isDragging - otherwise the remaining drag would
    // fall back to only the selection-based guard for the rest of the
    // gesture and a pointerover on a mark could re-open the popover before
    // the actual left release.
    const { wrapper, mark } = await mountedWithMark();
    fire(document, 'pointerdown', { button: 0, clientX: 10, clientY: 10 });
    fire(mark, 'pointerup', { button: 2, clientX: 12, clientY: 11 }); // secondary
    fire(mark, 'pointerover');
    expect(wrapper.vm.activeNote).toBeNull();
  });

  it('click-to-pin on one instance does not open the popover on another mounted instance', async () => {
    // Two AnnotatedText instances both register document-level pointerup
    // listeners. Without a containment guard, a tap on instance B's mark
    // would resolve to instance A's noteByIdx[idx] and pop A's popover
    // with the wrong note attached to B's DOM. The guard inside
    // markFromEvent rejects targets outside the instance's own
    // richTextEl, so A stays untouched.
    const { wrapper: wrapperA } = await mountedWithMark();
    const { wrapper: wrapperB, mark: markB } = await mountedWithMark();
    // Sanity: A's activeNote starts null.
    expect(wrapperA.vm.activeNote).toBeNull();
    fire(document, 'pointerdown', { button: 0, clientX: 20, clientY: 20 });
    fire(markB, 'pointerup', { button: 0, clientX: 21, clientY: 22 });
    expect(wrapperB.vm.activeNote).toBeTruthy(); // B's own click-to-pin
    expect(wrapperA.vm.activeNote).toBeNull(); // A must not have opened
  });

  it('closes an already-open popover immediately on a new pointerdown', async () => {
    const { wrapper, mark } = await mountedWithMark();
    fire(mark, 'pointerover');
    expect(wrapper.vm.activeNote).toBeTruthy();
    // Starting a drag while the hover-popover is up must drop the popover
    // straight away so the in-flight drag-select is not anchored over the
    // mark the popover is attached to.
    fire(document, 'pointerdown', { button: 0, clientX: 10, clientY: 10 });
    expect(wrapper.vm.activeNote).toBeNull();
  });
});

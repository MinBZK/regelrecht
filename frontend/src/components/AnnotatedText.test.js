import { describe, it, expect, beforeEach, vi } from 'vitest';
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

// Component-level tests for the two DOM-bound behaviours the pure
// useNotesHighlight tests cannot cover: idempotent re-apply (clear before
// re-wrap) and the deterministic overlap drop. The markdown render + offset
// alignment themselves are covered by useNotesHighlight.test.js.

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

  it('drops a later note overlapping an earlier one (document order wins)', async () => {
    const wrapper = mountWith(ART, [
      // Earlier note (by raw start) wins; the overlapping one is dropped,
      // matching markRanges()'s deterministic behaviour.
      { note: noteVerzekerde, spans: [{ start: 7, end: 17 }] },
      { note: noteZorgtoeslag, spans: [{ start: 12, end: 25 }] },
    ]);
    await nextTick();
    await nextTick();
    const marks = wrapper.element.querySelectorAll('mark[data-note-idx]');
    expect(marks).toHaveLength(1);
    expect(marks[0].dataset.noteIdx).toBe('0');
    expect(marks[0].textContent).toBe('verzekerde');
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
    // Span from "eerste" (raw 3) through "tweede lid" — crosses the \n\n.
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
    // selection anchor — the pane is purely a read view.
    expect(wrapper.find('[data-testid="create-note-btn"]').exists()).toBe(
      false,
    );
    expect(wrapper.find('.sel-anchor').exists()).toBe(false);
    expect(wrapper.findComponent({ name: 'NoteCreator' }).exists()).toBe(false);
  });

  it('mounts the authoring path without disturbing the highlight render', async () => {
    const wrapper = mountWith(ART, [{ note: noteVerzekerde, spans: [] }], {
      canCreate: true,
      lawId: 'zorgtoeslagwet',
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
      lawId: 'zorgtoeslagwet',
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

import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import AnnotatedText from './AnnotatedText.vue';

// Component-level tests for the two DOM-bound behaviours the pure
// useNotesHighlight tests cannot cover: idempotent re-apply (clear before
// re-wrap) and the deterministic overlap drop. The markdown render + offset
// alignment themselves are covered by useNotesHighlight.test.js.

const nlddStubs = {
  'nldd-rich-text': { template: '<div><slot/></div>' },
  'nldd-popover': { template: '<div><slot/></div>' },
  'nldd-inline-dialog': { template: '<div/>' },
};

function mountWith(article, notesForArticle) {
  return mount(AnnotatedText, {
    props: { article, notesForArticle },
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
    const marks = wrapper.element.querySelectorAll('mark[data-note-idx]');
    expect(marks).toHaveLength(1);
    expect(marks[0].dataset.noteIdx).toBe('0');
    expect(marks[0].textContent).toBe('verzekerde');
  });

  it('renders no marks (clean markdown) when there are no notes', async () => {
    const wrapper = mountWith(ART, []);
    await nextTick();
    expect(
      wrapper.element.querySelectorAll('mark[data-note-idx]'),
    ).toHaveLength(0);
    // The list rendered: an <ol><li> from "1. ".
    expect(wrapper.element.querySelector('ol li')).toBeTruthy();
  });
});

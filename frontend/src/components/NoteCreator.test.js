import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import NoteCreator from './NoteCreator.vue';
import { useAuth } from '../composables/useAuth.js';

// NoteCreator pulls in useAmbiguityVocabulary, which fetches the vocabulary
// file on first use. Stub fetch so the test does not hit the network (and
// does not leave an aborted request dangling at teardown).
beforeEach(() => {
  localStorage.clear();
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: true,
    text: async () =>
      'ambiguity:\n  - id: open-norm-partial\n    label: Open norm, deels ingevuld\n',
  });
});

// NoteCreator builds the W3C Annotation a draft becomes. The body shape is
// motivation-dependent and is the contract useDraftNotes exports and the
// resolver re-reads, so it is worth pinning per motivation.

const stubs = {
  'nldd-popover': {
    template: '<div><slot/></div>',
    methods: { showPopover() {}, hidePopover() {}, matches() { return false; } },
  },
  'nldd-inline-dialog': { template: '<div/>' },
  'nldd-segmented-control': { template: '<div><slot/></div>' },
  'nldd-segmented-control-item': { template: '<div/>' },
  'nldd-text-field': { template: '<input/>' },
  'nldd-button': {
    props: ['disabled'],
    template: '<button :disabled="disabled" @click="$emit(\'click\')"><slot/></button>',
  },
};

const RAW = 'Indien de normpremie voor een verzekerde';

function mountCreator(extraProps = {}, extraStubs = {}) {
  const engine = {
    resolveNote: () => ({
      status: 'found',
      matches: [{ article_number: '1', start: 10, end: 20 }],
    }),
  };
  return mount(NoteCreator, {
    props: {
      range: { start: 10, end: 20 }, // "normpremie"
      rawText: RAW,
      lawId: 'wet_op_de_zorgtoeslag',
      article: {
        number: '1',
        machine_readable: {
          execution: { output: [{ name: 'hoogte_zorgtoeslag' }] },
        },
      },
      engine,
      anchor: document.createElement('span'),
      ...extraProps,
    },
    global: { stubs: { ...stubs, ...extraStubs } },
  });
}

describe('NoteCreator', () => {
  it('builds a commenting note with a TextualBody', async () => {
    const w = mountCreator();
    await nextTick();
    w.vm.commentText = 'uitleg over de normpremie';
    await nextTick();
    w.vm.save();
    const [[note]] = w.emitted('create');
    expect(note.motivation).toBe('commenting');
    expect(note.target.selector.exact).toBe('normpremie');
    expect(note.body).toEqual({
      type: 'TextualBody',
      value: 'uitleg over de normpremie',
      purpose: 'commenting',
      format: 'text/plain',
      language: 'nl',
    });
    expect(note.__draft).toBe(true);
  });

  it('builds a linking note with a SpecificResource body', async () => {
    const w = mountCreator();
    await nextTick();
    w.vm.linkMode = 'element';
    w.vm.linkTarget = 'hoogte_zorgtoeslag';
    await nextTick();
    w.vm.save();
    const [[note]] = w.emitted('create');
    expect(note.motivation).toBe('linking');
    expect(note.body).toEqual({
      type: 'SpecificResource',
      source: 'regelrecht://wet_op_de_zorgtoeslag/hoogte_zorgtoeslag#hoogte_zorgtoeslag',
      purpose: 'linking',
    });
  });

  it('combines a comment, ambiguity tag and open action into one note', async () => {
    const w = mountCreator();
    await nextTick();
    w.vm.commentText = 'is dit een open norm?';
    w.vm.ambiguityTag = 'open-norm-partial';
    w.vm.workflow = 'open';
    await nextTick();
    w.vm.save();
    const [[note]] = w.emitted('create');
    expect(note.workflow).toBe('open');
    expect(Array.isArray(note.body)).toBe(true);
    expect(note.body[1]).toEqual({
      type: 'TextualBody',
      value: 'open-norm-partial',
      purpose: 'tagging',
    });
  });

  it('builds a tagging note with just the tag', async () => {
    const w = mountCreator();
    await nextTick();
    w.vm.ambiguityTag = 'missing-document';
    await nextTick();
    w.vm.save();
    const [[note]] = w.emitted('create');
    expect(note.body).toEqual({
      type: 'TextualBody',
      value: 'missing-document',
      purpose: 'tagging',
    });
  });

  it('does not save while the selector is ambiguous', async () => {
    const w = mountCreator({
      engine: {
        resolveNote: () => ({ status: 'ambiguous', matches: [{}, {}] }),
      },
    });
    await nextTick();
    w.vm.commentText = 'iets';
    await nextTick();
    expect(w.vm.canSave).toBe(false);
    w.vm.save();
    expect(w.emitted('create')).toBeUndefined();
  });

  it('hides the whole form when the selection is unusable', async () => {
    const w = mountCreator({
      engine: {
        resolveNote: () => ({ status: 'orphaned', matches: [] }),
      },
    });
    await nextTick();
    // Only the warning — no type picker, no save. The user dismisses by
    // clicking outside (no in-form cancel button anymore).
    expect(w.find('[data-testid="note-creator-status"]').exists()).toBe(true);
    expect(w.find('[data-testid="note-save"]').exists()).toBe(false);
    expect(w.find('[data-testid="note-comment-text"]').exists()).toBe(false);
    expect(w.find('[data-testid="note-cancel"]').exists()).toBe(false);
  });

  it('shows the full form once the selection resolves uniquely', async () => {
    const w = mountCreator(); // engine returns a matching unique result
    await nextTick();
    expect(w.find('[data-testid="note-creator-status"]').exists()).toBe(false);
    expect(w.find('[data-testid="note-save"]').exists()).toBe(true);
  });

  // nldd-popover is popover="auto": clicking outside (or Esc) light-dismisses
  // it in the browser without going through cancel(). The component fires
  // `close` on every dismissal; NoteCreator must treat a close while the form
  // is still open as a cancel, or the parent keeps creatorOpen=true and the
  // "Notitie" button never reappears on a new selection.
  it('emits cancel when the popover closes via light-dismiss', async () => {
    const w = mountCreator();
    await nextTick();
    // Same shape as the real nldd close event (bubbles + composed).
    w.element.dispatchEvent(new Event('close', { bubbles: true, composed: true }));
    await nextTick();
    expect(w.emitted('cancel')).toHaveLength(1);
  });

  it('does not emit cancel for the close that follows its own teardown', async () => {
    const w = mountCreator();
    await nextTick();
    await w.setProps({ range: null }); // parent already tore the flow down
    w.element.dispatchEvent(new Event('close', { bubbles: true, composed: true }));
    await nextTick();
    expect(w.emitted('cancel')).toBeUndefined();
  });

  // A future nested nldd component in the form slot would dispatch its own
  // bubbling `close`; that must not cancel the half-filled form.
  it('ignores a close event bubbling up from inside the form', async () => {
    const w = mountCreator();
    await nextTick();
    w.find('[data-testid="note-creator"]').element.dispatchEvent(
      new Event('close', { bubbles: true, composed: true }),
    );
    await nextTick();
    expect(w.emitted('cancel')).toBeUndefined();
  });

  // If an nldd-popover implementation ever dispatched close synchronously
  // from hidePopover() (instead of the spec's queued toggle task), save()
  // must still not read as a cancel. The stub simulates that regime.
  it('does not emit cancel when save() itself hides the popover', async () => {
    const w = mountCreator({}, {
      'nldd-popover': {
        template: '<div><slot/></div>',
        methods: {
          showPopover() {},
          hidePopover() {
            this.$el.dispatchEvent(new Event('close', { bubbles: true, composed: true }));
          },
          matches() { return false; },
        },
      },
    });
    await nextTick();
    w.vm.commentText = 'uitleg';
    await nextTick();
    w.vm.save();
    await nextTick();
    expect(w.emitted('cancel')).toBeUndefined();
    expect(w.emitted('create')).toHaveLength(1);
  });

  it('sets the creator to the signed-in user (id + name)', async () => {
    const { person } = useAuth();
    const w = mountCreator();
    await nextTick();
    // `/auth/status` returns the identity as `sub` (no `id` field); creator.id
    // is derived from it.
    person.value = { sub: 'u1', name: 'J. de Vries' };
    w.vm.commentText = 'x';
    await nextTick();
    w.vm.save();
    const note = w.emitted('create')[0][0];
    expect(note.creator).toEqual({ id: 'u1', name: 'J. de Vries' });
    person.value = null; // reset shared module state for other tests
  });
});

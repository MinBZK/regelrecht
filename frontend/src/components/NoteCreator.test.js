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

  it('builds a linking note with a SpecificResource body per linked target', async () => {
    const w = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    // One element link and one document link -> two SpecificResource bodies.
    w.vm.links = [
      { type: 'element', value: 'hoogte_zorgtoeslag', label: 'hoogte_zorgtoeslag' },
      { type: 'document', value: 'mvt/concept.md', label: 'mvt/concept.md' },
    ];
    await nextTick();
    w.vm.save();
    const [[note]] = w.emitted('create');
    expect(note.motivation).toBe('linking');
    expect(note.body).toEqual([
      {
        type: 'SpecificResource',
        source: 'regelrecht://wet_op_de_zorgtoeslag/hoogte_zorgtoeslag#hoogte_zorgtoeslag',
        purpose: 'linking',
      },
      {
        type: 'SpecificResource',
        source: 'regelrecht://doc/t-123/mvt/concept.md',
        purpose: 'linking',
      },
    ]);
  });

  it('maps the task switches to the workflow and reveals "done" only for a task', async () => {
    const w = mountCreator();
    await nextTick();
    // "Afgehandeld" is hidden until the note is a task.
    expect(w.find('[data-testid="note-task-done"]').exists()).toBe(false);
    w.vm.commentText = 'x';
    w.vm.isTask = true;
    w.vm.taskDone = true;
    await nextTick();
    expect(w.find('[data-testid="note-task-done"]').exists()).toBe(true);
    w.vm.save();
    expect(w.emitted('create')[0][0].workflow).toBe('resolved');
  });

  it('combines a comment, ambiguity tag and open action into one note', async () => {
    const w = mountCreator();
    await nextTick();
    w.vm.commentText = 'is dit een open norm?';
    w.vm.ambiguityTags = ['open-norm-partial'];
    w.vm.isTask = true; // a task, not done -> workflow 'open'
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

  it('builds a tagging note with multiple ambiguity labels', async () => {
    const w = mountCreator();
    await nextTick();
    w.vm.ambiguityTags = ['missing-document', 'open-norm-partial'];
    await nextTick();
    w.vm.save();
    const [[note]] = w.emitted('create');
    expect(note.body).toEqual([
      { type: 'TextualBody', value: 'missing-document', purpose: 'tagging' },
      { type: 'TextualBody', value: 'open-norm-partial', purpose: 'tagging' },
    ]);
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

  it('emits the share intent as the second create arg (default off)', async () => {
    const w = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    w.vm.commentText = 'x';
    await nextTick();
    w.vm.save();
    expect(w.emitted('create')[0][1]).toBe(false);

    const w2 = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    w2.vm.commentText = 'x';
    w2.vm.shareWithTraject = true;
    await nextTick();
    w2.vm.save();
    expect(w2.emitted('create')[0][1]).toBe(true);
  });

  it('shows the irreversible-share warning only when the switch is on', async () => {
    const w = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    expect(w.find('[data-testid="note-share-warning"]').exists()).toBe(false);
    w.vm.shareWithTraject = true;
    await nextTick();
    expect(w.find('[data-testid="note-share-warning"]').exists()).toBe(true);
  });

  it('the share switch change event drives the ref and the warning', async () => {
    const w = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    w.find('[data-testid="note-share"]').element.dispatchEvent(
      new CustomEvent('change', { detail: { checked: true }, bubbles: true, composed: true }),
    );
    await nextTick();
    expect(w.vm.shareWithTraject).toBe(true);
    expect(w.find('[data-testid="note-share-warning"]').exists()).toBe(true);
  });
});

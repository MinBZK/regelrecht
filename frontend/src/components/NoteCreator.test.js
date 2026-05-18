import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import NoteCreator from './NoteCreator.vue';

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

function mountCreator(extraProps = {}) {
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
      lawId: 'zorgtoeslagwet',
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
    global: { stubs },
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
    w.vm.motivation = 'linking';
    w.vm.linkTarget = 'hoogte_zorgtoeslag';
    await nextTick();
    w.vm.save();
    const [[note]] = w.emitted('create');
    expect(note.motivation).toBe('linking');
    expect(note.body).toEqual({
      type: 'SpecificResource',
      source: 'regelrecht://zorgtoeslagwet/hoogte_zorgtoeslag#hoogte_zorgtoeslag',
      purpose: 'linking',
    });
  });

  it('builds a questioning note with question text + ambiguity tag', async () => {
    const w = mountCreator();
    await nextTick();
    w.vm.motivation = 'questioning';
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
    w.vm.motivation = 'tagging';
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

  it('remembers the creator across mounts via localStorage', async () => {
    const w = mountCreator();
    await nextTick();
    w.vm.creator = 'J. de Vries';
    w.vm.commentText = 'x';
    await nextTick();
    w.vm.save();
    expect(localStorage.getItem('regelrecht-note-creator')).toBe('J. de Vries');
  });
});

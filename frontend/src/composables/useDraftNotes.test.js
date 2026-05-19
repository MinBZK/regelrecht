import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';
import yaml from 'js-yaml';
import { useDraftNotes } from './useDraftNotes.js';
import { lastSavedPr } from './useEditorSession.js';

/** Stub the sidecar fetch exportYaml does. `committed` is the file's notes. */
function stubSidecar(committed) {
  globalThis.fetch = vi.fn().mockResolvedValue(
    committed == null
      ? { ok: false, status: 404 }
      : {
          ok: true,
          text: async () => yaml.dump({ annotations: committed }),
        },
  );
}

// useDraftNotes is the local-only authoring store (RFC-018 write path MVP):
// per-law localStorage, append/clear, and a committed+drafts YAML export.
// These pin the storage keying, the per-law isolation, and the export shape
// (the file a user commits by hand).

const NOTE = {
  type: 'Annotation',
  motivation: 'commenting',
  target: {
    source: 'regelrecht://zorgtoeslagwet',
    selector: { type: 'TextQuoteSelector', exact: 'normpremie' },
  },
  body: { type: 'TextualBody', value: 'uitleg', purpose: 'commenting' },
};

beforeEach(() => {
  localStorage.clear();
});

describe('useDraftNotes', () => {
  it('appends a draft and persists it under a per-law key', () => {
    const lawId = ref('zorgtoeslagwet');
    const { addDraft, drafts } = useDraftNotes(lawId);
    addDraft(NOTE);
    expect(drafts.value).toHaveLength(1);
    const raw = localStorage.getItem('regelrecht-draft-notes:zorgtoeslagwet');
    expect(JSON.parse(raw)).toHaveLength(1);
  });

  it('stamps created when the note has none', () => {
    const { addDraft } = useDraftNotes(ref('w'));
    const stored = addDraft(NOTE);
    expect(stored.created).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });

  it('keeps an explicit created timestamp', () => {
    const { addDraft } = useDraftNotes(ref('w'));
    const stored = addDraft({ ...NOTE, created: '2020-01-01T00:00:00Z' });
    expect(stored.created).toBe('2020-01-01T00:00:00Z');
  });

  it('isolates drafts per law', () => {
    localStorage.setItem(
      'regelrecht-draft-notes:lawA',
      JSON.stringify([NOTE]),
    );
    const a = useDraftNotes(ref('lawA'));
    expect(a.drafts.value).toHaveLength(1);
    const b = useDraftNotes(ref('lawB'));
    expect(b.drafts.value).toHaveLength(0);
  });

  it('removeDraft drops the right index', () => {
    const { addDraft, removeDraft, drafts } = useDraftNotes(ref('w'));
    addDraft({ ...NOTE, body: { ...NOTE.body, value: 'a' } });
    addDraft({ ...NOTE, body: { ...NOTE.body, value: 'b' } });
    removeDraft(0);
    expect(drafts.value).toHaveLength(1);
    expect(drafts.value[0].body.value).toBe('b');
  });

  it('clearDrafts empties storage', () => {
    const { addDraft, clearDrafts, drafts } = useDraftNotes(ref('w'));
    addDraft(NOTE);
    clearDrafts();
    expect(drafts.value).toHaveLength(0);
    expect(
      JSON.parse(localStorage.getItem('regelrecht-draft-notes:w')),
    ).toEqual([]);
  });

  it('exports the raw sidecar notes + drafts (preserving cross-law notes)', async () => {
    // The committed file carries a note for ANOTHER law (federated sidecar).
    // The resolver would drop it; exportYaml must NOT — it re-parses the file.
    const otherLawNote = {
      ...NOTE,
      target: { ...NOTE.target, source: 'regelrecht://andere_wet' },
    };
    stubSidecar([{ ...NOTE, creator: 'Dienst Toeslagen' }, otherLawNote]);
    const { addDraft, exportYaml } = useDraftNotes(ref('w'));
    addDraft({ ...NOTE, __draft: true });
    const doc = yaml.load(await exportYaml());
    expect(doc.$schema).toContain('annotation-schema.json');
    // 2 committed (incl. the other-law one) + 1 draft.
    expect(doc.annotations).toHaveLength(3);
    expect(doc.annotations[0].creator).toBe('Dienst Toeslagen');
    expect(doc.annotations[1].target.source).toBe('regelrecht://andere_wet');
    // The internal __draft marker must never reach the exported YAML.
    expect(JSON.stringify(doc)).not.toContain('__draft');
  });

  it('exports drafts only when there is no sidecar yet (404)', async () => {
    stubSidecar(null);
    const { addDraft, exportYaml } = useDraftNotes(ref('w'));
    addDraft({ ...NOTE, __draft: true });
    const doc = yaml.load(await exportYaml());
    expect(doc.annotations).toHaveLength(1);
  });

  it('does not fold long body strings in the export', async () => {
    stubSidecar(null);
    const longValue = 'x '.repeat(120).trim();
    const { addDraft, exportYaml } = useDraftNotes(ref('w'));
    addDraft({
      ...NOTE,
      body: { type: 'TextualBody', value: longValue, purpose: 'commenting' },
    });
    // lineWidth -1: the value stays on one logical line (no YAML fold marker).
    const reparsed = yaml.load(await exportYaml());
    expect(reparsed.annotations[0].body.value).toBe(longValue);
  });
});

// saveToRepo PUTs the exported sidecar to editor-api. fetch is called
// twice per save: the sidecar GET inside exportYaml, then the annotations
// PUT. This stub answers GET with a 404 (drafts-only export) and the PUT
// with the supplied response.
function stubSave(putResponse) {
  globalThis.fetch = vi.fn((url, opts) => {
    if (opts?.method === 'PUT') return Promise.resolve(putResponse);
    return Promise.resolve({ ok: false, status: 404 });
  });
}

describe('useDraftNotes.saveToRepo', () => {
  beforeEach(() => {
    lastSavedPr.value = null;
  });

  it('PUTs to the law annotations endpoint, clears drafts, sets the PR', async () => {
    stubSave({
      ok: true,
      json: async () => ({
        pr: { url: 'https://github.com/x/y/pull/7', number: 7, branch: 'b' },
      }),
    });
    const { addDraft, saveToRepo, drafts } = useDraftNotes(ref('zorgtoeslagwet'));
    addDraft({ ...NOTE, __draft: true });

    await saveToRepo();

    const putCall = globalThis.fetch.mock.calls.find(
      ([, o]) => o?.method === 'PUT',
    );
    expect(putCall[0]).toBe('/api/corpus/laws/zorgtoeslagwet/annotations');
    expect(putCall[1].headers['X-Editor-Session']).toBeTruthy();
    expect(drafts.value).toHaveLength(0);
    expect(lastSavedPr.value).toEqual({
      url: 'https://github.com/x/y/pull/7',
      number: 7,
      branch: 'b',
    });
  });

  it('appends ?source= when a target source is given', async () => {
    stubSave({ ok: true, json: async () => ({ pr: null }) });
    const { addDraft, saveToRepo } = useDraftNotes(ref('zorgtoeslagwet'));
    addDraft({ ...NOTE, __draft: true });

    await saveToRepo('amsterdam');

    const putCall = globalThis.fetch.mock.calls.find(
      ([, o]) => o?.method === 'PUT',
    );
    expect(putCall[0]).toBe(
      '/api/corpus/laws/zorgtoeslagwet/annotations?source=amsterdam',
    );
  });

  it('throws the editor-api message and keeps drafts on a 400', async () => {
    stubSave({
      ok: false,
      status: 400,
      headers: { get: () => 'text/plain; charset=utf-8' },
      text: async () => 'Note file is invalid:\n/annotations: required',
    });
    const { addDraft, saveToRepo, drafts } = useDraftNotes(ref('w'));
    addDraft({ ...NOTE, __draft: true });

    await expect(saveToRepo()).rejects.toThrow(/Note file is invalid/);
    expect(drafts.value).toHaveLength(1);
  });
});

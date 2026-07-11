import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref, nextTick } from 'vue';

// The manager orchestrates the lower-level useTrajectDocuments (which does the
// real network I/O). Mock that layer with controllable refs + spies so these
// tests exercise only the manager's own logic: rename-as-create+delete,
// delete-returns-open-flag, close, and untitled-name generation.
const h = vi.hoisted(() => ({ api: null }));
vi.mock('./useTrajectDocuments.js', () => ({
  useTrajectDocuments: () => h.api,
}));

import { useDocumentsManager } from './useDocumentsManager.js';

function makeApi(overrides = {}) {
  const documents = ref([]);
  const currentPath = ref(null);
  const currentBody = ref('');
  const currentEtag = ref(null);
  return {
    documents,
    loading: ref(false),
    listError: ref(null),
    currentPath,
    currentBody,
    savedBody: ref(''),
    currentEtag,
    docLoading: ref(false),
    docError: ref(null),
    saving: ref(false),
    saveError: ref(null),
    conflict: ref(null),
    deletedRemotely: ref(null),
    openDocument: vi.fn(async (p) => {
      currentPath.value = p;
    }),
    saveCurrent: vi.fn(async () => ({ ok: true })),
    reloadCurrent: vi.fn(async () => {}),
    createDocument: vi.fn(async (p) => {
      currentPath.value = p;
      return { ok: true, created: true };
    }),
    deleteDocument: vi.fn(async () => ({ ok: true })),
    dropDraft: vi.fn(),
    ...overrides,
  };
}

let m;
beforeEach(() => {
  h.api = makeApi();
  m = useDocumentsManager(ref('traject-abc12345'));
});

describe('useDocumentsManager', () => {
  it('displayTitle hides .md but keeps .txt visible', () => {
    expect(m.displayTitle('beleid.md')).toBe('beleid');
    expect(m.displayTitle('nota/bijlage.txt')).toBe('nota/bijlage.txt');
    expect(m.displayTitle(null)).toBe('');
  });

  it('onTitleInput sanitizes the name to a valid path (no error shown)', () => {
    m.onTitleInput({ target: { value: 'Beleid Nota' } });
    expect(m.titleDraft.value).toBe('beleid-nota'); // uppercase -> lower, space -> '-'
    m.onTitleInput({ target: { value: 'ABC_123.def' } });
    expect(m.titleDraft.value).toBe('abc_123.def'); // letters/digits/._ kept
    m.onTitleInput({ target: { value: 'map/Sub Doc' } });
    expect(m.titleDraft.value).toBe('map/sub-doc'); // '/' kept as folder separator
    m.onTitleInput({ detail: { value: 'Rare!!Tekens@hier' } });
    expect(m.titleDraft.value).toBe('rare-tekens-hier'); // runs of invalid chars -> single '-'
    expect(m.titleError.value).toBe(null);
  });

  it('typing the name clears a stale title error', () => {
    m.titleError.value = 'Een document met deze naam bestaat al.';
    m.onTitleInput({ target: { value: 'nieuwe-naam' } });
    expect(m.titleError.value).toBe(null);
  });

  it('startNew creates the next free untitled path and returns it', async () => {
    h.api.documents.value = [{ path: 'untitled.md' }, { path: 'untitled-2.md' }];
    const path = await m.startNew();
    expect(path).toBe('untitled-3.md');
    expect(h.api.createDocument).toHaveBeenCalledWith('untitled-3.md');
  });

  it('handleSave without a rename just saves the current path', async () => {
    h.api.currentPath.value = 'beleid.md';
    await nextTick(); // watch initialises titleDraft from the path
    const ok = await m.handleSave();
    expect(ok).toBe(true);
    expect(h.api.saveCurrent).toHaveBeenCalledTimes(1);
    expect(h.api.deleteDocument).not.toHaveBeenCalled();
  });

  it('handleSave with a rename writes the new path then deletes the old one', async () => {
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'nieuw';
    const ok = await m.handleSave();
    expect(ok).toBe(true);
    // New path adopted before the blind create (ifMatch: null).
    expect(h.api.currentPath.value).toBe('nieuw.md');
    expect(h.api.saveCurrent).toHaveBeenCalledWith({ ifMatch: null });
    expect(h.api.deleteDocument).toHaveBeenCalledWith('oud.md');
  });

  it('handleSave keeps the rename but flags the orphan when the old delete fails', async () => {
    h.api.deleteDocument.mockResolvedValueOnce({ ok: false });
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'nieuw';
    const ok = await m.handleSave();
    // The rename itself succeeded — content is saved under the new name.
    expect(ok).toBe(true);
    expect(h.api.currentPath.value).toBe('nieuw.md');
    expect(h.api.deleteDocument).toHaveBeenCalledWith('oud.md');
    // ...but the old file could not be removed, so the orphan is surfaced.
    expect(m.titleError.value).toMatch(/oude bestand kon niet worden verwijderd/i);
  });

  it('handleSave refuses a rename onto an existing document', async () => {
    h.api.documents.value = [{ path: 'bezet.md' }];
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'bezet';
    const ok = await m.handleSave();
    expect(ok).toBe(false);
    expect(m.titleError.value).toMatch(/bestaat al/i);
    expect(h.api.saveCurrent).not.toHaveBeenCalled();
    expect(h.api.currentPath.value).toBe('oud.md'); // unchanged
  });

  it('handleSave rejects an invalid name without saving', async () => {
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'Bad Name'; // spaces + capitals are not allowed
    const ok = await m.handleSave();
    expect(ok).toBe(false);
    expect(m.titleError.value).toBeTruthy();
    expect(h.api.saveCurrent).not.toHaveBeenCalled();
  });

  it('confirmDelete reports true only when the open document was removed', async () => {
    h.api.currentPath.value = 'open.md';
    await nextTick();
    m.askDelete('open.md');
    expect(await m.confirmDelete()).toBe(true);

    m.askDelete('ander.md');
    expect(await m.confirmDelete()).toBe(false);
  });

  it('confirmDelete surfaces a notice on a 412 conflict and reports false', async () => {
    h.api.deleteDocument.mockResolvedValueOnce({ ok: false, conflict: true });
    h.api.currentPath.value = 'open.md';
    await nextTick();
    m.askDelete('open.md');
    const removed = await m.confirmDelete();
    expect(removed).toBe(false);
    expect(m.deleteNotice.value).toMatch(/gewijzigd/i);
  });

  it('close clears the open document and resets the dirty baseline', () => {
    h.api.currentPath.value = 'open.md';
    h.api.currentBody.value = '# hi';
    h.api.savedBody.value = '# hi';
    h.api.currentEtag.value = 'etag-1';
    m.close();
    expect(h.api.currentPath.value).toBeNull();
    expect(h.api.currentBody.value).toBe('');
    expect(h.api.savedBody.value).toBe('');
    expect(h.api.currentEtag.value).toBeNull();
    // Without resetting savedBody, hasChanges would stay true and the next
    // navigation would trip the unsaved-changes guard.
    expect(m.hasChanges.value).toBe(false);
  });

  it('hasChanges is true when the body diverges from the saved baseline', () => {
    h.api.currentBody.value = '# edited';
    h.api.savedBody.value = '# original';
    expect(m.hasChanges.value).toBe(true);
  });

  it('hasChanges stays false while loading or on a blocking load error', () => {
    h.api.currentBody.value = '# edited';
    h.api.savedBody.value = '# original';
    h.api.docLoading.value = true;
    expect(m.hasChanges.value).toBe(false);
    h.api.docLoading.value = false;
    h.api.docError.value = { kind: 'load-error', message: 'x' };
    expect(m.hasChanges.value).toBe(false);
  });

  it('hasChanges stays true on a draft-present notice (Save button must show)', () => {
    // A local draft diverging from the server is unsaved changes, not a blocking
    // error — mirrors DocumentEditor.blockingError, which does not block on it.
    h.api.currentBody.value = '# draft';
    h.api.savedBody.value = '# server';
    h.api.docError.value = { kind: 'draft-present', message: 'x', serverBody: '# server' };
    expect(m.hasChanges.value).toBe(true);
  });
});

/**
 * useDocumentsManager — the full werkdocumenten editing logic (list + active
 * document + title/save/delete/conflict/draft handling), lifted out of the old
 * TrajectDocuments window so it can back BOTH the in-sheet master-detail and
 * the standalone navigation-split-view page.
 *
 * State is per-call (one instance per consumer tree). The sheet owns one
 * instance shared by its list + editor; the standalone page (a separate
 * browser tab) owns its own. Anything purely presentational (which view is
 * shown, sheet open/close) stays in the components.
 */
import { computed, ref, watch } from 'vue';
import { useTrajectDocuments } from './useTrajectDocuments.js';

export function useDocumentsManager(trajectRef) {
  const docs = useTrajectDocuments(trajectRef);
  const {
    documents,
    loading: listLoading,
    listError,
    currentPath,
    currentBody,
    savedBody,
    currentEtag,
    docLoading,
    docError,
    saving,
    saveError,
    conflict,
    deletedRemotely,
    openDocument,
    saveCurrent,
    reloadCurrent,
    createDocument,
    deleteDocument,
    dropDraft,
  } = docs;

  // The body flows straight to currentBody; the nldd-text-editor is a hybrid
  // Markdown editor (live-styled source), so there is no separate preview mode.
  function onBodyInput(e) {
    currentBody.value = e.detail?.value ?? e.target?.value ?? currentBody.value;
  }

  // Unsaved changes: the live body differs from the last-saved baseline. Drives
  // the editor's footer toolbar (shown only when dirty) and the navigate-away
  // guard. Guard on docError and docLoading: a document that failed to load or is
  // still loading can never be dirty. While loading, currentPath is already set
  // (so the shell shows) but currentBody still holds the previous document, which
  // would otherwise register as a change and flash the Save button + leave-guard.
  const hasChanges = computed(
    () => !docError.value && !docLoading.value && currentBody.value !== savedBody.value,
  );

  // --- Titels ---
  // '.md' blijft verborgen voor de gebruiker; '.txt' wijkt af van de default en
  // blijft daarom zichtbaar.
  function displayTitle(path) {
    return path ? path.replace(/\.md$/, '') : '';
  }
  function pathFromTitle(title) {
    const t = title.trim();
    if (!t) return '';
    return /\.(md|txt)$/.test(t) ? t : `${t}.md`;
  }
  // Lightweight client-side validation mirroring the backend rules.
  function validatePath(value) {
    if (!value) return 'Geef een naam op.';
    if (value.startsWith('/')) return "Naam mag niet beginnen met '/'.";
    if (value.includes('\\')) return 'Naam mag geen backslashes bevatten.';
    const segments = value.split('/');
    for (const seg of segments) {
      if (!seg) return 'Naam bevat lege segmenten.';
      if (seg.startsWith('.')) return "Naam mag geen verborgen segmenten ('.') bevatten.";
      if (!/^[a-z0-9._-]+$/.test(seg)) {
        return "Gebruik alleen kleine letters, cijfers en '._-'.";
      }
    }
    return null;
  }

  // --- Open / nieuw ---
  async function open(path) {
    await openDocument(path);
  }

  const creating = ref(false);
  function nextUntitledPath() {
    let path = 'untitled.md';
    for (let n = 2; documents.value.some((d) => d.path === path); n++) {
      path = `untitled-${n}.md`;
    }
    return path;
  }
  // Creates an untitled document and loads it; the backend allows only
  // [a-z0-9._-] in paths. Returns the new path so the caller can route to it.
  async function startNew() {
    if (creating.value) return null;
    creating.value = true;
    const path = nextUntitledPath();
    try {
      await createDocument(path);
      return path;
    } finally {
      creating.value = false;
    }
  }

  // Close the open document without deleting it — clears the editor back to
  // "nothing selected". Used by the standalone page's back affordance (on a
  // narrow viewport the split view stacks, so "terug" returns to the list).
  function close() {
    currentPath.value = null;
    currentBody.value = '';
    // Reset the saved baseline too — otherwise hasChanges stays true (empty
    // body vs the just-closed document's body) and the next navigation trips
    // the unsaved-changes guard spuriously.
    savedBody.value = '';
    currentEtag.value = null;
  }

  // --- Titel bewerken ---
  const titleDraft = ref('');
  const titleError = ref(null);
  watch(currentPath, (p) => {
    titleDraft.value = displayTitle(p);
    titleError.value = null;
  });
  function onTitleInput(e) {
    const raw = e.detail?.value ?? e.target?.value ?? titleDraft.value;
    // Sanitize to a valid path as the user types instead of rejecting invalid
    // input: lowercase everything and turn any space or other disallowed
    // character into '-'. '/' is kept as a folder separator. This keeps the name
    // always valid, so the "Gebruik alleen kleine letters…" error never appears.
    titleDraft.value = raw.toLowerCase().replace(/[^a-z0-9._/-]+/g, '-');
    // Editing the name clears any stale validation notice (e.g. a duplicate-name
    // error from a prior save attempt).
    titleError.value = null;
  }

  async function handleSave() {
    if (saving.value) return false;
    titleError.value = null;
    const finalPath = pathFromTitle(titleDraft.value);
    const err = validatePath(finalPath);
    if (err) {
      titleError.value = err;
      return false;
    }
    if (finalPath === currentPath.value) {
      const result = await saveCurrent();
      return !!result?.ok;
    }
    // Hernoemen: geen rename-API — schrijf eerst onder het nieuwe pad (blind
    // create) en verwijder daarna het oude pad. In die volgorde raakt een
    // mislukking nooit inhoud kwijt.
    //
    // Multi-user-kanttekening: de bestaat-al-check hieronder kijkt naar de
    // gecachte lijst. Maakt een andere sessie tussen die check en de PUT een
    // bestand op finalPath, dan overschrijft deze blinde write het zonder
    // waarschuwing. Sluitend te maken met `If-None-Match: *` zodra de backend
    // dat ondersteunt (zie useTrajectDocuments.saveCurrent).
    if (documents.value.some((d) => d.path === finalPath)) {
      titleError.value = 'Een document met deze naam bestaat al.';
      return false;
    }
    const oldPath = currentPath.value;
    const oldEtag = currentEtag.value;
    currentPath.value = finalPath;
    currentEtag.value = null;
    const result = await saveCurrent({ ifMatch: null });
    if (!result?.ok) {
      currentPath.value = oldPath;
      currentEtag.value = oldEtag;
      return false;
    }
    const deleted = await deleteDocument(oldPath);
    if (!deleted?.ok) {
      // The new name is saved, so content is never lost, but the old path
      // could not be removed and lingers on the server as an orphan copy.
      // Surface it so the user can delete it by hand instead of silently
      // leaving a duplicate behind.
      titleError.value =
        'Hernoemd en opgeslagen, maar het oude bestand kon niet worden verwijderd. Verwijder het handmatig.';
    }
    return true;
  }

  // "Maak alle wijzigingen ongedaan": gooi de lokale draft weg, laad de server-versie.
  function undoChanges() {
    titleDraft.value = displayTitle(currentPath.value);
    titleError.value = null;
    return reloadCurrent();
  }
  // Resolve a 412 conflict by force-overwriting with `If-Match: *`.
  function overwriteServer() {
    return saveCurrent({ ifMatch: '*' });
  }

  // --- Delete ---
  const pendingDeletePath = ref(null);
  const deleteNotice = ref(null);
  function askDelete(path) {
    if (path) pendingDeletePath.value = path;
  }
  function cancelDelete() {
    if (pendingDeletePath.value === null) return;
    pendingDeletePath.value = null;
  }
  // Resolves to `true` when the currently open document was the one deleted, so
  // the caller can return to the list / navigate away.
  async function confirmDelete() {
    const path = pendingDeletePath.value;
    pendingDeletePath.value = null;
    if (!path) return false;
    deleteNotice.value = null;
    const wasOpenDocument = path === currentPath.value;
    const result = await deleteDocument(path);
    if (result?.ok) {
      return wasOpenDocument;
    }
    if (result?.conflict) {
      deleteNotice.value =
        `"${displayTitle(path)}" is intussen door iemand anders gewijzigd; de lijst is ververst. ` +
        `Open het document om de huidige versie te zien voordat je het verwijdert.`;
    } else {
      deleteNotice.value =
        saveError.value?.message || `Verwijderen van "${displayTitle(path)}" is mislukt.`;
    }
    return false;
  }

  return {
    // state
    documents, listLoading, listError,
    currentPath, currentBody, hasChanges, docLoading, docError,
    saving, saveError, conflict, deletedRemotely,
    creating,
    titleDraft, titleError,
    pendingDeletePath, deleteNotice,
    // derived helpers
    displayTitle,
    // actions
    open, startNew, close,
    onBodyInput, onTitleInput,
    handleSave, undoChanges, overwriteServer,
    reloadCurrent, dropDraft,
    askDelete, cancelDelete, confirmDelete,
  };
}

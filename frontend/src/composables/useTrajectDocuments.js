/**
 * useTrajectDocuments - manage markdown / plain-text documents that
 * live alongside laws in a traject's corpus branch.
 *
 * Reads, writes and deletes go through `/api/trajects/{ref}/corpus/documents`
 * (handlers in editor-api). ETag/If-Match is used end-to-end for
 * conflict-detection: a 412 on save means "your view is stale, reload".
 * Tussentijdse edits worden gedebounced naar `localStorage` zodat een
 * pagina-refresh geen werk verliest, identiek aan het patroon dat
 * `useDraftNotes` voor notities aanhoudt.
 *
 * @param {import('vue').Ref<string|null>} trajectRef Active traject ref.
 *   Required for every operation here - documents only exist under a
 *   traject scope (there is no global counterpart).
 */
import { ref, watch } from 'vue';
import {
  documentsListUrl,
  documentFileUrl,
  documentUploadUrl,
  requireTraject,
} from './corpusUrls.js';
import { apiFetchJson } from '../lib/apiFetch.js';
import { useLatest } from '../lib/useLatest.js';

const STORAGE_PREFIX = 'regelrecht-doc-draft:';
const DRAFT_DEBOUNCE_MS = 500;

function draftKey(trajectRef, docPath) {
  return `${STORAGE_PREFIX}${trajectRef}:${docPath}`;
}

function loadDraft(trajectRef, docPath) {
  if (!trajectRef || !docPath) return null;
  try {
    return localStorage.getItem(draftKey(trajectRef, docPath));
  } catch {
    return null;
  }
}

function persistDraft(trajectRef, docPath, text) {
  try {
    localStorage.setItem(draftKey(trajectRef, docPath), text);
  } catch {
    /* quota / unavailable - drafts are best-effort */
  }
}

function clearDraft(trajectRef, docPath) {
  try {
    localStorage.removeItem(draftKey(trajectRef, docPath));
  } catch {
    /* ignore */
  }
}

export function useTrajectDocuments(trajectRef) {
  // Top-level state - the list of documents in the current traject.
  const documents = ref([]);
  const loading = ref(false);
  const listError = ref(null);

  // Per-open-document state. One document is loaded into the editor at
  // a time; switching to another document overwrites these.
  const currentPath = ref(null);
  const currentBody = ref('');
  // The last body known to be persisted on the server - the baseline for the
  // "unsaved changes" (hasChanges) check. Set on load and after a save.
  const savedBody = ref('');
  // The ETag we received on the last successful read or save. Sent back
  // as `If-Match` on the next PUT/DELETE so the server can detect a
  // concurrent edit from another tab/user.
  const currentEtag = ref(null);
  const docLoading = ref(false);
  const docError = ref(null);
  const saving = ref(false);
  const saveError = ref(null);
  // Set to a localised string when a save returned 412 - the editor
  // surfaces a conflict banner and lets the user choose
  // "lokaal overschrijven" or "server-versie laden".
  const conflict = ref(null);
  // Set to a localised string when a save/overwrite returned 412 because
  // the document no longer exists on the server (deleted between the
  // initial conflict and the "Lokaal overschrijven" click). Distinct from
  // `conflict`: the concurrent-edit resolution buttons are a dead end here,
  // so the editor shows a separate "document is verwijderd" banner instead.
  const deletedRemotely = ref(null);

  async function fetchList() {
    if (!trajectRef.value) {
      documents.value = [];
      return;
    }
    loading.value = true;
    listError.value = null;
    try {
      const json = await apiFetchJson(documentsListUrl(trajectRef.value), {
        errorMessage: (status) => `Lijst ophalen mislukt: ${status}`,
      });
      documents.value = Array.isArray(json?.documents) ? json.documents : [];
    } catch (e) {
      listError.value = e;
      documents.value = [];
    } finally {
      loading.value = false;
    }
  }

  // Discards out-of-order `openDocument` responses. Clicking document A
  // then B fires two concurrent fetches; if A resolves after B, its
  // (stale) response must not clobber B's loaded state.
  const claimOpen = useLatest();

  async function openDocument(path) {
    requireTraject(trajectRef.value, 'document open');
    const isCurrent = claimOpen();
    docLoading.value = true;
    docError.value = null;
    saveError.value = null;
    conflict.value = null;
    deletedRemotely.value = null;
    // Show the document shell immediately: adopt the path now, before the fetch,
    // so hasOpenDoc flips true and the editor renders its "Document laden"
    // indicator right away instead of leaving the user on the old view until a
    // (possibly slow) fetch resolves. The body follows when the fetch lands; the
    // editor stays behind the indicator while docLoading, so the previous
    // document's stale body is never shown. Only currentPath changes here (not
    // currentBody), so the draft watch doesn't fire.
    currentPath.value = path;
    // Cancel any debounce that was scheduled by the previous document's
    // last keystroke: when the watch fires after we swap `currentPath`
    // it would otherwise persist the new body under the old path.
    cancelDraftTimer();
    try {
      // Raw fetch on purpose: every status maps to its own docError shape
      // below (404, other non-ok, draft divergence) - a thrown error would
      // collapse those branches.
      const res = await fetch(documentFileUrl(trajectRef.value, path));
      // A newer openDocument started while this fetch was in flight - drop
      // this stale response so it can't overwrite the newer document.
      if (!isCurrent()) return;
      if (res.status === 404) {
        currentPath.value = path;
        currentBody.value = '';
        // Reset the baseline too so an empty body isn't seen as a change vs the
        // previous document's saved content (spurious dirty state).
        savedBody.value = '';
        currentEtag.value = null;
        // Distinct kind from the generic 'load-error' below: a document-task
        // review (useDocumentTaskReview) needs to tell "doesn't exist yet -
        // seed the proposal as a new document" apart from a real fetch
        // failure. DocumentEditor's blockingError check treats both the same
        // (`kind !== 'draft-present'`), so this is not a behaviour change for
        // the normal not-a-review flow.
        docError.value = {
          kind: 'not-found',
          message: 'Document niet gevonden',
        };
        return;
      }
      if (!res.ok) {
        // Keep state consistent with the URL the consumer already
        // navigated to: adopt the new path with an empty body so a
        // subsequent Save can't PUT stale content back to the old path.
        currentPath.value = path;
        currentBody.value = '';
        // Reset the baseline too (see 404 branch) so the failed load doesn't
        // register as unsaved changes against the previous document.
        savedBody.value = '';
        currentEtag.value = null;
        docError.value = {
          kind: 'load-error',
          message: `Document ophalen mislukt: ${res.status}`,
        };
        return;
      }
      const serverBody = await res.text();
      // Re-check after the body read - another open may have superseded us.
      if (!isCurrent()) return;
      const serverEtag = res.headers.get('ETag');

      // Set the path + body atomically (post-await) so the debounce
      // watch only fires on user input, not on this controlled swap.
      const draft = loadDraft(trajectRef.value, path);
      cancelDraftTimer();
      currentPath.value = path;
      currentEtag.value = serverEtag;
      savedBody.value = serverBody;
      // If the user had an unsaved draft for this document, prefer it
      // over the server body but flag the divergence so the editor can
      // offer "drop draft, keep server version".
      if (draft !== null && draft !== serverBody) {
        currentBody.value = draft;
        docError.value = {
          kind: 'draft-present',
          message:
            'Niet-opgeslagen wijzigingen geladen uit lokale opslag. Klik op Opslaan om door te zetten of op Draft verwerpen om de versie van de server te tonen.',
          serverBody,
        };
      } else {
        currentBody.value = serverBody;
      }
    } catch (e) {
      if (isCurrent()) docError.value = e;
    } finally {
      // Only the latest open controls the loading flag, so a stale
      // response settling late can't flip it off mid-load.
      if (isCurrent()) docLoading.value = false;
    }
  }

  // Discard local changes for the open document: drop the saved draft AND
  // revert the in-memory body to the last-saved baseline, so a discard truly
  // reverts instead of only clearing localStorage (otherwise the in-memory edit
  // lingers, re-drafts on the next keystroke, and trips the leave-guard again).
  function dropDraft() {
    if (!currentPath.value) return;
    // Cancel a pending debounced flush so it can't re-create the row we clear.
    cancelDraftTimer();
    clearDraft(trajectRef.value, currentPath.value);
    // savedBody is the server version (set on open/save); on a draft-present
    // notice it equals docError.serverBody, so "keep server version" still holds.
    currentBody.value = savedBody.value;
    docError.value = null;
  }

  // Debounced localStorage write on every typed character. We attach
  // this as a watch on currentBody so the consumer can simply v-model
  // the textarea against currentBody and get the draft persistence for
  // free.
  let draftTimer = null;
  function cancelDraftTimer() {
    if (draftTimer) {
      clearTimeout(draftTimer);
      draftTimer = null;
    }
  }
  watch(currentBody, (text) => {
    if (!currentPath.value || !trajectRef.value) return;
    cancelDraftTimer();
    draftTimer = setTimeout(() => {
      persistDraft(trajectRef.value, currentPath.value, text);
    }, DRAFT_DEBOUNCE_MS);
  });

  /**
   * Save the current body. Honors `currentEtag` as `If-Match` so a
   * concurrent edit surfaces as a conflict instead of a silent overwrite.
   * Pass `{ ifMatch: '*' }` to force-overwrite whatever version exists
   * (used by `overwriteServer` in the conflict-resolution path); note `*`
   * still 412s when the file does not exist, so it cannot create. The
   * create flow instead passes `{ ifMatch: null }` - a blind write with no
   * precondition, which is what lands a brand-new file.
   */
  async function saveCurrent({ ifMatch } = {}) {
    if (!currentPath.value) return;
    requireTraject(trajectRef.value, 'document save');
    // Drop any pending draft flush - if it fires after `clearDraft`
    // below, it'd re-create the localStorage row we just removed and
    // leak a phantom draft for the next open.
    cancelDraftTimer();
    saving.value = true;
    saveError.value = null;
    conflict.value = null;
    deletedRemotely.value = null;
    const headers = {
      'Content-Type': currentPath.value.endsWith('.txt')
        ? 'text/plain; charset=utf-8'
        : 'text/markdown; charset=utf-8',
    };
    const ifMatchValue = ifMatch ?? currentEtag.value;
    if (ifMatchValue) headers['If-Match'] = ifMatchValue;
    try {
      // Raw fetch on purpose: this is a result-object API ({ ok, conflict,
      // deleted }) where 412 and other non-ok statuses each map to their
      // own branch instead of a thrown error.
      const res = await fetch(
        documentFileUrl(trajectRef.value, currentPath.value),
        {
          method: 'PUT',
          headers,
          body: currentBody.value,
        },
      );
      if (res.status === 412) {
        // The backend returns 412 for two distinct preconditions: a stale
        // ETag (someone else edited) and an `If-Match: *` against a file
        // that no longer exists (someone deleted it). Discriminate on what
        // WE sent rather than parsing the server's (localisable) error
        // string: the backend only emits the "deleted" 412 for `If-Match: *`
        // against a missing file - i.e. exactly the force-overwrite path -
        // so `ifMatchValue === '*'` here is an unambiguous, language-stable
        // signal. The deleted case is a dead end for the concurrent-edit
        // buttons ("Server-versie laden" 404s, "Lokaal overschrijven" 412s
        // again), so surface a distinct, actionable banner instead.
        if (ifMatchValue === '*') {
          deletedRemotely.value =
            'Dit document is intussen verwijderd op de server. Je wijzigingen ' +
            'staan nog lokaal bewaard; sluit dit venster en maak het document ' +
            'opnieuw aan om ze terug te zetten.';
          return { ok: false, deleted: true };
        }
        conflict.value =
          'Het document is intussen door iemand anders gewijzigd. ' +
          'Kies "Server-versie laden" om de nieuwe versie te zien of ' +
          '"Lokaal overschrijven" om jouw wijzigingen door te zetten.';
        return { ok: false, conflict: true };
      }
      if (!res.ok) {
        const text = await safeText(res);
        saveError.value = new Error(text || `Opslaan mislukt: ${res.status}`);
        return { ok: false };
      }
      const json = await safeJson(res);
      // Refresh ETag from the response so a subsequent save chains
      // correctly. The header is authoritative; the body echo is a
      // convenience for clients that can't read headers.
      currentEtag.value = res.headers.get('ETag') ?? json?.etag ?? currentEtag.value;
      clearDraft(trajectRef.value, currentPath.value);
      savedBody.value = currentBody.value;
      // Reload the list - a freshly created document needs to appear
      // in the sidebar without a manual refresh (await: the caller may
      // navigate to it). An updated document refreshes non-blocking so a
      // changed frontmatter title doesn't linger stale in the sidebar.
      if (res.status === 201) {
        await fetchList();
      } else {
        fetchList().catch(() => {});
      }
      return { ok: true, created: res.status === 201, pr: json?.pr ?? null };
    } catch (e) {
      saveError.value = e;
      return { ok: false };
    } finally {
      saving.value = false;
    }
  }

  /** Force-replace the local body with whatever the server currently
   *  holds - used as the resolution path for a 412 conflict. */
  async function reloadCurrent() {
    if (!currentPath.value) return;
    clearDraft(trajectRef.value, currentPath.value);
    await openDocument(currentPath.value);
  }

  /**
   * Create a new document at `path`. Generates a minimal H1 template
   * body and PUTs it without `If-Match`, so a brand-new file lands at
   * `200/201 OK`. The caller (`useDocumentsManager.startNew`) only ever
   * passes a freshly generated `untitled-*.md` path that isn't in the
   * already-fetched list, so a collision needs a concurrent create of the
   * same name between list-refresh and submit - a race a future iteration
   * can close by adding `If-None-Match: *` support to the backend.
   */
  async function createDocument(path) {
    requireTraject(trajectRef.value, 'document create');
    // The template lives client-side: keeps the backend's create path
    // empty-body-tolerant and gives users an editable starting point
    // they can immediately replace.
    const stem = path.split('/').pop().replace(/\.[^.]+$/, '');
    const body = `# ${stem}\n\n`;
    cancelDraftTimer();
    currentPath.value = path;
    currentBody.value = body;
    currentEtag.value = null;
    const result = await saveCurrent({ ifMatch: null });
    return result;
  }

  /**
   * Upload a PDF/Word document. Stores the bytes server-side and enqueues an
   * async conversion-to-markdown job; the resulting `.md` appears in the list
   * once the job completes (surfaced meanwhile by the conversion-status
   * poller). Returns a result object `{ ok, targetPath }`.
   */
  async function uploadDocument(file) {
    requireTraject(trajectRef.value, 'document upload');
    saveError.value = null;
    const form = new FormData();
    form.append('file', file);
    try {
      // Raw fetch, result-object style like `saveCurrent`. Do NOT set
      // Content-Type - the browser adds the multipart boundary itself.
      const res = await fetch(documentUploadUrl(trajectRef.value), {
        method: 'POST',
        body: form,
      });
      if (!res.ok) {
        // 404/405/501 mean the backend doesn't offer the upload endpoint (the
        // conversion feature isn't enabled yet) - a human message, and retrying
        // won't help. Other statuses keep the server's text (or the code) and
        // stay retryable.
        const unsupported = res.status === 404 || res.status === 405 || res.status === 501;
        const text = await safeText(res);
        const error = unsupported
          ? 'Uploaden wordt door de server nog niet ondersteund.'
          : (text || `Uploaden mislukt (foutcode ${res.status}).`);
        // Surface via the returned result only (the consumer shows its own
        // upload dialog); don't also set saveError, which raises a 2nd modal.
        return { ok: false, error, retryable: !unsupported };
      }
      const json = await safeJson(res);
      // Refresh the list so the poller (and, once converted, the .md) show up.
      await fetchList();
      return { ok: true, targetPath: json?.target_path ?? null };
    } catch (e) {
      return { ok: false, error: e.message };
    }
  }

  async function deleteDocument(path) {
    requireTraject(trajectRef.value, 'document delete');
    // Same reasoning as in `saveCurrent`: if the doc being deleted is
    // the open one, the pending debounce would resurrect a draft after
    // we clear it below.
    if (path === currentPath.value) cancelDraftTimer();
    saveError.value = null;
    // Send `If-Match` so the delete is conditional: a concurrent edit then
    // surfaces as a 412 instead of being silently destroyed. The open
    // document already has its ETag in `currentEtag`; for any other entry
    // (the list does not cache per-entry ETags) fetch the current ETag
    // first. A non-OK GET (e.g. 404 - already gone) leaves `ifMatch` null
    // and the DELETE falls through to report the real outcome.
    let ifMatch =
      path === currentPath.value && currentEtag.value ? currentEtag.value : null;
    if (!ifMatch) {
      try {
        // HEAD, not GET: Axum serves HEAD from the GET handler with the
        // body suppressed, so we get the current `ETag` header without
        // downloading the document body just to read it.
        const head = await fetch(documentFileUrl(trajectRef.value, path), {
          method: 'HEAD',
        });
        if (head.ok) ifMatch = head.headers.get('ETag');
      } catch {
        /* network error - proceed unconditionally; DELETE surfaces its own error */
      }
    }
    const headers = {};
    if (ifMatch) headers['If-Match'] = ifMatch;
    try {
      // Raw fetch on purpose: same result-object style as `saveCurrent` -
      // 412 and 404 are normal branches here, not errors.
      const res = await fetch(documentFileUrl(trajectRef.value, path), {
        method: 'DELETE',
        headers,
      });
      if (res.status === 412) {
        // A concurrent modification means our delete precondition failed.
        // Do NOT reuse the save-conflict `conflict` ref: its banner offers
        // "reload"/"overwrite" actions that operate on the *open* document
        // (and "overwrite" is a PUT - the wrong resolution for a delete).
        // Refresh the list so the caller re-evaluates against current
        // state, and return a typed result for the panel to surface.
        await fetchList();
        return { ok: false, conflict: true };
      }
      if (res.status === 404) {
        // The document is already gone (deleted remotely between the list
        // load and this click - e.g. the unconditional path where the HEAD
        // probe 404'd). Delete is idempotent, so treat it as success: drop
        // the local draft, clear it if it was the open one, and refresh the
        // list so the stale sidebar entry disappears instead of lingering
        // until a reload.
        clearDraft(trajectRef.value, path);
        if (path === currentPath.value) {
          currentPath.value = null;
          currentBody.value = '';
          currentEtag.value = null;
        }
        await fetchList();
        return { ok: true };
      }
      if (!res.ok) {
        const text = await safeText(res);
        saveError.value = new Error(text || `Verwijderen mislukt: ${res.status}`);
        return { ok: false };
      }
      clearDraft(trajectRef.value, path);
      if (path === currentPath.value) {
        currentPath.value = null;
        currentBody.value = '';
        currentEtag.value = null;
      }
      await fetchList();
      return { ok: true };
    } catch (e) {
      // Network error (Failed to fetch / timeout) - surface it through
      // the same `saveError` banner the editor already shows, so a failed
      // delete isn't swallowed silently. Mirrors `saveCurrent`'s catch.
      saveError.value = e;
      return { ok: false };
    }
  }

  // Re-fetch the list whenever the active traject changes - switching
  // trajects routes through a different writable backend.
  //
  // Critically, also reset the per-document state: a document loaded from
  // the previous traject must NOT stay open across a switch. Its path/body/
  // etag belong to the old traject, so a save would write the old content
  // to the NEW traject's URL (and the stale etag would trip a misleading
  // 412 "overschrijven"-prompt). Clearing here makes the switch safe;
  // TrajectDocuments additionally returns the sheet to its list on the same
  // change.
  watch(
    trajectRef,
    () => {
      cancelDraftTimer();
      currentPath.value = null;
      currentBody.value = '';
      currentEtag.value = null;
      docError.value = null;
      conflict.value = null;
      deletedRemotely.value = null;
      fetchList().catch(() => {});
    },
    { immediate: true },
  );

  return {
    documents,
    loading,
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
    fetchList,
    openDocument,
    saveCurrent,
    reloadCurrent,
    createDocument,
    uploadDocument,
    deleteDocument,
    dropDraft,
  };
}

async function safeText(res) {
  // Read the body unconditionally: Axum's `(StatusCode, String)` errors are
  // text/plain, but a 413 (DefaultBodyLimit) or a proxy-wrapped error may
  // arrive without that exact content-type. Returning the body regardless
  // lets the caller surface the real message instead of a generic status.
  try {
    return await res.text();
  } catch {
    return '';
  }
}

async function safeJson(res) {
  try {
    return await res.json();
  } catch {
    return null;
  }
}

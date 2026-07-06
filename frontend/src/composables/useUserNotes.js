/**
 * useUserNotes — persoonlijke notities on a law, private per user.
 *
 * Server-side store (Postgres via editor-api, /api/user/notes/{law_id});
 * nothing is written to git or localStorage. Notes come back in W3C Web
 * Annotation shape (RFC-005): `body` is a TextualBody whose `format` is
 * `text/markdown`, so consumers render `body.value` through the shared
 * sanitizing markdown pipeline (renderArticleHtml).
 *
 * Availability: personal notes require an authenticated session and a
 * database. A 401 (anonymous) or 503 (auth/DB disabled, e.g. plain local
 * dev) flips `available` to false so the UI hides the feature instead of
 * showing a broken form — same graceful degradation as useUserSettings.
 */
import { ref, watch } from 'vue';
import { apiFetch, apiFetchJson, ApiError } from '../lib/apiFetch.js';

const UNAVAILABLE_STATUSES = [401, 403, 503];

function notesUrl(lawId, noteId) {
  const base = `/api/user/notes/${encodeURIComponent(lawId)}`;
  return noteId ? `${base}/${encodeURIComponent(noteId)}` : base;
}

export function useUserNotes(lawId) {
  const notes = ref([]);
  const loading = ref(false);
  const error = ref(null);
  // Starts false so the UI renders nothing until the first fetch settles —
  // an anonymous user never sees the section flash in and disappear. A
  // non-availability failure (e.g. 500) still flips it on so the error
  // surfaces.
  const available = ref(false);

  async function reload() {
    const id = lawId.value;
    if (!id) {
      notes.value = [];
      return;
    }
    loading.value = true;
    error.value = null;
    try {
      const data = await apiFetchJson(notesUrl(id), {
        errorMessage: (status) => `HTTP ${status}`,
      });
      // A slow response for a previous law must not clobber the current one.
      if (lawId.value !== id) return;
      notes.value = data;
      available.value = true;
    } catch (e) {
      if (lawId.value !== id) return;
      notes.value = [];
      if (e instanceof ApiError && UNAVAILABLE_STATUSES.includes(e.status)) {
        // Anonymous session or no DB: feature is off, not broken.
        available.value = false;
      } else {
        available.value = true;
        error.value = e;
      }
    } finally {
      if (lawId.value === id) loading.value = false;
    }
  }

  // Mutations capture the law at call time and only touch the local list
  // when the user is still on that law — a slow response resolving after
  // a law switch must not leak a note into the other law's list.

  /** Create a markdown note; resolves with the created annotation. */
  async function addNote(value) {
    const id = lawId.value;
    const created = await apiFetchJson(notesUrl(id), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value }),
      errorMessage: (status) => `HTTP ${status}`,
    });
    if (lawId.value === id) notes.value = [...notes.value, created];
    return created;
  }

  /** Update a note's markdown body; resolves with the updated annotation. */
  async function updateNote(noteId, value) {
    const id = lawId.value;
    const updated = await apiFetchJson(notesUrl(id, noteId), {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value }),
      errorMessage: (status) => `HTTP ${status}`,
    });
    if (lawId.value === id) {
      notes.value = notes.value.map((n) => (n.id === noteId ? updated : n));
    }
    return updated;
  }

  /** Delete a note. */
  async function removeNote(noteId) {
    const id = lawId.value;
    await apiFetch(notesUrl(id, noteId), {
      method: 'DELETE',
      errorMessage: (status) => `HTTP ${status}`,
    });
    if (lawId.value === id) {
      notes.value = notes.value.filter((n) => n.id !== noteId);
    }
  }

  watch(lawId, reload, { immediate: true });

  return { notes, loading, error, available, reload, addNote, updateNote, removeNote };
}

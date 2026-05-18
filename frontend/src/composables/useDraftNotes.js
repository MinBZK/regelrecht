/**
 * useDraftNotes — local-only note authoring store (RFC-018 write path, MVP).
 *
 * The MVP does not write the sidecar back to the repo: notes a user creates in
 * the editor live in `localStorage`, keyed per law, and are exported as a YAML
 * document the user commits to `corpus/annotations/{lawId}/annotations.yaml`
 * by hand (RFC-018 step 6 — git stays the source of truth, no write API).
 *
 * Draft notes are merged into the resolved-notes list by the caller so they
 * highlight live, exactly like committed notes; they just carry an extra
 * `__draft` marker so the UI can show them as unsaved and offer delete/export.
 */
import { ref, computed, watch } from 'vue';
import yaml from 'js-yaml';

const STORAGE_PREFIX = 'regelrecht-draft-notes:';

function storageKey(lawId) {
  return `${STORAGE_PREFIX}${lawId}`;
}

function loadFor(lawId) {
  if (!lawId) return [];
  try {
    const raw = localStorage.getItem(storageKey(lawId));
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function persist(lawId, list) {
  try {
    localStorage.setItem(storageKey(lawId), JSON.stringify(list));
  } catch {
    /* quota/full or disabled — drafts are best-effort in the MVP */
  }
}

/**
 * @param {import('vue').Ref<string>} lawId reactive law $id
 */
export function useDraftNotes(lawId) {
  const drafts = ref(loadFor(lawId.value));

  // Re-read from storage whenever the law changes. A real watch (not a lazy
  // resync inside every method/computed) keeps `drafts` authoritative for the
  // current law at all times, so useResolvedDraftNotes — which watches lawId
  // independently — never resolves the previous law's selectors against the
  // new law between a law switch and the next store method call.
  watch(lawId, (id) => {
    drafts.value = loadFor(id);
  });

  /**
   * Append a note. `note` is a full W3C Annotation object (already validated
   * shape from NoteCreator). Returns the stored note (with its draft marker).
   */
  function addDraft(note) {
    const stored = {
      ...note,
      created: note.created || new Date().toISOString(),
    };
    drafts.value = [...drafts.value, stored];
    persist(lawId.value, drafts.value);
    return stored;
  }

  /** Remove the draft at `index` (index into the current drafts array). */
  function removeDraft(index) {
    if (index < 0 || index >= drafts.value.length) return;
    drafts.value = drafts.value.filter((_, i) => i !== index);
    persist(lawId.value, drafts.value);
  }

  /** Drop every draft for the current law (after a successful export/commit). */
  function clearDrafts() {
    drafts.value = [];
    persist(lawId.value, []);
  }

  /**
   * The full sidecar YAML for this law: every committed note plus the drafts.
   *
   * The committed notes are read from the *raw sidecar file*, not from the
   * resolver output. The WASM resolver drops notes whose target.source names a
   * different law (federated sidecars may carry notes for several laws — see
   * resolveNotes in wasm.rs); rebuilding the export from resolved notes would
   * silently truncate those on commit. Re-parsing the original file preserves
   * every committed note verbatim and only appends the drafts.
   *
   * Async because it fetches the sidecar; a missing sidecar (404) is normal
   * for a law that has no committed notes yet — the export is then just the
   * drafts.
   */
  async function exportYaml() {
    const id = lawId.value;
    let committed = [];
    try {
      const res = await fetch(
        `/data/annotations/${encodeURIComponent(id)}/annotations.yaml`,
      );
      if (res.ok) {
        const doc = yaml.load(await res.text());
        if (Array.isArray(doc?.annotations)) committed = doc.annotations;
      }
      // 404 (no sidecar yet) and parse failures fall through to drafts-only;
      // the author still gets a valid file to commit.
    } catch {
      /* network/parse error — export the drafts so work is not lost */
    }
    const annotations = [
      ...committed.map(stripDraftMarker),
      ...drafts.value.map(stripDraftMarker),
    ];
    const doc = {
      $schema:
        'https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.2/annotation-schema.json',
      annotations,
    };
    // lineWidth -1: never fold long body strings, so the YAML diff stays
    // readable and selectors are not silently wrapped.
    return yaml.dump(doc, { lineWidth: -1, noRefs: true });
  }

  const draftCount = computed(() => drafts.value.length);

  return {
    drafts,
    draftCount,
    addDraft,
    removeDraft,
    clearDrafts,
    exportYaml,
  };
}

function stripDraftMarker(note) {
  if (note && typeof note === 'object' && '__draft' in note) {
    const { __draft, ...rest } = note;
    return rest;
  }
  return note;
}

/**
 * useNotes — fetch a law's note sidecar and resolve it against the loaded law.
 *
 * Notes are W3C Web Annotations anchored to legal text via a TextQuoteSelector
 * (RFC-005). The Rust resolver runs in WASM (`engine.resolveNotes`) so the
 * editor shows exactly what the engine and CI validate. Match offsets are
 * `char` offsets into the article text, not UTF-16 code units (see the WASM
 * binding docs); `markRanges` converts them when slicing.
 */
import { ref, computed, watch } from 'vue';
import { useEngine } from './useEngine.js';
import { annotationsUrl } from './corpusUrls.js';

// Cache resolved notes per `${trajectRef}::${lawId}` for the session.
// The resolver result only changes when the law text or the sidecar
// changes; scoping by trajectRef prevents cross-traject leakage when
// the user switches between trajects.
//
// Caveat (acceptable for the display-only, default-off MVP; revisit in
// the note-editing phase): the lawId key part is `law.$id`, which does
// not encode the law *version*. A save through the editor changes the
// text without changing `$id`, so the cache is not invalidated on save
// — editing a law in-session and reopening its Notities pane could show
// offsets resolved against the pre-save text. Once notes become
// editable, key by `$id` + version and invalidate on save.
const cache = new Map();

function cacheKey(trajectRef, lawId) {
  return `${trajectRef || ''}::${lawId}`;
}

/**
 * @param {import('vue').Ref<string>} lawId reactive law $id
 * @param {import('vue').Ref<object>} selectedArticle reactive current article
 * @param {import('vue').Ref<string|null>} trajectRef reactive traject ref
 *   (`null` for global / no-traject reads)
 */
export function useNotes(lawId, selectedArticle, trajectRef) {
  const { initEngine, loadDependency } = useEngine();
  const resolved = ref([]); // [{ note, match, error }]
  const loading = ref(false);
  const error = ref(null);

  // Generation guard: each load() call claims a generation; only the latest
  // is allowed to write reactive state. Without this, navigating between laws
  // while a slow annotations fetch is in flight lets the older response
  // overwrite the newer law's notes — and because article numbers collide
  // across laws ('1','2','3' everywhere) the stale offsets would silently
  // highlight wrong spans. useLaw guards the same race the same way.
  let generation = 0;

  async function load() {
    const id = lawId.value;
    const tr = trajectRef?.value ?? null;
    const gen = ++generation;
    const isStale = () => gen !== generation;

    // These early returns resolve synchronously. They must clear `loading`
    // too: if a slow uncached load is in flight and the user navigates to a
    // cached law, that older load is now stale and skips its own
    // `loading = false` reset (gated on !isStale), so without clearing it
    // here the "Notities laden…" spinner stays stuck forever.
    if (!id) {
      resolved.value = [];
      error.value = null;
      loading.value = false;
      return;
    }
    const key = cacheKey(tr, id);
    if (cache.has(key)) {
      // Reset error too: a cached law (e.g. a 404 → []) must not keep showing
      // the previous law's "kon notities niet laden" alert.
      resolved.value = cache.get(key);
      error.value = null;
      loading.value = false;
      return;
    }

    loading.value = true;
    error.value = null;
    try {
      // With an active traject the read goes through that traject's
      // backend (where `save_annotations` writes) so a freshly-appended
      // note is visible immediately. Without a traject this falls back
      // to the global annotation route — the central source's main view.
      const res = await fetch(annotationsUrl(tr, id));
      if (res.status === 404) {
        // A law without a sidecar is normal, not an error.
        cache.set(key, []);
        if (!isStale()) resolved.value = [];
        return;
      }
      if (!res.ok) {
        throw new Error(`Kon notities niet laden: ${res.status}`);
      }
      const yamlText = await res.text();

      const engine = await initEngine();
      // The resolver needs the law's articles loaded; mirror how the rest of
      // the editor pulls a law into the engine. Pass the traject so the
      // dependency fetch sees the same scope (read-your-writes parity).
      if (!engine.hasLaw(id)) {
        await loadDependency(id, tr);
      }
      const result = engine.resolveNotes(id, yamlText);
      const list = Array.isArray(result) ? result : [];
      cache.set(key, list);
      if (!isStale()) resolved.value = list;
    } catch (e) {
      if (!isStale()) {
        error.value = e;
        resolved.value = [];
      }
    } finally {
      // Only the latest load owns the loading flag.
      if (!isStale()) loading.value = false;
    }
  }

  // Re-load on either the law or the active-traject changing — the
  // sidecar lives per traject branch, so a switch needs a fresh fetch
  // even if the law id stayed put.
  const trackers = trajectRef ? [lawId, trajectRef] : [lawId];
  watch(trackers, load, { immediate: true });

  /**
   * Force a fresh fetch for the current law: drop its cache entry first
   * so `load()` can't shortcut to the previously-resolved value, then
   * run `load`. Used after `saveToRepo` so a just-committed note shows
   * up immediately instead of waiting for a navigation away and back.
   *
   * `load()` alone won't do — it returns the cached `[]` from the
   * first pre-save fetch and silently leaves the user looking at an
   * empty notes pane right after they hit "Opslaan".
   */
  async function reload() {
    const id = lawId.value;
    if (id) cache.delete(cacheKey(trajectRef?.value ?? null, id));
    await load();
  }

  /**
   * Notes whose match falls in the currently-selected article, each with the
   * resolved span(s) for that article. Notes that are orphaned, ambiguous, or
   * failed to parse are surfaced separately via `issues` so the UI can show
   * them without anchoring them in the text.
   */
  const notesForArticle = computed(() => {
    const articleNr = selectedArticle.value?.number;
    if (articleNr == null || articleNr === '') return [];
    // String() both sides: js-yaml decodes an unquoted `number: 2` to a JS
    // number while the resolver's article_number is always a string. useLaw
    // applies the same defensive coercion for the same reason.
    const target = String(articleNr);
    const out = [];
    for (const entry of resolved.value) {
      if (entry.error || !entry.match) continue;
      if (entry.match.status !== 'found') continue;
      const spans = entry.match.matches.filter(
        (m) => String(m.article_number) === target,
      );
      if (spans.length > 0) out.push({ note: entry.note, spans });
    }
    return out;
  });

  /** Orphaned / ambiguous / parse-failed notes, for a status list. */
  const issues = computed(() =>
    resolved.value
      .filter(
        (e) => e.error || (e.match && e.match.status !== 'found'),
      )
      .map((e) => ({
        note: e.note,
        reason: e.error
          ? `parsefout: ${e.error}`
          : e.match.status === 'orphaned'
            ? 'niet gevonden in de wettekst (orphaned)'
            : 'meerdere matches (ambigu) — voeg context toe',
      })),
  );

  return { notesForArticle, issues, loading, error, reload };
}

/**
 * Resolve a list of in-memory draft notes against the loaded law and project
 * them onto the selected article, returning the same `{ note, spans }` shape
 * as `notesForArticle` so the editor highlights drafts exactly like committed
 * notes. Drafts live only in localStorage until exported (RFC-018 write path);
 * they are resolved here per-note via the same WASM resolver, not refetched.
 *
 * @param {import('vue').Ref<Array>} draftNotes reactive list of W3C Annotation
 * @param {import('vue').Ref<string>} lawId
 * @param {import('vue').Ref<object>} selectedArticle
 * @param {import('vue').Ref<string|null>=} trajectRef Active traject ref.
 *   Routes the dependency load through the matching scope so a draft
 *   resolves against the same law copy the editor shows.
 */
export function useResolvedDraftNotes(draftNotes, lawId, selectedArticle, trajectRef) {
  const { initEngine, loadDependency } = useEngine();
  const resolvedDrafts = ref([]); // [{ note, match }]

  // Generation guard: resolve() awaits initEngine/loadDependency (slow on a
  // law switch). Without this, a resolve started before a law switch can
  // finish after the one started by the switch and overwrite it with stale
  // data — and because draft selectors resolve per-law, that would highlight
  // the previous law's drafts on the new law. useNotes.load() guards the same
  // race the same way.
  let generation = 0;

  async function resolve() {
    const id = lawId.value;
    const notes = draftNotes.value;
    const tr = trajectRef?.value ?? null;
    const gen = ++generation;
    const isStale = () => gen !== generation;
    if (!id || !notes || notes.length === 0) {
      resolvedDrafts.value = [];
      return;
    }
    try {
      const engine = await initEngine();
      // Pass the scope so the engine cache can detect a stale copy
      // from a previous traject and refetch — without this a switch
      // would keep highlighting drafts against the old law content.
      await loadDependency(id, tr);
      const out = [];
      for (const note of notes) {
        const selector = note?.target?.selector;
        if (!selector) continue;
        let match;
        try {
          match = engine.resolveNote(id, selector);
        } catch {
          continue; // a malformed draft selector simply does not highlight
        }
        out.push({ note, match });
      }
      if (!isStale()) resolvedDrafts.value = out;
    } catch {
      if (!isStale()) resolvedDrafts.value = [];
    }
  }

  const trackers = trajectRef ? [draftNotes, lawId, trajectRef] : [draftNotes, lawId];
  watch(trackers, resolve, { immediate: true, deep: true });

  const draftNotesForArticle = computed(() => {
    const articleNr = selectedArticle.value?.number;
    if (articleNr == null || articleNr === '') return [];
    const target = String(articleNr);
    const out = [];
    for (const entry of resolvedDrafts.value) {
      if (!entry.match || entry.match.status !== 'found') continue;
      const spans = entry.match.matches.filter(
        (m) => String(m.article_number) === target,
      );
      if (spans.length > 0) out.push({ note: entry.note, spans });
    }
    return out;
  });

  return { draftNotesForArticle };
}

/**
 * Slice `text` into segments around resolved note spans, for rendering.
 *
 * Returns an ordered array of `{ text, note }` segments where `note` is null
 * for plain text and the annotating note for a highlighted span. The result is
 * a gap-free partition of `text` (every character emitted exactly once).
 *
 * Overlap handling: marks are sorted by start (longest first on ties); a mark
 * that starts inside an already-emitted mark is dropped. The resolver
 * de-duplicates a single selector's matches, but two *different* notes
 * annotating overlapping spans is a legitimate RFC-018 case — the later one
 * is silently not rendered here (it is not surfaced in `issues` either, since
 * it resolved fine). Acceptable for display-only; the note-editing phase will
 * need layered rendering instead of a flat partition.
 *
 * @param {string} text          article text (the same string the resolver saw)
 * @param {Array<{note:object,spans:Array}>} notesForArticle
 */
export function markRanges(text, notesForArticle) {
  const chars = Array.from(text); // char (code-point) array: offsets are char-based
  const marks = [];
  for (const { note, spans } of notesForArticle) {
    for (const s of spans) {
      marks.push({ start: s.start, end: s.end, note });
    }
  }
  marks.sort((a, b) => a.start - b.start || b.end - a.end);

  const segments = [];
  let cursor = 0;
  for (const m of marks) {
    // A zero-length span (start === end) would emit an empty, styled <mark>.
    // The Rust resolver never produces this for a TextQuoteSelector, but a
    // malformed hand-authored sidecar could; drop it so the contract is
    // explicit and the partition stays clean.
    if (m.end <= m.start) continue;
    if (m.start < cursor) continue; // skip overlap with an already-emitted mark
    if (m.start > cursor) {
      segments.push({ text: chars.slice(cursor, m.start).join(''), note: null });
    }
    segments.push({
      text: chars.slice(m.start, m.end).join(''),
      note: m.note,
    });
    cursor = m.end;
  }
  if (cursor < chars.length) {
    segments.push({ text: chars.slice(cursor).join(''), note: null });
  }
  return segments;
}

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

// Cache resolved notes per lawId for the session. The resolver result only
// changes when the law text or the sidecar changes, neither of which happens
// within a session without a reload.
const cache = new Map();

/**
 * @param {import('vue').Ref<string>} lawId        reactive law $id
 * @param {import('vue').Ref<object>} selectedArticle reactive current article
 */
export function useNotes(lawId, selectedArticle) {
  const { initEngine, loadDependency } = useEngine();
  const resolved = ref([]); // [{ note, match, error }]
  const loading = ref(false);
  const error = ref(null);

  async function load() {
    const id = lawId.value;
    if (!id) {
      resolved.value = [];
      return;
    }
    if (cache.has(id)) {
      resolved.value = cache.get(id);
      return;
    }

    loading.value = true;
    error.value = null;
    try {
      const res = await fetch(
        `/data/annotations/${encodeURIComponent(id)}/annotations.yaml`,
      );
      if (res.status === 404) {
        // A law without a sidecar is normal, not an error.
        resolved.value = [];
        cache.set(id, []);
        return;
      }
      if (!res.ok) {
        throw new Error(`Kon notities niet laden: ${res.status}`);
      }
      const yamlText = await res.text();

      const engine = await initEngine();
      // The resolver needs the law's articles loaded; mirror how the rest of
      // the editor pulls a law into the engine.
      if (!engine.hasLaw(id)) {
        await loadDependency(id);
      }
      const result = engine.resolveNotes(id, yamlText);
      resolved.value = Array.isArray(result) ? result : [];
      cache.set(id, resolved.value);
    } catch (e) {
      error.value = e;
      resolved.value = [];
    } finally {
      loading.value = false;
    }
  }

  watch(lawId, load, { immediate: true });

  /**
   * Notes whose match falls in the currently-selected article, each with the
   * resolved span(s) for that article. Notes that are orphaned, ambiguous, or
   * failed to parse are surfaced separately via `issues` so the UI can show
   * them without anchoring them in the text.
   */
  const notesForArticle = computed(() => {
    const articleNr = selectedArticle.value?.number;
    if (!articleNr) return [];
    const out = [];
    for (const entry of resolved.value) {
      if (entry.error || !entry.match) continue;
      if (entry.match.status !== 'found') continue;
      const spans = entry.match.matches.filter(
        (m) => m.article_number === articleNr,
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

  return { resolved, notesForArticle, issues, loading, error, reload: load };
}

/**
 * Slice `text` into segments around resolved note spans, for rendering.
 *
 * Returns an ordered array of `{ text, note }` segments where `note` is null
 * for plain text and the annotating note for a highlighted span. Overlapping
 * spans are resolved by taking the earliest-starting, longest span (the
 * resolver already de-duplicates, so overlaps across notes are rare).
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

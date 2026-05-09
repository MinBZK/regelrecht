import { ref, computed } from 'vue';

/**
 * Shared corpus-laws list. The `/api/corpus/laws` payload is small (~kb) and
 * stable per page-load, so we fetch once and reuse across components that
 * need to resolve a law_id to its human name (Bron-regelgeving combo-box,
 * MachineReadable input rows, etc.).
 *
 * Module-level state — every consumer sees the same list/promise.
 */

const laws = ref([]);
let fetchPromise = null;

// Backend hard-caps `?limit` at 1000 (see editor-api/corpus_handlers.rs
// MAX_LIMIT). With the current curated Test Corpus this is far above
// what's loaded, but if the corpus ever grows past 1000, MachineReadable
// input rows that reference an overflow law would silently fall back to
// the title-cased snake_case identifier. Warn loudly when we hit the cap
// so the gap is visible rather than silently broken.
const FETCH_LIMIT = 1000;

function ensureFetched() {
  if (fetchPromise) return fetchPromise;
  fetchPromise = fetch(`/api/corpus/laws?limit=${FETCH_LIMIT}`)
    .then(r => {
      // Throw on non-ok so the catch below resets fetchPromise — otherwise
      // a transient 500/404 at first call would lock us into the empty-list
      // fallback for the rest of the session, since later useCorpusLaws()
      // calls would see the (already-resolved) promise and skip refetch.
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      return r.json();
    })
    .then(list => {
      laws.value = Array.isArray(list) ? list : [];
      if (laws.value.length >= FETCH_LIMIT) {
        console.warn(
          `useCorpusLaws: hit the ${FETCH_LIMIT}-law cap — laws beyond this won't resolve to display names. ` +
          `Pagination needs to be added if the corpus has grown past ${FETCH_LIMIT} entries.`,
        );
      }
      return laws.value;
    })
    .catch(() => {
      laws.value = [];
      // Reset so the next consumer mount triggers a fresh fetch.
      fetchPromise = null;
      return [];
    });
  return fetchPromise;
}

/**
 * Title-cased fallback for a law_id when the corpus payload hasn't returned
 * yet (or doesn't carry the law). Mirrors EditSheet/LibraryApp's displayName.
 */
function fallbackName(lawId) {
  if (!lawId) return '';
  return lawId.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

export function useCorpusLaws() {
  ensureFetched();

  const lawsById = computed(() => {
    const map = new Map();
    for (const law of laws.value) map.set(law.law_id, law);
    return map;
  });

  function displayName(lawId) {
    const law = lawsById.value.get(lawId);
    if (law?.display_name) return law.display_name;
    if (law?.name) return law.name;
    return fallbackName(lawId);
  }

  return { laws, displayName };
}

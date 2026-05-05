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

function ensureFetched() {
  if (fetchPromise) return fetchPromise;
  fetchPromise = fetch('/api/corpus/laws?limit=1000')
    .then(r => (r.ok ? r.json() : []))
    .then(list => {
      laws.value = Array.isArray(list) ? list : [];
      return laws.value;
    })
    .catch(() => {
      laws.value = [];
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

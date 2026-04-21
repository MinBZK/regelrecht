/**
 * useBwbSearch — search for laws on external sources (wetten.overheid.nl).
 *
 * Provides debounced search and result management. Results come from the
 * pipeline-api (proxied through editor-api at /api/harvest/search).
 */
import { ref } from 'vue';

const DEBOUNCE_MS = 400;
export const MIN_QUERY_LENGTH = 3;

export function useBwbSearch() {
  const results = ref([]);
  const loading = ref(false);

  let debounceTimer = null;

  /**
   * Search for laws matching the query. Debounced — safe to call on every keystroke.
   * @param {string} query - The search query (minimum 3 characters)
   */
  function search(query) {
    clearTimeout(debounceTimer);
    const q = (query || '').trim();

    if (q.length < MIN_QUERY_LENGTH) {
      results.value = [];
      return;
    }

    debounceTimer = setTimeout(async () => {
      loading.value = true;
      try {
        const res = await fetch(`/api/harvest/search?q=${encodeURIComponent(q)}`);
        if (res.ok) {
          results.value = await res.json();
        } else {
          results.value = [];
        }
      } catch {
        // BWB search is best-effort — clear stale results on failure
        results.value = [];
      } finally {
        loading.value = false;
      }
    }, DEBOUNCE_MS);
  }

  /** Clear all search results and cancel pending searches. */
  function clear() {
    clearTimeout(debounceTimer);
    results.value = [];
    loading.value = false;
  }

  return { results, loading, search, clear };
}

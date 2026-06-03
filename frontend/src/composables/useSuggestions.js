/**
 * useSuggestions — fetch a law's AI-suggestion sidecar and resolve it against
 * the loaded law, plus poll the pipeline for in-flight suggestion jobs.
 *
 * Suggestions are just W3C Web Annotations (creator = an Agent), written by the
 * pipeline to `suggestions.yaml` on the traject branch. They resolve through the
 * same WASM resolver as notes (`engine.resolveNotes`) and render with the same
 * `markRanges` helper — the editor highlights them with the `.note-generated`
 * dotted style `AnnotatedText` already applies to `creator: llm/tool/generated`.
 *
 * Differs from useNotes in two ways:
 *  - read-only of a different sidecar (`suggestionsUrl`), and
 *  - a `pollStatus()` that watches `/suggestions/status` until both suggestion
 *    kinds reach a terminal state, then reloads the sidecar once.
 */
import { ref, computed, watch } from 'vue';
import { useEngine } from './useEngine.js';
import { suggestionsUrl, suggestionsStatusUrl } from './corpusUrls.js';

// Resolved (accepted/rejected) suggestions are remembered locally so they drop
// out of the pane. There is no server write path for the suggestions sidecar
// (it is pipeline-owned), and the sidecar is regenerated each save anyway, so a
// per-(traject,law) localStorage set of resolved keys is the right granularity:
// it survives refresh, and a fresh save that re-raises the same finding will
// re-key identically and stay hidden (which is the desired "I already handled
// this" behaviour) until the user clears it.
const RESOLVED_KEY = 'regelrecht-resolved-suggestions';

function loadResolved() {
  try {
    return new Set(JSON.parse(localStorage.getItem(RESOLVED_KEY) || '[]'));
  } catch {
    return new Set();
  }
}

function saveResolved(set) {
  try {
    localStorage.setItem(RESOLVED_KEY, JSON.stringify([...set]));
  } catch {
    /* ignore quota / private-mode failures */
  }
}

/**
 * Stable identity for a suggestion: traject + law + anchor + body text. Used to
 * remember which suggestions the user already accepted/rejected.
 */
export function suggestionKey(trajectRef, lawId, note) {
  const exact = note?.target?.selector?.exact ?? '';
  const body = note?.body;
  const value = (Array.isArray(body) ? body[0] : body)?.value ?? '';
  return `${trajectRef || ''}::${lawId}::${exact}::${value}`;
}

// Cache resolved suggestions per `${trajectRef}::${lawId}`, like useNotes. The
// same save-doesn't-bump-$id caveat applies; `reload()` drops the entry so a
// freshly-generated sidecar shows up without a navigation round-trip.
const cache = new Map();

function cacheKey(trajectRef, lawId) {
  return `${trajectRef || ''}::${lawId}`;
}

const TERMINAL = new Set(['completed', 'failed']);

/**
 * @param {import('vue').Ref<string>} lawId reactive law $id
 * @param {import('vue').Ref<object>} selectedArticle reactive current article
 * @param {import('vue').Ref<string|null>} trajectRef reactive traject ref
 */
export function useSuggestions(lawId, selectedArticle, trajectRef) {
  const { initEngine, loadDependency } = useEngine();
  const resolved = ref([]); // [{ note, match, error }]
  const loading = ref(false);
  const error = ref(null);
  // 'idle' | 'running' | 'done' — whether suggestion jobs are in flight.
  const jobState = ref('idle');
  // Reactive set of locally-resolved suggestion keys; accepted/rejected items
  // are filtered out of the pane. Reactivity via a bumped counter (a plain Set
  // is not deeply reactive).
  const resolvedKeys = ref(loadResolved());
  const resolvedTick = ref(0);

  /** Mark a suggestion resolved (accepted or rejected) and persist it. */
  function markResolved(note) {
    const key = suggestionKey(trajectRef?.value ?? null, lawId.value, note);
    resolvedKeys.value.add(key);
    saveResolved(resolvedKeys.value);
    resolvedTick.value++;
  }

  // Generation guard: only the latest load()/poll() may write reactive state,
  // mirroring useNotes — article numbers collide across laws, so a stale
  // response could otherwise highlight the wrong spans.
  let generation = 0;

  async function load() {
    const id = lawId.value;
    const tr = trajectRef?.value ?? null;
    const gen = ++generation;
    const isStale = () => gen !== generation;

    if (!id) {
      resolved.value = [];
      error.value = null;
      loading.value = false;
      return;
    }
    const key = cacheKey(tr, id);
    if (cache.has(key)) {
      resolved.value = cache.get(key);
      error.value = null;
      loading.value = false;
      return;
    }

    loading.value = true;
    error.value = null;
    try {
      const res = await fetch(suggestionsUrl(tr, id));
      if (res.status === 404) {
        // No sidecar yet is normal (no suggestions generated), not an error.
        cache.set(key, []);
        if (!isStale()) resolved.value = [];
        return;
      }
      if (!res.ok) {
        throw new Error(`Kon suggesties niet laden: ${res.status}`);
      }
      const yamlText = await res.text();

      const engine = await initEngine();
      await loadDependency(id, tr);
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
      if (!isStale()) loading.value = false;
    }
  }

  // Re-load when the law or active traject changes.
  const trackers = trajectRef ? [lawId, trajectRef] : [lawId];
  watch(trackers, load, { immediate: true });

  /** Drop the cache entry for the current law, then reload. */
  async function reload() {
    const id = lawId.value;
    if (id) cache.delete(cacheKey(trajectRef?.value ?? null, id));
    await load();
  }

  /**
   * Poll `/suggestions/status` until both suggestion kinds are terminal (or the
   * timeout elapses), then reload the sidecar once. Called by the editor right
   * after a save. Self-cancels if the law/traject changes mid-poll.
   *
   * The pipeline runs `claude -p` (up to ~10 min), so we poll every 4s with a
   * generous cap rather than a tight loop.
   */
  async function pollStatus({ intervalMs = 4000, timeoutMs = 12 * 60 * 1000 } = {}) {
    const id = lawId.value;
    const tr = trajectRef?.value ?? null;
    if (!id || !tr) return;
    const gen = ++generation;
    const isStale = () => gen !== generation;

    jobState.value = 'running';
    const deadline = Date.now() + timeoutMs;

    while (!isStale() && Date.now() < deadline) {
      let entries = [];
      try {
        const res = await fetch(suggestionsStatusUrl(tr, id));
        if (res.ok) {
          const body = await res.json();
          entries = Array.isArray(body?.results) ? body.results : [];
        }
      } catch {
        // Transient; keep polling until the deadline.
      }
      if (isStale()) return;

      // Done when there is at least one job and every reported job is terminal.
      const allTerminal =
        entries.length > 0 && entries.every((e) => TERMINAL.has(e.status));
      if (allTerminal) {
        await reload();
        if (!isStale()) jobState.value = 'done';
        return;
      }
      await new Promise((r) => setTimeout(r, intervalMs));
    }
    if (!isStale()) jobState.value = 'idle';
  }

  /**
   * Suggestions whose match falls in the currently-selected article, each with
   * the resolved span(s). Same shape and filtering as useNotes.notesForArticle
   * so the editor can render them through the same `markRanges` path.
   */
  // True when this note has been accepted/rejected this session. Reading
  // resolvedTick makes the computeds re-run when markResolved fires.
  function isResolved(note) {
    void resolvedTick.value;
    return resolvedKeys.value.has(
      suggestionKey(trajectRef?.value ?? null, lawId.value, note),
    );
  }

  const suggestionsForArticle = computed(() => {
    const articleNr = selectedArticle.value?.number;
    if (articleNr == null || articleNr === '') return [];
    const target = String(articleNr);
    const out = [];
    for (const entry of resolved.value) {
      if (entry.error || !entry.match) continue;
      if (entry.match.status !== 'found') continue;
      if (isResolved(entry.note)) continue;
      const spans = entry.match.matches.filter(
        (m) => String(m.article_number) === target,
      );
      if (spans.length > 0) out.push({ note: entry.note, spans });
    }
    return out;
  });

  /** Orphaned / ambiguous / parse-failed suggestions, for a status list. */
  const issues = computed(() =>
    resolved.value
      .filter((e) => e.error || (e.match && e.match.status !== 'found'))
      .filter((e) => !isResolved(e.note))
      .map((e) => ({
        note: e.note,
        reason: e.error
          ? `parsefout: ${e.error}`
          : e.match.status === 'orphaned'
            ? 'niet gevonden in de wettekst (orphaned)'
            : 'meerdere matches (ambigu)',
      })),
  );

  return {
    suggestionsForArticle,
    issues,
    loading,
    error,
    jobState,
    reload,
    pollStatus,
    markResolved,
  };
}

import { computed, ref, shallowRef } from 'vue';
import yaml from 'js-yaml';
import { getEditorSessionId, lastSavedPr, sanitizeSavedPr } from './useEditorSession.js';
import { lawUrl, requireTraject } from './corpusUrls.js';

// Shared law cache, keyed by `${trajectRef || ''}::${lawId}` so a law
// opened in traject A and in traject B (or globally) returns the
// per-traject view rather than whichever was fetched first. The
// trajectRef is part of the URL, so per-tab navigation never silently
// mixes content across trajects.
//
// LRU-capped so a long session that hops across many trajects doesn't
// grow the cache without bound. 50 entries comfortably covers global +
// a handful of trajects × the laws a session realistically opens;
// evictions are cheap (the next fetch re-populates from the API).
// Insertion order on a Map IS the LRU order — `touchLawCache` bumps a
// key by `delete` + `set`.
const LAW_CACHE_MAX = 50;
const lawCache = new Map();

function lawCacheKey(trajectRef, lawId) {
  return `${trajectRef || ''}::${lawId}`;
}

function touchLawCache(key) {
  if (lawCache.has(key)) {
    const v = lawCache.get(key);
    lawCache.delete(key);
    lawCache.set(key, v);
  }
  while (lawCache.size > LAW_CACHE_MAX) {
    const oldest = lawCache.keys().next().value;
    lawCache.delete(oldest);
  }
}

export function resolveLawName(law) {
  if (!law) return '';
  const nameRef = law.name;
  if (typeof nameRef === 'string' && nameRef.startsWith('#')) {
    const outputName = nameRef.slice(1);
    for (const article of law.articles ?? []) {
      const actions = article.machine_readable?.execution?.actions;
      if (!actions) continue;
      for (const action of actions) {
        if (action.output === outputName) return action.value;
      }
    }
  }
  return nameRef || law.$id || '';
}

// Carry HTTP status on the Error so callers can branch on `.status === 404`.
export function lawFetchError(status) {
  const err = new Error(`Failed to fetch: ${status}`);
  err.status = status;
  return err;
}

/**
 * Fetch a law's YAML, possibly from cache. The traject id is part of
 * the cache key so this never returns content from a different traject
 * than the caller asked for.
 *
 * @param {string|null} trajectRef - active traject (null = global read)
 * @param {string} lawId
 */
export async function fetchLaw(trajectRef, lawId) {
  const key = lawCacheKey(trajectRef, lawId);
  if (lawCache.has(key)) {
    touchLawCache(key);
    return lawCache.get(key);
  }
  const res = await fetch(lawUrl(trajectRef, lawId));
  if (!res.ok) throw lawFetchError(res.status);
  const text = await res.text();
  const law = yaml.load(text);
  const entry = { law, rawYaml: text, lawName: resolveLawName(law) };
  lawCache.set(key, entry);
  touchLawCache(key);
  return entry;
}

export function useLaw(lawParam, articleParam, trajectRefParam) {
  if (!lawParam) {
    const params = new URLSearchParams(window.location.search);
    lawParam = params.get('law') || 'zorgtoeslagwet';
  }
  const initialArticle = articleParam || null;
  // Current traject id for this composable instance. `switchLaw` may
  // update this when navigation crosses trajects, so URL builders read
  // through the closure instead of capturing a snapshot.
  let currentTrajectRef = trajectRefParam || null;
  // If the parameter looks like a URL, fetch directly; otherwise build
  // the API URL from the current trajectRef.
  const initialDirectUrl =
    lawParam.startsWith('/') || lawParam.startsWith('http') ? lawParam : null;
  const law = shallowRef(null);
  const rawYaml = ref('');
  const selectedArticleNumber = ref(null);
  const loading = ref(true);
  const error = ref(null);
  const saving = ref(false);
  const saveError = ref(null);

  const articles = computed(() => law.value?.articles ?? []);

  const lawName = computed(() => resolveLawName(law.value));

  const selectedArticle = computed(() => {
    if (!selectedArticleNumber.value) return null;
    return articles.value.find(
      (a) => String(a.number) === String(selectedArticleNumber.value)
    ) ?? null;
  });

  // Shared version counter for `load()` and `switchLaw()`; stale awaits compare and discard.
  let switchVersion = 0;

  async function load() {
    const version = ++switchVersion;
    try {
      loading.value = true;
      const url = initialDirectUrl ?? lawUrl(currentTrajectRef, lawParam);
      const res = await fetch(url);
      if (!res.ok) throw lawFetchError(res.status);
      if (version !== switchVersion) return;
      const text = await res.text();
      if (version !== switchVersion) return;
      rawYaml.value = text;
      law.value = yaml.load(text);
      const resolvedId = law.value?.$id || lawParam;
      const key = lawCacheKey(currentTrajectRef, resolvedId);
      if (!lawCache.has(key)) {
        lawCache.set(key, { law: law.value, rawYaml: text, lawName: resolveLawName(law.value) });
      }
      touchLawCache(key);
      if (articles.value.length > 0 && !selectedArticleNumber.value) {
        if (initialArticle && articles.value.some(a => String(a.number) === initialArticle)) {
          selectedArticleNumber.value = initialArticle;
        } else {
          selectedArticleNumber.value = String(articles.value[0].number);
        }
      }
    } catch (e) {
      if (version !== switchVersion) return;
      error.value = e;
    } finally {
      if (version === switchVersion) {
        loading.value = false;
      }
    }
  }

  load();

  // Derive the law ID from the parsed law or the original param
  const lawId = computed(() => law.value?.$id || lawParam);

  /**
   * Re-load the open law's content, optionally switching law id /
   * article / traject in one step. Passing `newTrajectRef` is how a
   * cross-traject URL change drives a fresh fetch (and the cache key
   * keeps the previous traject's copy untouched).
   */
  async function switchLaw(newLawId, articleNumber, newTrajectRef) {
    const version = ++switchVersion;
    try {
      loading.value = true;
      error.value = null;
      saveError.value = null;
      saving.value = false;
      if (newTrajectRef !== undefined) {
        currentTrajectRef = newTrajectRef || null;
      }
      const entry = await fetchLaw(currentTrajectRef, newLawId);
      if (version !== switchVersion) return;
      law.value = entry.law;
      rawYaml.value = entry.rawYaml;
      if (articleNumber) {
        selectedArticleNumber.value = String(articleNumber);
      } else if (articles.value.length > 0) {
        selectedArticleNumber.value = String(articles.value[0].number);
      }
    } catch (e) {
      if (version !== switchVersion) return;
      error.value = e;
    } finally {
      if (version === switchVersion) {
        loading.value = false;
      }
    }
  }

  /**
   * Persist edited law YAML to the backend via PUT.
   *
   * On success, updates `rawYaml` + `law` locally so downstream consumers
   * (currentLawYaml computed, engine reload, scenario re-run) converge on
   * the saved text and the editor's dirty-state marker clears.
   *
   * Throws on failure so callers can decide how to surface the error; the
   * `saveError` ref is also populated for passive UI display.
   *
   * @param {string} yamlText - Full law YAML (must contain matching $id)
   */
  async function saveLaw(yamlText) {
    if (!lawId.value) {
      throw new Error('Cannot save law: no lawId');
    }
    if (!currentTrajectRef) {
      throw new Error('Cannot save law: no active traject');
    }
    // Snapshot the law we're saving *before* the await. If the user
    // switches laws while the PUT is in flight, `switchLaw` will replace
    // `lawId` / `rawYaml` / `law` with the new law's state; when the stale
    // response eventually arrives, we must not overwrite the new law's
    // reactive state with the old law's YAML.
    const savedLawId = lawId.value;
    const savedTrajectRef = currentTrajectRef;
    saving.value = true;
    saveError.value = null;
    try {
      requireTraject(savedTrajectRef, 'law save');
      const res = await fetch(lawUrl(savedTrajectRef, savedLawId), {
        method: 'PUT',
        headers: {
          'Content-Type': 'text/yaml; charset=utf-8',
          // Required by editor-api on every write — scopes this save to
          // a per-(session, source) feature branch + PR upstream.
          'X-Editor-Session': getEditorSessionId(),
        },
        body: yamlText,
      });
      if (!res.ok) {
        // Only surface the body when it's our editor-api speaking. The
        // editor-api returns plain `text/plain; charset=utf-8` for its
        // 400/403 bodies (corpus_handlers.rs), so a non-text/plain
        // content-type means a reverse proxy is intercepting (5xx HTML
        // page, etc.) and we should fall back to a generic message
        // rather than render proxy HTML in the save error dialog.
        // res.text() can also throw on a network drop after headers;
        // the same fallback covers that.
        let text = `Save failed: ${res.status}`;
        const contentType = res.headers.get('content-type') || '';
        if (contentType.startsWith('text/plain')) {
          try {
            text = (await res.text()) || text;
          } catch { /* keep status fallback */ }
        }
        throw new Error(text);
      }
      // Backend returns `{ pr: { url, number, branch } | null }` on 200.
      try {
        const json = await res.json();
        lastSavedPr.value = sanitizeSavedPr(json?.pr);
      } catch {
        // Older deployments return a bare 200 without JSON — keep the
        // existing PR (if any) and treat the save as successful.
      }
      const parsed = yaml.load(yamlText);
      // Bail on the success path if the user navigated away mid-flight.
      if (lawId.value === savedLawId && currentTrajectRef === savedTrajectRef) {
        rawYaml.value = yamlText;
        law.value = parsed;
      }
      const resolvedId = parsed?.$id || savedLawId;
      const savedKey = lawCacheKey(savedTrajectRef, resolvedId);
      lawCache.set(savedKey, {
        law: parsed,
        rawYaml: yamlText,
        lawName: resolveLawName(parsed),
      });
      touchLawCache(savedKey);
    } catch (e) {
      if (lawId.value === savedLawId && currentTrajectRef === savedTrajectRef) {
        saveError.value = e;
      }
      throw e;
    } finally {
      if (lawId.value === savedLawId && currentTrajectRef === savedTrajectRef) {
        saving.value = false;
      }
    }
  }

  return {
    law,
    lawId,
    rawYaml,
    articles,
    lawName,
    selectedArticle,
    selectedArticleNumber,
    switchLaw,
    loading,
    error,
    saving,
    saveError,
    saveLaw,
    lastSavedPr,
  };
}

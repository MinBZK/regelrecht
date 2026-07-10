import { computed, ref, shallowRef } from 'vue';
import * as yaml from 'js-yaml';
import { lastSavedPr, sanitizeSavedPr } from './useSavedPr.js';
import { lawUrl } from './corpusUrls.js';
import { apiFetch } from '../lib/apiFetch.js';
import { createLruMap } from '../lib/lruMap.js';
import { useLatest } from '../lib/useLatest.js';
import { humanizeLawId } from '../lib/lawName.js';

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
const lawCache = createLruMap(50);

function lawCacheKey(trajectRef, lawId) {
  return `${trajectRef || ''}::${lawId}`;
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
  // No explicit name → a readable version of the id, never the raw snake_case
  // (e.g. tab labels / titles for laws whose YAML carries no `name`).
  return nameRef || humanizeLawId(law.$id);
}

// Shared apiFetch options for law GETs. The thrown ApiError carries the
// HTTP status so callers can branch on `.status === 404`; the message
// keeps its historical shape.
export const lawFetchInit = Object.freeze({
  errorMessage: (status) => `Failed to fetch: ${status}`,
});

// In-flight law GETs, keyed like `lawCache`. Simultaneous callers share
// one request instead of racing duplicate GETs against an empty cache —
// which is exactly what an editor mount does: `useLaw().load()` fetches
// the routed law while the persisted-tab label loader `fetchLaw`s the
// same id in parallel. Entries are removed when the request settles, so
// a failed fetch never pins a rejected promise (retries hit the network).
const pendingLawFetches = new Map();

/**
 * The network leg shared by `fetchLaw` and `useLaw().load()`: always
 * fetches (no cache read — `load()` relies on that for freshness),
 * single-flighted per cache key, and stores the entry in `lawCache`
 * under the requested id (and the body's resolved `$id`, when the two
 * differ) before resolving.
 */
function fetchLawFresh(trajectRef, lawId) {
  const key = lawCacheKey(trajectRef, lawId);
  const pending = pendingLawFetches.get(key);
  if (pending) return pending;
  const p = (async () => {
    const res = await apiFetch(lawUrl(trajectRef, lawId), lawFetchInit);
    const text = await res.text();
    const law = yaml.load(text);
    const entry = {
      law,
      rawYaml: text,
      lawName: resolveLawName(law),
      // Echoed back as `If-Match` on the next save so a concurrent edit
      // by another traject member surfaces as a 412 instead of a silent
      // overwrite (same chain as useTrajectDocuments).
      etag: res.headers.get('ETag'),
    };
    // Always overwrite: this is fresh content, so any pre-existing entry
    // is by definition stale (older body and/or ETag) — keeping it would
    // hand `fetchLaw`/`switchLaw` callers outdated YAML and a
    // precondition doomed to 412.
    lawCache.set(key, entry);
    const resolvedId = law?.$id;
    if (resolvedId && resolvedId !== lawId) {
      lawCache.set(lawCacheKey(trajectRef, resolvedId), entry);
    }
    return entry;
  })();
  pendingLawFetches.set(key, p);
  p.catch(() => {}).finally(() => pendingLawFetches.delete(key));
  return p;
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
  const cached = lawCache.get(key);
  if (cached) return cached;
  return fetchLawFresh(trajectRef, lawId);
}

export function useLaw(lawParam, articleParam, trajectRefParam) {
  if (!lawParam) {
    const params = new URLSearchParams(window.location.search);
    lawParam = params.get('law') || 'wet_op_de_zorgtoeslag';
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
  // ETag of the last loaded/saved law content; sent as `If-Match` on
  // save so the backend can 412 when someone else edited in between.
  const currentEtag = ref(null);

  const articles = computed(() => law.value?.articles ?? []);

  const lawName = computed(() => resolveLawName(law.value));

  const selectedArticle = computed(() => {
    if (!selectedArticleNumber.value) return null;
    return articles.value.find(
      (a) => String(a.number) === String(selectedArticleNumber.value)
    ) ?? null;
  });

  // Shared guard for `load()` and `switchLaw()`; stale awaits discard their writes.
  const claimSwitch = useLatest();

  async function load() {
    const isCurrent = claimSwitch();
    try {
      loading.value = true;
      let entry;
      if (initialDirectUrl) {
        // Direct URL: no law id to key the shared in-flight map on, so
        // fetch inline and cache under the body's resolved `$id`.
        const res = await apiFetch(initialDirectUrl, lawFetchInit);
        if (!isCurrent()) return;
        const text = await res.text();
        const parsed = yaml.load(text);
        entry = {
          law: parsed,
          rawYaml: text,
          lawName: resolveLawName(parsed),
          etag: res.headers.get('ETag'),
        };
        const resolvedId = parsed?.$id || lawParam;
        lawCache.set(lawCacheKey(currentTrajectRef, resolvedId), entry);
      } else {
        // Fresh fetch (never the cache), shared with any concurrent
        // `fetchLaw` for the same law so a mount doesn't fire duplicate
        // GETs. Cache writes happen inside `fetchLawFresh`.
        entry = await fetchLawFresh(currentTrajectRef, lawParam);
      }
      if (!isCurrent()) return;
      rawYaml.value = entry.rawYaml;
      law.value = entry.law;
      currentEtag.value = entry.etag;
      if (articles.value.length > 0 && !selectedArticleNumber.value) {
        if (initialArticle && articles.value.some(a => String(a.number) === initialArticle)) {
          selectedArticleNumber.value = initialArticle;
        } else {
          selectedArticleNumber.value = String(articles.value[0].number);
        }
      }
    } catch (e) {
      if (!isCurrent()) return;
      error.value = e;
    } finally {
      if (isCurrent()) {
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
    const isCurrent = claimSwitch();
    try {
      loading.value = true;
      error.value = null;
      saveError.value = null;
      saving.value = false;
      if (newTrajectRef !== undefined) {
        currentTrajectRef = newTrajectRef || null;
      }
      const entry = await fetchLaw(currentTrajectRef, newLawId);
      if (!isCurrent()) return;
      law.value = entry.law;
      rawYaml.value = entry.rawYaml;
      currentEtag.value = entry.etag ?? null;
      if (articleNumber) {
        selectedArticleNumber.value = String(articleNumber);
      } else if (articles.value.length > 0) {
        selectedArticleNumber.value = String(articles.value[0].number);
      }
    } catch (e) {
      if (!isCurrent()) return;
      error.value = e;
    } finally {
      if (isCurrent()) {
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
      const headers = {
        'Content-Type': 'text/yaml; charset=utf-8',
      };
      // Optimistic concurrency: echo the ETag of the version we loaded
      // so a concurrent edit by another traject member 412s instead of
      // being silently overwritten. Absent (older deployments, direct
      // URLs without ETag) the save stays a permissive blind write.
      if (currentEtag.value) headers['If-Match'] = currentEtag.value;
      const res = await apiFetch(lawUrl(savedTrajectRef, savedLawId), {
        method: 'PUT',
        headers,
        body: yamlText,
        // 412 resolves (handled as a conflict below); other failures throw.
        allowStatuses: [412],
        // Only surface the body when it's our editor-api speaking. The
        // editor-api returns plain `text/plain; charset=utf-8` for its
        // 400/403 bodies (corpus_handlers.rs), so a non-text/plain
        // content-type means a reverse proxy is intercepting (5xx HTML
        // page, etc.) and we should fall back to a generic message
        // rather than render proxy HTML in the save error dialog.
        errorMessage: (status, body, contentType) =>
          contentType.startsWith('text/plain') && body
            ? body
            : `Save failed: ${status}`,
      });
      if (res.status === 412) {
        throw new Error(
          'De wet is intussen door iemand anders gewijzigd. ' +
          'Herlaad de pagina om de nieuwste versie te zien en voer je ' +
          'wijziging daarna opnieuw door.',
        );
      }
      // Backend returns `{ pr: { url, number, branch } | null, etag }` on 200.
      let json = null;
      try {
        json = await res.json();
        lastSavedPr.value = sanitizeSavedPr(json?.pr);
      } catch {
        // Older deployments return a bare 200 without JSON — keep the
        // existing PR (if any) and treat the save as successful.
      }
      // Chain the new ETag for the next save. The header is
      // authoritative; the body echo is the fallback (mirrors
      // useTrajectDocuments).
      const newEtag = res.headers.get('ETag') ?? json?.etag ?? null;
      const parsed = yaml.load(yamlText);
      // Bail on the success path if the user navigated away mid-flight.
      if (lawId.value === savedLawId && currentTrajectRef === savedTrajectRef) {
        rawYaml.value = yamlText;
        law.value = parsed;
        currentEtag.value = newEtag;
      }
      const resolvedId = parsed?.$id || savedLawId;
      const savedKey = lawCacheKey(savedTrajectRef, resolvedId);
      lawCache.set(savedKey, {
        law: parsed,
        rawYaml: yamlText,
        lawName: resolveLawName(parsed),
        etag: newEtag,
      });
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
    currentEtag,
    lastSavedPr,
  };
}

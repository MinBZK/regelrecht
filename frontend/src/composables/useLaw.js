import { computed, ref, shallowRef } from 'vue';
import yaml from 'js-yaml';

// --- Shared law cache ---
const lawCache = new Map();

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

export async function fetchLaw(lawId) {
  if (lawCache.has(lawId)) return lawCache.get(lawId);
  const res = await fetch(`/api/corpus/laws/${encodeURIComponent(lawId)}`);
  if (!res.ok) throw new Error(`Failed to fetch: ${res.status}`);
  const text = await res.text();
  const law = yaml.load(text);
  const entry = { law, rawYaml: text, lawName: resolveLawName(law) };
  lawCache.set(lawId, entry);
  return entry;
}

export function useLaw(lawParam) {
  const params = new URLSearchParams(window.location.search);
  if (!lawParam) {
    lawParam = params.get('law') || 'zorgtoeslagwet';
  }
  const initialArticle = params.get('article') || null;
  // If the parameter looks like a URL, fetch directly; otherwise use the API.
  const yamlUrl = (lawParam.startsWith('/') || lawParam.startsWith('http'))
    ? lawParam
    : `/api/corpus/laws/${encodeURIComponent(lawParam)}`;
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

  async function load() {
    try {
      loading.value = true;
      const res = await fetch(yamlUrl);
      if (!res.ok) throw new Error(`Failed to fetch: ${res.status}`);
      const text = await res.text();
      rawYaml.value = text;
      law.value = yaml.load(text);
      // Populate cache
      const resolvedId = law.value?.$id || lawParam;
      if (!lawCache.has(resolvedId)) {
        lawCache.set(resolvedId, { law: law.value, rawYaml: text, lawName: resolveLawName(law.value) });
      }
      if (articles.value.length > 0 && !selectedArticleNumber.value) {
        if (initialArticle && articles.value.some(a => String(a.number) === initialArticle)) {
          selectedArticleNumber.value = initialArticle;
        } else {
          selectedArticleNumber.value = String(articles.value[0].number);
        }
      }
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  load();

  // Derive the law ID from the parsed law or the original param
  const lawId = computed(() => law.value?.$id || lawParam);

  let switchVersion = 0;

  async function switchLaw(newLawId, articleNumber) {
    const version = ++switchVersion;
    try {
      loading.value = true;
      error.value = null;
      // Reset save state too — a failed save on the previous law must not
      // leak its error dialog (or spinner) into the new law's Machine
      // panel. `saving` is cleared here alongside `saveError` because an
      // in-flight PUT from the previous law will still set `saving = false`
      // in its own `finally`, but until that stale response arrives the
      // new law should not inherit the spinner.
      saveError.value = null;
      saving.value = false;
      const entry = await fetchLaw(newLawId);
      if (version !== switchVersion) return; // stale, discard
      law.value = entry.law;
      rawYaml.value = entry.rawYaml;
      if (articleNumber) {
        selectedArticleNumber.value = String(articleNumber);
      } else if (articles.value.length > 0) {
        selectedArticleNumber.value = String(articles.value[0].number);
      }
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
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
    // Snapshot the law we're saving *before* the await. If the user
    // switches laws while the PUT is in flight, `switchLaw` will replace
    // `lawId` / `rawYaml` / `law` with the new law's state; when the stale
    // response eventually arrives, we must not overwrite the new law's
    // reactive state with the old law's YAML.
    const savedLawId = lawId.value;
    saving.value = true;
    saveError.value = null;
    try {
      const res = await fetch(
        `/api/corpus/laws/${encodeURIComponent(savedLawId)}`,
        {
          method: 'PUT',
          headers: { 'Content-Type': 'text/yaml; charset=utf-8' },
          body: yamlText,
        },
      );
      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || `Save failed: ${res.status}`);
      }
      // Parse once and reuse for both reactive state and the shared
      // lawCache so they remain referentially consistent.
      const parsed = yaml.load(yamlText);
      // Bail on the success path if the user navigated away mid-flight.
      // The write succeeded on the backend (so the cache update below is
      // still worth doing), but we must not touch the now-foreign
      // reactive refs.
      if (lawId.value === savedLawId) {
        rawYaml.value = yamlText;
        law.value = parsed;
      }
      // Keep the shared cache in sync so other tabs on the same law see
      // the edited version on their next fetchLaw() call. The cache key
      // is the saved law's ID, independent of the composable's current
      // `lawId` ref, so this refresh is safe even if the user switched.
      const resolvedId = parsed?.$id || savedLawId;
      lawCache.set(resolvedId, {
        law: parsed,
        rawYaml: yamlText,
        lawName: resolveLawName(parsed),
      });
    } catch (e) {
      // Only surface the error on the originating law's state. If the user
      // navigated away, the error belongs to law A and the new law's
      // Machine panel must not inherit it.
      if (lawId.value === savedLawId) {
        saveError.value = e;
      }
      throw e;
    } finally {
      // Same story for the spinner: only clear it if we're still on the
      // same law. If the user switched, switchLaw already reset `saving`
      // and we don't want to fight that.
      if (lawId.value === savedLawId) {
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
  };
}

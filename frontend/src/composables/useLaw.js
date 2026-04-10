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
      // leak its error dialog into the new law's Machine panel.
      saveError.value = null;
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
    saving.value = true;
    saveError.value = null;
    try {
      const res = await fetch(
        `/api/corpus/laws/${encodeURIComponent(lawId.value)}`,
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
      // Update local state so dirty-state tracking sees the edit as clean.
      rawYaml.value = yamlText;
      law.value = yaml.load(yamlText);
      // Keep the shared cache in sync so other tabs on the same law see the
      // edited version on their next fetchLaw() call.
      const resolvedId = law.value?.$id || lawId.value;
      lawCache.set(resolvedId, {
        law: law.value,
        rawYaml: yamlText,
        lawName: resolveLawName(law.value),
      });
    } catch (e) {
      saveError.value = e;
      throw e;
    } finally {
      saving.value = false;
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

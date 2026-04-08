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
  };
}

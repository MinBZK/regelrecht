import { computed, ref, shallowRef } from 'vue';
import yaml from 'js-yaml';

export function useLaw(lawParam) {
  if (!lawParam) {
    const params = new URLSearchParams(window.location.search);
    lawParam = params.get('law') || 'zorgtoeslagwet';
  }
  // If the parameter looks like a URL, fetch directly; otherwise use the API.
  const yamlUrl = (lawParam.startsWith('/') || lawParam.startsWith('http'))
    ? lawParam
    : `/api/corpus/laws/${encodeURIComponent(lawParam)}`;
  const law = shallowRef(null);
  const selectedArticleNumber = ref(null);
  const loading = ref(true);
  const error = ref(null);

  const articles = computed(() => law.value?.articles ?? []);

  const lawName = computed(() => {
    if (!law.value) return '';
    const nameRef = law.value.name;
    if (typeof nameRef === 'string' && nameRef.startsWith('#')) {
      const outputName = nameRef.slice(1);
      for (const article of articles.value) {
        const actions = article.machine_readable?.execution?.actions;
        if (!actions) continue;
        for (const action of actions) {
          if (action.output === outputName) {
            return action.value;
          }
        }
      }
    }
    return nameRef || law.value.$id || '';
  });

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
      law.value = yaml.load(text);
      if (articles.value.length > 0 && !selectedArticleNumber.value) {
        selectedArticleNumber.value = String(articles.value[0].number);
      }
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  load();

  return {
    law,
    articles,
    lawName,
    selectedArticle,
    selectedArticleNumber,
    loading,
    error,
  };
}

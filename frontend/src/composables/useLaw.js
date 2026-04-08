import { computed, ref, shallowRef } from 'vue';
import yaml from 'js-yaml';

export function useLaw(lawParam) {
  if (!lawParam) {
    const params = new URLSearchParams(window.location.search);
    lawParam = params.get('law') || 'wet_op_de_zorgtoeslag';
  }
  // If the parameter looks like a URL, fetch directly; otherwise use the API.
  const yamlUrl = (lawParam.startsWith('/') || lawParam.startsWith('http') || lawParam.startsWith('blob:'))
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
      let text;
      if (lawParam === 'local') {
        text = localStorage.getItem('regelrecht_yaml');
        if (!text) throw new Error('Geen YAML data gevonden in localStorage');
      } else {
        const res = await fetch(yamlUrl);
        if (!res.ok) throw new Error(`Failed to fetch: ${res.status}`);
        text = await res.text();
      }
      law.value = yaml.load(text);
      const articleParam = new URLSearchParams(window.location.search).get('article');
      if (articleParam && articles.value.some(a => String(a.number) === articleParam)) {
        selectedArticleNumber.value = articleParam;
      } else if (articles.value.length > 0 && !selectedArticleNumber.value) {
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

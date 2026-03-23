import { computed, ref, shallowRef } from 'vue';
import yaml from 'js-yaml';

/** Derive the admin backend URL from the current editor origin. */
function getAdminUrl() {
  const origin = window.location.origin;
  // RIG naming: editor-{name}-regel-k4c → admin-{name}-regel-k4c
  if (origin.includes('editor-')) {
    return origin.replace('editor-', 'admin-');
  }
  // Local dev: assume admin runs on port 3001
  return origin.replace(/:\d+$/, ':3001');
}

export function useLaw(yamlUrl) {
  if (!yamlUrl) {
    const params = new URLSearchParams(window.location.search);
    const lawParam = params.get('law') || '/data/local/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml';
    // Bare law_id (not a path or URL) → fetch via admin API
    if (lawParam && !lawParam.startsWith('/') && !lawParam.startsWith('http')) {
      yamlUrl = `${getAdminUrl()}/api/corpus/laws/${encodeURIComponent(lawParam)}`;
    } else {
      yamlUrl = lawParam;
    }
  }
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

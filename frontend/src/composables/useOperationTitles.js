import { ref, watch } from 'vue';

export function useOperationTitles(operationTree) {
  const aiTitles = ref({});
  const loading = ref(false);

  async function fetchTitles(tree) {
    if (!tree || tree.length === 0) {
      aiTitles.value = {};
      return;
    }

    loading.value = true;

    try {
      const res = await fetch('/api/ai/operation-titles', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ operations: tree }),
      });

      if (!res.ok) return;

      const data = await res.json();
      aiTitles.value = data.titles || {};
    } catch {
      // Silently degrade — fallback titles remain
    } finally {
      loading.value = false;
    }
  }

  watch(operationTree, (newTree) => {
    fetchTitles(newTree);
  }, { immediate: true });

  return { aiTitles, loading };
}

/**
 * useScenarios — fetch and manage scenario files for a law.
 */
import { ref, watch } from 'vue';

export function useScenarios(lawId) {
  const scenarios = ref([]);
  const selectedScenario = ref(null);
  const featureText = ref('');
  const loading = ref(false);
  const error = ref(null);

  async function fetchScenarios() {
    if (!lawId.value) return;

    loading.value = true;
    error.value = null;

    try {
      const res = await fetch(
        `/api/corpus/laws/${encodeURIComponent(lawId.value)}/scenarios`,
      );
      if (!res.ok) {
        scenarios.value = [];
        return;
      }
      scenarios.value = await res.json();

      // Auto-select first scenario
      if (scenarios.value.length > 0 && !selectedScenario.value) {
        await selectScenario(scenarios.value[0].filename);
      }
    } catch (e) {
      error.value = e;
      scenarios.value = [];
    } finally {
      loading.value = false;
    }
  }

  async function selectScenario(filename) {
    selectedScenario.value = filename;

    try {
      const res = await fetch(
        `/api/corpus/laws/${encodeURIComponent(lawId.value)}/scenarios/${encodeURIComponent(filename)}`,
      );
      if (!res.ok) throw new Error(`Failed to fetch scenario: ${res.status}`);
      featureText.value = await res.text();
    } catch (e) {
      error.value = e;
      featureText.value = '';
    }
  }

  // Re-fetch when lawId changes
  watch(lawId, () => {
    selectedScenario.value = null;
    featureText.value = '';
    fetchScenarios();
  }, { immediate: true });

  return {
    scenarios,
    selectedScenario,
    featureText,
    loading,
    error,
    selectScenario,
    fetchScenarios,
  };
}

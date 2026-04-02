/**
 * useScenarios — fetch, manage, and save scenario files for a law.
 */
import { ref, watch } from 'vue';

export function useScenarios(lawId) {
  const scenarios = ref([]);
  const selectedScenario = ref(null);
  const featureText = ref('');
  const loading = ref(false);
  const saving = ref(false);
  const error = ref(null);
  const saveError = ref(null);

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
      selectedScenario.value = null;
    }
  }

  /**
   * Save scenario content to the backend via PUT.
   *
   * @param {string} filename - Scenario filename (e.g. "eligibility.feature")
   * @param {string} content - Gherkin feature text
   */
  async function saveScenario(filename, content) {
    if (!lawId.value) return;

    saving.value = true;
    saveError.value = null;

    try {
      const res = await fetch(
        `/api/corpus/laws/${encodeURIComponent(lawId.value)}/scenarios/${encodeURIComponent(filename)}`,
        {
          method: 'PUT',
          headers: { 'Content-Type': 'text/plain; charset=utf-8' },
          body: content,
        },
      );
      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || `Save failed: ${res.status}`);
      }
      // Update local state with saved content
      featureText.value = content;
    } catch (e) {
      saveError.value = e;
      throw e;
    } finally {
      saving.value = false;
    }
  }

  // Re-fetch when lawId changes
  watch(lawId, () => {
    selectedScenario.value = null;
    featureText.value = '';
    fetchScenarios().catch(() => {});
  }, { immediate: true });

  return {
    scenarios,
    selectedScenario,
    featureText,
    loading,
    saving,
    error,
    saveError,
    selectScenario,
    fetchScenarios,
    saveScenario,
  };
}

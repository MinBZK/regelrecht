/**
 * useScenarios — fetch, manage, and save scenario files for a law.
 *
 * Reads pick the global or traject-scoped URL based on `trajectRef`;
 * writes only succeed with a traject, matching the backend invariant
 * that scenario writes live under `/api/trajects/{tid}/corpus/...`.
 *
 * @param {import('vue').Ref<string>} lawId
 * @param {import('vue').Ref<string|null>} trajectRef active traject id,
 *   `null` for a global read context (no edits possible)
 */
import { ref, watch } from 'vue';
import { getEditorSessionId, lastSavedPr, sanitizeSavedPr } from './useEditorSession.js';
import { requireTraject, scenarioFileUrl, scenariosListUrl } from './corpusUrls.js';

export function useScenarios(lawId, trajectRef) {
  const scenarios = ref([]);
  const selectedScenario = ref(null);
  const featureText = ref('');
  const loading = ref(false);
  const saving = ref(false);
  const error = ref(null);
  const saveError = ref(null);
  // `lastSavedPr` is imported as a module-shared ref from useEditorSession
  // so scenario saves and law-content saves both update the same value,
  // and EditorApp's "Bekijk op GitHub" badge stays in sync regardless of
  // which pane the user pressed Save in.

  async function fetchScenarios() {
    if (!lawId.value) return;

    loading.value = true;
    error.value = null;
    // Drop any stale save error from a previously selected law so the
    // banner does not linger after navigating to a different law.
    saveError.value = null;

    try {
      const res = await fetch(scenariosListUrl(trajectRef.value, lawId.value));
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
    saveError.value = null;

    try {
      const res = await fetch(scenarioFileUrl(trajectRef.value, lawId.value, filename));
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
    requireTraject(trajectRef.value, 'scenario save');

    saving.value = true;
    saveError.value = null;

    try {
      const res = await fetch(
        scenarioFileUrl(trajectRef.value, lawId.value, filename),
        {
          method: 'PUT',
          headers: {
            'Content-Type': 'text/plain; charset=utf-8',
            'X-Editor-Session': getEditorSessionId(),
          },
          body: content,
        },
      );
      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || `Save failed: ${res.status}`);
      }
      // Same `{ pr }` shape as the law-content save; capture for the
      // shared "Bekijk op GitHub" badge in EditorApp.vue.
      try {
        const json = await res.json();
        lastSavedPr.value = sanitizeSavedPr(json?.pr);
      } catch {
        // Older deployments returned a bare 200 — keep the prior PR.
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

  // Re-fetch when the law id OR active traject changes. Switching
  // traject (URL change) re-routes reads through the new traject's
  // backends, so a refresh is needed even if the law id stayed the same.
  watch([lawId, trajectRef], () => {
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
    lastSavedPr,
  };
}

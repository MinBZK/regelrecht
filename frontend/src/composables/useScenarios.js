/**
 * useScenarios - fetch, manage, and save scenario files for a law.
 *
 * Reads pick the global or traject-scoped URL based on `trajectRef`;
 * writes only succeed with a traject, matching the backend invariant
 * that scenario writes live under `/api/trajects/{tid}/corpus/...`.
 *
 * @param {import('vue').Ref<string>} lawId
 * @param {import('vue').Ref<string|null>=} trajectRef Active traject
 *   ref. Defaults to `ref(null)` so omitting the second arg gives a
 *   read-only / global-scope view instead of a `TypeError: Cannot read
 *   properties of undefined` deep inside the fetch.
 */
import { ref, watch } from 'vue';
import { lastSavedPr, sanitizeSavedPr } from './useSavedPr.js';
import { requireTraject, scenarioFileUrl, scenariosListUrl } from './corpusUrls.js';
import { apiFetch } from '../lib/apiFetch.js';

/**
 * True when the scenario file declares execution targets and none of
 * them is the opened law - running it would evaluate a different law.
 * Files without a parseable execution step (`target_law_ids` empty)
 * count as "unknown", not as a mismatch.
 */
export function isScenarioMismatch(entry, lawId) {
  const targets = entry?.target_law_ids || [];
  return targets.length > 0 && !targets.includes(lawId);
}

export function useScenarios(lawId, trajectRef = ref(null)) {
  const scenarios = ref([]);
  const selectedScenario = ref(null);
  const featureText = ref('');
  const loading = ref(false);
  const saving = ref(false);
  const error = ref(null);
  const saveError = ref(null);
  // ETag per scenario filename, captured on read and echoed back as
  // `If-Match` on save so a concurrent edit by another traject member
  // surfaces as a 412 instead of a silent overwrite. A filename without
  // an entry (e.g. a brand-new scenario) saves without a precondition.
  const scenarioEtags = new Map();
  // `lastSavedPr` is imported as a module-shared ref from useSavedPr
  // so scenario saves and law-content saves both update the same value,
  // and EditorApp's "Bekijk op GitHub" badge stays in sync regardless of
  // which pane the user pressed Save in.

  async function fetchScenarios() {
    // Clear before the early return, not after: navigating from a failed law
    // to "no law selected" would otherwise leave the banner up.
    error.value = null;
    // Drop any stale save error from a previously selected law so the
    // banner does not linger after navigating to a different law.
    saveError.value = null;
    if (!lawId.value) return;

    // Snapshot the scope before the await, like saveScenario does. The read
    // failure is now rendered, so a slow 502 for law A must not paint law B's
    // screen with A's error after the user navigated away.
    const savedLawId = lawId.value;
    const savedTrajectRef = trajectRef.value;
    const stillInScope = () =>
      lawId.value === savedLawId && trajectRef.value === savedTrajectRef;

    loading.value = true;

    try {
      // A law without scenarios is already a 200 with `[]` (the backend's
      // `list_scenarios_in_scope` returns that for a missing directory), so no
      // status needs to be read as "empty" here. Every non-ok status is a real
      // failure: 404 means the law is not in this scope at all, 500 a broken
      // traject scan, 502 an unreachable corpus backend. They used to become
      // the same empty list, so "we could not read the scenarios" and "this
      // law has none" were one screen.
      const res = await apiFetch(scenariosListUrl(trajectRef.value, lawId.value), {
        errorMessage: (status) =>
          status === 404
            ? `Deze wet is niet gevonden in deze scope (${status})`
            : `Scenario's konden niet worden geladen: ${status}`,
      });
      const listed = await res.json();
      if (!stillInScope()) return;
      scenarios.value = listed;

      // Auto-select the first scenario that explicitly targets this law;
      // then prefer files whose targets are unknown (no parseable
      // execution step) over known mismatches; finally fall back to the
      // first file. Folder placement is no longer the source of truth
      // for the law↔scenario binding - the file's execution steps are.
      if (scenarios.value.length > 0 && !selectedScenario.value) {
        const preferred =
          scenarios.value.find((s) => s.target_law_ids?.includes(lawId.value)) ||
          scenarios.value.find((s) => !isScenarioMismatch(s, lawId.value)) ||
          scenarios.value[0];
        await selectScenario(preferred.filename);
      }
    } catch (e) {
      if (!stillInScope()) return;
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
      const res = await apiFetch(
        scenarioFileUrl(trajectRef.value, lawId.value, filename),
        { errorMessage: (status) => `Failed to fetch scenario: ${status}` },
      );
      featureText.value = await res.text();
      const etag = res.headers.get('ETag');
      if (etag) scenarioEtags.set(filename, etag);
      else scenarioEtags.delete(filename);
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

    // Snapshot the scope before the await (mirrors useLaw.saveLaw): if
    // the user switches law/traject while the PUT is in flight, the
    // watch below clears `scenarioEtags` for the new scope - the stale
    // completion must not re-insert its ETag (keys are bare filenames
    // like `basis.feature`, which recur across laws, so the entry would
    // poison the new law's same-named scenario with a foreign
    // precondition) nor overwrite the new scope's featureText.
    const savedLawId = lawId.value;
    const savedTrajectRef = trajectRef.value;

    saving.value = true;
    saveError.value = null;

    try {
      const headers = {
        'Content-Type': 'text/plain; charset=utf-8',
      };
      // Echo the ETag we read so the backend can detect a concurrent
      // edit (412). New scenarios have no entry → permissive create.
      const ifMatch = scenarioEtags.get(filename);
      if (ifMatch) headers['If-Match'] = ifMatch;
      const res = await apiFetch(
        scenarioFileUrl(trajectRef.value, lawId.value, filename),
        {
          method: 'PUT',
          headers,
          body: content,
          // 412 resolves (handled as a conflict below); other failures throw.
          allowStatuses: [412],
          errorMessage: (status, body) => body || `Save failed: ${status}`,
        },
      );
      if (res.status === 412) {
        throw new Error(
          'Het scenario is intussen door iemand anders gewijzigd. ' +
          'Herlaad het scenario om de nieuwste versie te zien en voer ' +
          'je wijziging daarna opnieuw door.',
        );
      }
      // Same `{ pr, etag }` shape as the law-content save; capture the
      // PR for the shared "Bekijk op GitHub" badge in EditorApp.vue.
      let json = null;
      try {
        json = await res.json();
        lastSavedPr.value = sanitizeSavedPr(json?.pr);
      } catch {
        // Older deployments returned a bare 200 - keep the prior PR.
      }
      // Bail on the success path if the user switched law/traject
      // mid-flight - the new scope's state must not absorb this save.
      if (lawId.value !== savedLawId || trajectRef.value !== savedTrajectRef) return;
      // Chain the new ETag for the next save of this scenario. Header
      // is authoritative; body echo is the fallback.
      const newEtag = res.headers.get('ETag') ?? json?.etag ?? null;
      if (newEtag) scenarioEtags.set(filename, newEtag);
      else scenarioEtags.delete(filename);
      // Update local state with saved content
      featureText.value = content;
    } catch (e) {
      if (lawId.value === savedLawId && trajectRef.value === savedTrajectRef) {
        saveError.value = e;
      }
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
    // ETags belong to the previous law/traject's files - a save in the
    // new scope must not carry a stale precondition.
    scenarioEtags.clear();
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

/**
 * useDependencies — recursive dependency graph walker for law YAML files.
 *
 * Parses a law's YAML structure to find all `source.regulation` references,
 * fetches each dependency via the API, loads it into the engine, and recurses.
 * Implementing regulations (the IoC reverse link) are loaded separately via
 * `loadImplementors`, which calls the backend `implementors` endpoint — kept
 * OFF the critical path because that corpus scan can be slow on a large
 * federated corpus and scenarios already declare the regulations they need.
 */
import { ref } from 'vue';
import yaml from 'js-yaml';
import { useBwbHarvest } from './useBwbHarvest.js';
import { loadLawVersions } from './useEngine.js';
import { implementorsUrl } from './corpusUrls.js';
import { apiFetchJson } from '../lib/apiFetch.js';

/**
 * Extract all unique `source.regulation` references from a parsed law object.
 * Skips self-references (where regulation === the law's own $id).
 *
 * @param {object} law - Parsed law YAML object
 * @returns {string[]} Array of unique law IDs referenced
 */
export function extractRegulationRefs(law) {
  const refs = new Set();
  const selfId = law.$id;

  for (const article of law.articles || []) {
    const inputs = article.machine_readable?.execution?.input || [];
    for (const input of inputs) {
      const reg = input.source?.regulation;
      if (reg && reg !== selfId) {
        refs.add(reg);
      }
    }
  }

  return [...refs];
}

/**
 * Composable for loading all dependencies of a law recursively.
 */
export function useDependencies() {
  const loading = ref(false);
  const loadedDeps = ref([]);
  const progress = ref('');
  const error = ref(null);
  const { requestHarvestBatch } = useBwbHarvest();

  // Guards `loadImplementors` so a re-render / re-trigger of the scenario
  // panel doesn't restart the corpus-wide implementor scan for a law it
  // already scanned. Keyed on `{trajectRef}::{lawId}`.
  let implementorsKey = null;

  /**
   * Load a law's direct + transitive `source.regulation` dependencies into
   * the engine. Returns the law's `$id` (or null) so the caller can kick off
   * the off-critical-path `loadImplementors` scan.
   *
   * `fetchLawVersions` already resolves through the active traject, so no
   * traject ref is needed here. It returns **all** versions of a law (the
   * engine keys versions by `($id, valid_from)`, so several bodies of one law
   * coexist), and the engine picks the version in force on the scenario's
   * calculation date — a referenced law that has a future-dated version would
   * otherwise load only that future version and fail "not yet in force".
   *
   * @param {string} lawYamlText - Raw YAML text of the main law
   * @param {object} engine - WasmEngine instance
   * @param {(lawId: string) => Promise<string[]>} fetchLawVersions - Fetch all version YAMLs by ID
   * @returns {Promise<string|null>} The main law's `$id`.
   */
  async function loadAllDependencies(lawYamlText, engine, fetchLawVersions) {
    loading.value = true;
    error.value = null;
    loadedDeps.value = [];
    progress.value = 'Afhankelijkheden analyseren...';

    let mainLawId = null;
    try {
      const mainLaw = yaml.load(lawYamlText);
      mainLawId = mainLaw.$id || null;
      const visited = new Set();
      const toLoad = [];

      // Phase 1: Collect all transitive regulation references
      collectDeps(mainLaw, visited, toLoad);

      // Phase 2: Load all collected dependencies
      let total = toLoad.length;
      let loaded = 0;
      const missingDeps = [];

      for (const lawId of toLoad) {
        if (engine.hasLaw(lawId)) {
          loaded++;
          loadedDeps.value = [...loadedDeps.value, lawId];
          progress.value = `${loaded}/${total} wetten geladen`;
          continue;
        }

        try {
          const yamls = await fetchLawVersions(lawId);
          // Load every version (isolating each) so the engine's date-aware
          // selection can pick the one in force on the scenario's calculation
          // date. No version loading — empty list or every version unloadable —
          // is treated as a missing dependency (harvest requested below).
          if (!loadLawVersions(engine, yamls, lawId)) {
            throw new Error(`no loadable version for '${lawId}'`);
          }
          loaded++;
          loadedDeps.value = [...loadedDeps.value, lawId];
          progress.value = `${loaded}/${total} wetten geladen`;

          // Recurse for transitive deps. Collect from every version — a
          // `source.regulation` reference can appear in one version and not
          // another, and a scenario may be evaluated at any calculation date
          // (including a future one), so a reference introduced only by a
          // future version must still be loadable. This can over-fetch a
          // transitive law that no in-force version needs; that's an accepted,
          // bounded cost — the engine's `select_in` simply never selects a
          // not-yet-in-force version.
          const newDeps = [];
          for (const versionYaml of yamls) {
            // Isolate the ref scan per version too: a version that loaded into
            // the engine but is malformed for `js-yaml` must not throw here and
            // get the (already-loaded) law spuriously marked missing + harvested.
            try {
              collectDeps(yaml.load(versionYaml), visited, newDeps);
            } catch (e) {
              console.warn(`Skipped transitive-ref scan of an unparseable version of '${lawId}':`, e);
            }
          }
          if (newDeps.length > 0) {
            toLoad.push(...newDeps);
            total = toLoad.length;
          }
        } catch (e) {
          console.warn(`Failed to load dependency '${lawId}':`, e);
          missingDeps.push(lawId);
          loaded++;
          progress.value = `${loaded}/${total} wetten geladen (${lawId} mislukt)`;
        }
      }

      // Phase 4: Request harvest for missing dependencies
      const defaultProgress = total > 0
        ? `${loadedDeps.value.length}/${total} wetten geladen`
        : 'Geen afhankelijkheden';

      if (missingDeps.length > 0) {
        const harvestResult = await requestHarvestBatch(missingDeps);
        const requested = harvestResult?.results?.filter(
          (r) => r.status === 'queued' || r.status === 'already_queued',
        ) ?? [];
        progress.value = requested.length > 0
          ? `${defaultProgress} \u2014 ${requested.length} ontbrekende wet(ten) aangevraagd`
          : defaultProgress;
      } else {
        progress.value = defaultProgress;
      }
    } catch (e) {
      error.value = e.message || String(e);
    } finally {
      loading.value = false;
    }
    return mainLawId;
  }

  /**
   * Load implementing regulations (the IoC reverse link) into the engine.
   *
   * Deliberately OFF the critical path: the backend implementors scan can be
   * slow on a large federated corpus, and scenarios declare the regulations
   * they need explicitly (their `Given law "x" is loaded` background), so the
   * scenario panel must not block on this. The caller fires it without
   * awaiting, after the panel is already usable. Runs at most once per
   * `(trajectRef, lawId)` so re-renders don't restart the scan.
   *
   * @param {string|null} lawId - The `$id` of the law to find implementors of.
   * @param {object} engine - WasmEngine instance.
   * @param {(lawId: string) => Promise<string[]>} fetchLawVersions - Fetch all version YAMLs.
   * @param {string|null} trajectRef - Active traject reference.
   */
  async function loadImplementors(lawId, engine, fetchLawVersions, trajectRef = null) {
    if (!lawId) return;
    const key = `${trajectRef || ''}::${lawId}`;
    if (implementorsKey === key) return;
    // Claim the key up front so a concurrent re-trigger doesn't start a second
    // scan; reset it on failure so a transient error (network blip, throttled
    // backend) can be retried on the next trigger rather than suppressed for
    // the component's lifetime.
    implementorsKey = key;
    let implementors;
    try {
      implementors = await apiFetchJson(implementorsUrl(trajectRef, lawId));
    } catch {
      // Best-effort: if the scan fails (HTTP error or network), explicitly-
      // declared deps still cover the common case. Allow a retry on the
      // next trigger.
      implementorsKey = null;
      return;
    }
    if (!Array.isArray(implementors)) {
      // A 200 with a non-array body (proxy error page parsed as JSON, null)
      // must stay retryable, like any other failed scan.
      implementorsKey = null;
      return;
    }
    for (const implId of implementors) {
      try {
        if (!engine.hasLaw(implId)) {
          const yamls = await fetchLawVersions(implId);
          if (loadLawVersions(engine, yamls, implId)) {
            loadedDeps.value = [...loadedDeps.value, implId];
          }
        }
      } catch (e) {
        console.warn(`Failed to load implementing regulation '${implId}':`, e);
      }
    }
  }

  return { loading, loadedDeps, progress, error, loadAllDependencies, loadImplementors };
}

/**
 * Recursively collect dependency law IDs from a parsed law.
 * Mutates `visited` and `toLoad` in place.
 */
function collectDeps(law, visited, toLoad) {
  const selfId = law.$id;
  if (selfId) visited.add(selfId);

  const refs = extractRegulationRefs(law);
  for (const depId of refs) {
    if (!visited.has(depId)) {
      visited.add(depId);
      toLoad.push(depId);
    }
  }
}

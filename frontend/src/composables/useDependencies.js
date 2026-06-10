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
import { implementorsUrl } from './corpusUrls.js';

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
   * `fetchLawYaml` already resolves through the active traject, so no traject
   * ref is needed here.
   *
   * @param {string} lawYamlText - Raw YAML text of the main law
   * @param {object} engine - WasmEngine instance
   * @param {(lawId: string) => Promise<string>} fetchLawYaml - Fetch law YAML by ID
   * @returns {Promise<string|null>} The main law's `$id`.
   */
  async function loadAllDependencies(lawYamlText, engine, fetchLawYaml) {
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
          const yamlText = await fetchLawYaml(lawId);
          engine.loadLaw(yamlText);
          loaded++;
          loadedDeps.value = [...loadedDeps.value, lawId];
          progress.value = `${loaded}/${total} wetten geladen`;

          // Recurse into newly loaded law for transitive deps
          const depLaw = yaml.load(yamlText);
          const newDeps = [];
          collectDeps(depLaw, visited, newDeps);
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
   * @param {(lawId: string) => Promise<string>} fetchLawYaml - Fetch law YAML.
   * @param {string|null} trajectRef - Active traject reference.
   */
  async function loadImplementors(lawId, engine, fetchLawYaml, trajectRef = null) {
    if (!lawId) return;
    const key = `${trajectRef || ''}::${lawId}`;
    if (implementorsKey === key) return;
    // Claim the key up front so a concurrent re-trigger doesn't start a second
    // scan; reset it on failure so a transient error (network blip, throttled
    // backend) can be retried on the next trigger rather than suppressed for
    // the component's lifetime.
    implementorsKey = key;
    try {
      const res = await fetch(implementorsUrl(trajectRef, lawId));
      if (!res.ok) {
        implementorsKey = null;
        return;
      }
      const implementors = await res.json();
      for (const implId of implementors) {
        try {
          if (!engine.hasLaw(implId)) {
            const yamlText = await fetchLawYaml(implId);
            engine.loadLaw(yamlText);
            loadedDeps.value = [...loadedDeps.value, implId];
          }
        } catch (e) {
          console.warn(`Failed to load implementing regulation '${implId}':`, e);
        }
      }
    } catch {
      // Best-effort: if the scan fails, explicitly-declared deps still cover
      // the common case. Allow a retry on the next trigger.
      implementorsKey = null;
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

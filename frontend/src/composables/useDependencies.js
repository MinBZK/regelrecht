/**
 * useDependencies — recursive dependency graph walker for law YAML files.
 *
 * Parses a law's YAML structure to find all `source.regulation` references,
 * fetches each dependency via the API, loads it into the engine, and recurses.
 * Also pulls in implementing regulations (the IoC reverse link) via the
 * backend `implementors` endpoint, which scans the in-memory corpus
 * server-side — one request instead of fetching and parsing every law.
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

  /**
   * Load all dependencies for a law, recursively.
   *
   * @param {string} lawYamlText - Raw YAML text of the main law
   * @param {object} engine - WasmEngine instance
   * @param {(lawId: string) => Promise<string>} fetchLawYaml - Fetch law YAML by ID
   * @param {string|null} trajectRef - Active traject reference. Drives
   *   which corpus list is scanned for implementing regulations so
   *   in-progress trajects pick up their own implementors.
   */
  async function loadAllDependencies(lawYamlText, engine, fetchLawYaml, trajectRef = null) {
    loading.value = true;
    error.value = null;
    loadedDeps.value = [];
    progress.value = 'Afhankelijkheden analyseren...';

    try {
      const mainLaw = yaml.load(lawYamlText);
      const visited = new Set();
      const toLoad = [];

      // Phase 1: Collect all transitive regulation references
      collectDeps(mainLaw, visited, toLoad);

      // Phase 2: Discover implementing regulations (IoC reverse link).
      // The backend scans the in-memory corpus and returns just the
      // implementing law ids, so this is a single request — not a
      // fetch-and-parse of every law in the (possibly federated, hundreds
      // strong) corpus. Best-effort: a failure here only means implementing
      // regulations aren't auto-loaded, the rest of the scan still runs.
      if (mainLaw.$id) {
        try {
          const res = await fetch(implementorsUrl(trajectRef, mainLaw.$id));
          if (res.ok) {
            const implementors = await res.json();
            for (const implId of implementors) {
              if (!visited.has(implId)) {
                visited.add(implId);
                toLoad.push(implId);
              }
            }
          }
        } catch {
          // Implementor discovery is best-effort.
        }
      }

      // Phase 3: Load all collected dependencies
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
  }

  return { loading, loadedDeps, progress, error, loadAllDependencies };
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

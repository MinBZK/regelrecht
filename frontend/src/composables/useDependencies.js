/**
 * useDependencies — recursive dependency graph walker for law YAML files.
 *
 * Parses a law's YAML structure to find all `source.regulation` references,
 * fetches each dependency via the API, loads it into the engine, and recurses.
 * Also discovers implementing regulations via corpus scan.
 */
import { ref } from 'vue';
import yaml from 'js-yaml';

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
 * Find laws in the corpus that implement open_terms of the given law.
 *
 * @param {string} lawId - The law ID to find implementors for
 * @param {object[]} allLaws - Full corpus law list (from /api/corpus/laws)
 * @returns {Promise<string[]>} Law IDs that implement open_terms of lawId
 */
async function discoverImplementors(lawId, allLaws, fetchLawYaml) {
  const candidates = allLaws.filter((entry) => entry.law_id !== lawId);

  // Fetch all candidate laws in parallel (batched)
  const BATCH_SIZE = 10;
  const implementors = [];

  for (let i = 0; i < candidates.length; i += BATCH_SIZE) {
    const batch = candidates.slice(i, i + BATCH_SIZE);
    const results = await Promise.allSettled(
      batch.map(async (entry) => {
        let text;
        try {
          text = await fetchLawYaml(entry.law_id);
        } catch {
          return null;
        }
        const law = yaml.load(text);

        for (const article of law.articles || []) {
          const impls = article.machine_readable?.implements || [];
          for (const impl of impls) {
            if (impl.law === lawId) {
              return law.$id || entry.law_id;
            }
          }
        }
        return null;
      }),
    );

    for (const result of results) {
      if (result.status === 'fulfilled' && result.value) {
        implementors.push(result.value);
      }
    }
  }

  return implementors;
}

/**
 * Composable for loading all dependencies of a law recursively.
 */
export function useDependencies() {
  const loading = ref(false);
  const loadedDeps = ref([]);
  const progress = ref('');
  const error = ref(null);

  /**
   * Load all dependencies for a law, recursively.
   *
   * @param {string} lawYamlText - Raw YAML text of the main law
   * @param {object} engine - WasmEngine instance
   * @param {(lawId: string) => Promise<string>} fetchLawYaml - Fetch law YAML by ID
   */
  async function loadAllDependencies(lawYamlText, engine, fetchLawYaml) {
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

      // Phase 2: Discover implementing regulations
      try {
        const corpusRes = await fetch('/api/corpus/laws?limit=1000');
        if (corpusRes.ok) {
          const allLaws = await corpusRes.json();

          // Check for implementors of the main law
          const implementors = await discoverImplementors(
            mainLaw.$id,
            allLaws,
            fetchLawYaml,
          );
          for (const implId of implementors) {
            if (!visited.has(implId)) {
              visited.add(implId);
              toLoad.push(implId);
            }
          }
        }
      } catch {
        // Corpus scan is best-effort
      }

      // Phase 3: Load all collected dependencies
      let total = toLoad.length;
      let loaded = 0;

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
          loaded++;
          progress.value = `${loaded}/${total} wetten geladen (${lawId} mislukt)`;
        }
      }

      progress.value = total > 0
        ? `${loadedDeps.value.length}/${total} wetten geladen`
        : 'Geen afhankelijkheden';
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
  for (const ref of refs) {
    if (!visited.has(ref)) {
      visited.add(ref);
      toLoad.push(ref);
    }
  }
}

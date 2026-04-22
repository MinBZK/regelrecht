/**
 * useDependencies — cross-law reference graph extractor + loader.
 *
 * Exports:
 *   Per-law extractors (sync, on a parsed law object):
 *     - extractRegulationRefs(law)    → unique target law_ids via source.regulation
 *     - extractImplements(law)        → [{article, law, article: target, open_term, gelet_op}]
 *     - extractOverrides(law)         → [{article, law, article: target, output}]
 *     - extractOpenTerms(law)         → [{article, id, delegated_to, delegation_type, legal_basis}]
 *     - extractEnables(law)           → [{article, regulatory_layer, subject, ...}]
 *     - extractLegalBasis(law)        → top-level legal_basis array as-is
 *     - extractArticleSources(law)    → [{article, inputs: [{name, regulation, output, params}]}]
 *
 *   Corpus-scanning reverse-lookups (async, need fetchLawYaml + allLaws list):
 *     - discoverImplementors(lawId, allLaws, fetchLawYaml[, targetArticle])
 *     - discoverOverriders(lawId, allLaws, fetchLawYaml[, targetArticle[, targetOutput]])
 *     - discoverUsers(lawId, allLaws, fetchLawYaml[, targetOutput])
 *
 *   Recursive loader composable:
 *     - useDependencies() — loadAllDependencies(yamlText, engine, fetchLawYaml)
 */
import { ref } from 'vue';
import yaml from 'js-yaml';

// ---------- Sync extractors ----------

export function extractRegulationRefs(law) {
  const refs = new Set();
  const selfId = law.$id;
  for (const article of law.articles || []) {
    const inputs = article.machine_readable?.execution?.input || [];
    for (const input of inputs) {
      const reg = input.source?.regulation;
      if (reg && reg !== selfId) refs.add(reg);
    }
  }
  return [...refs];
}

export function extractImplements(law) {
  const out = [];
  for (const article of law.articles || []) {
    const impls = article.machine_readable?.implements || [];
    for (const impl of impls) {
      out.push({
        article: String(article.number),
        target_law: impl.law,
        target_article: impl.article != null ? String(impl.article) : undefined,
        open_term: impl.open_term,
        gelet_op: impl.gelet_op,
      });
    }
  }
  return out;
}

export function extractOverrides(law) {
  const out = [];
  for (const article of law.articles || []) {
    const ovs = article.machine_readable?.overrides || [];
    for (const ov of ovs) {
      out.push({
        article: String(article.number),
        target_law: ov.law,
        target_article: ov.article != null ? String(ov.article) : undefined,
        target_output: ov.output,
      });
    }
  }
  return out;
}

export function extractOpenTerms(law) {
  const out = [];
  for (const article of law.articles || []) {
    const terms = article.machine_readable?.open_terms || [];
    for (const term of terms) {
      out.push({
        article: String(article.number),
        id: term.id,
        type: term.type,
        delegated_to: term.delegated_to,
        delegation_type: term.delegation_type,
        legal_basis: term.legal_basis,
        required: term.required,
        description: term.description,
      });
    }
  }
  return out;
}

export function extractEnables(law) {
  const out = [];
  for (const article of law.articles || []) {
    const en = article.machine_readable?.enables || [];
    for (const entry of en) {
      out.push({
        article: String(article.number),
        regulatory_layer: entry.regulatory_layer,
        subject: entry.subject,
        interface: entry.interface,
      });
    }
  }
  return out;
}

export function extractLegalBasis(law) {
  return Array.isArray(law.legal_basis) ? law.legal_basis.slice() : [];
}

export function extractArticleSources(law) {
  const out = [];
  for (const article of law.articles || []) {
    const inputs = article.machine_readable?.execution?.input || [];
    const items = [];
    for (const input of inputs) {
      if (!input.source) continue;
      items.push({
        name: input.name,
        regulation: input.source.regulation,
        output: input.source.output,
        parameters: input.source.parameters,
      });
    }
    if (items.length) out.push({ article: String(article.number), inputs: items });
  }
  return out;
}

// ---------- Async reverse-lookups over the corpus ----------

const BATCH_SIZE = 10;

async function scanCorpus(allLaws, fetchLawYaml, visitor) {
  const results = [];
  for (let i = 0; i < allLaws.length; i += BATCH_SIZE) {
    const batch = allLaws.slice(i, i + BATCH_SIZE);
    const batchResults = await Promise.allSettled(
      batch.map(async (entry) => {
        let text;
        try {
          text = await fetchLawYaml(entry.law_id);
        } catch {
          return null;
        }
        const law = yaml.load(text);
        return visitor(law, entry);
      }),
    );
    for (const r of batchResults) {
      if (r.status === 'fulfilled' && r.value) {
        if (Array.isArray(r.value)) results.push(...r.value);
        else results.push(r.value);
      }
    }
  }
  return results;
}

/**
 * Laws whose articles implement open_terms of (lawId[, targetArticle]).
 * Returns [{law_id, article, target_article, open_term, gelet_op}].
 */
export async function discoverImplementors(lawId, allLaws, fetchLawYaml, targetArticle) {
  const candidates = allLaws.filter((e) => e.law_id !== lawId);
  return scanCorpus(candidates, fetchLawYaml, (law, entry) => {
    const hits = [];
    for (const article of law.articles || []) {
      const impls = article.machine_readable?.implements || [];
      for (const impl of impls) {
        if (impl.law !== lawId) continue;
        if (targetArticle && String(impl.article) !== String(targetArticle)) continue;
        hits.push({
          law_id: law.$id || entry.law_id,
          article: String(article.number),
          target_article: impl.article != null ? String(impl.article) : undefined,
          open_term: impl.open_term,
          gelet_op: impl.gelet_op,
        });
      }
    }
    return hits.length ? hits : null;
  });
}

/**
 * Laws whose articles override outputs of (lawId[, targetArticle[, targetOutput]]).
 * Returns [{law_id, article, target_article, target_output}].
 */
export async function discoverOverriders(lawId, allLaws, fetchLawYaml, targetArticle, targetOutput) {
  const candidates = allLaws.filter((e) => e.law_id !== lawId);
  return scanCorpus(candidates, fetchLawYaml, (law, entry) => {
    const hits = [];
    for (const article of law.articles || []) {
      const ovs = article.machine_readable?.overrides || [];
      for (const ov of ovs) {
        if (ov.law !== lawId) continue;
        if (targetArticle && String(ov.article) !== String(targetArticle)) continue;
        if (targetOutput && ov.output !== targetOutput) continue;
        hits.push({
          law_id: law.$id || entry.law_id,
          article: String(article.number),
          target_article: ov.article != null ? String(ov.article) : undefined,
          target_output: ov.output,
        });
      }
    }
    return hits.length ? hits : null;
  });
}

/**
 * Laws whose articles source outputs from (lawId[, targetOutput]).
 * Returns [{law_id, article, input_name, target_output}].
 */
export async function discoverUsers(lawId, allLaws, fetchLawYaml, targetOutput) {
  const candidates = allLaws.filter((e) => e.law_id !== lawId);
  return scanCorpus(candidates, fetchLawYaml, (law, entry) => {
    const hits = [];
    for (const article of law.articles || []) {
      const inputs = article.machine_readable?.execution?.input || [];
      for (const input of inputs) {
        const reg = input.source?.regulation;
        if (reg !== lawId) continue;
        if (targetOutput && input.source?.output !== targetOutput) continue;
        hits.push({
          law_id: law.$id || entry.law_id,
          article: String(article.number),
          input_name: input.name,
          target_output: input.source?.output,
        });
      }
    }
    return hits.length ? hits : null;
  });
}

// ---------- Recursive loader composable (unchanged behaviour) ----------

export function useDependencies() {
  const loading = ref(false);
  const loadedDeps = ref([]);
  const progress = ref('');
  const error = ref(null);

  async function loadAllDependencies(lawYamlText, engine, fetchLawYaml) {
    loading.value = true;
    error.value = null;
    loadedDeps.value = [];
    progress.value = 'Afhankelijkheden analyseren...';

    try {
      const mainLaw = yaml.load(lawYamlText);
      const visited = new Set();
      const toLoad = [];

      collectDeps(mainLaw, visited, toLoad);

      try {
        const corpusRes = await fetch('/api/corpus/laws?limit=1000');
        if (corpusRes.ok) {
          const allLaws = await corpusRes.json();
          const implementors = await discoverImplementors(mainLaw.$id, allLaws, fetchLawYaml);
          for (const hit of implementors) {
            if (!visited.has(hit.law_id)) {
              visited.add(hit.law_id);
              toLoad.push(hit.law_id);
            }
          }
        }
      } catch {
        // Corpus scan is best-effort
      }

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

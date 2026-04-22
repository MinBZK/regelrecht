// Shared engine-singleton composable.
// The WASM-engine is loaded once and memoised in a module-local promise.
// Data-sources are re-registered every evaluation (engine is stateful).

import { ref, shallowRef } from 'vue';

let enginePromise = null;
let demoIndexPromise = null;
const profileCache = new Map();
const lawYamlCache = new Map();

async function loadEngineModule() {
  // /wasm/pkg/ is served from public/ at runtime — not a module that Vite can
  // resolve at build time. Fetch the glue, wrap in a Blob, dynamic-import that
  // with @vite-ignore so rolldown leaves the reference alone.
  const jsRes = await fetch('/wasm/pkg/regelrecht_engine.js');
  if (!jsRes.ok) throw new Error(`WASM glue fetch failed: ${jsRes.status}`);
  const jsText = await jsRes.text();
  const blob = new Blob([jsText], { type: 'application/javascript' });
  const blobUrl = URL.createObjectURL(blob);
  const mod = await import(/* @vite-ignore */ blobUrl);
  URL.revokeObjectURL(blobUrl);
  await mod.default('/wasm/pkg/regelrecht_engine_bg.wasm');
  return mod;
}

export function useEngine() {
  const loading = ref(false);
  const error = ref(null);
  const lastResult = shallowRef(null);

  async function getEngine() {
    if (!enginePromise) {
      enginePromise = loadEngineModule().then((mod) => new mod.WasmEngine());
    }
    return enginePromise;
  }

  async function getDemoIndex() {
    if (!demoIndexPromise) {
      demoIndexPromise = fetch('/demo-assets/demo-index.json').then((r) => {
        if (!r.ok) throw new Error(`demo-index fetch failed: ${r.status}`);
        return r.json();
      });
    }
    return demoIndexPromise;
  }

  async function getProfile(name) {
    if (profileCache.has(name)) return profileCache.get(name);
    const p = fetch(`/demo-assets/profiles/${name}.json`).then((r) => {
      if (!r.ok) throw new Error(`profile ${name} fetch failed: ${r.status}`);
      return r.json();
    });
    profileCache.set(name, p);
    return p;
  }

  async function loadLaw(engine, lawPath) {
    const basename = lawPath.split('/').pop();
    let yaml = lawYamlCache.get(basename);
    if (!yaml) {
      const resp = await fetch(`/demo-assets/laws/${basename}`);
      if (!resp.ok) throw new Error(`law fetch failed: ${resp.status}`);
      yaml = await resp.text();
      lawYamlCache.set(basename, yaml);
    }
    if (!engine.hasLaw || !engine.hasLaw(deriveLawId(yaml))) {
      engine.loadLaw(yaml);
    }
  }

  function deriveLawId(yaml) {
    // quick-and-dirty $id extractor so we don't parse the full YAML client-side
    const match = yaml.match(/^\$id:\s*(\S+)/m);
    return match ? match[1] : '';
  }

  function registerProfile(engine, profile) {
    engine.clearDataSources();
    for (const ds of profile.data_sources || []) {
      engine.registerDataSource(ds.table, ds.key, ds.records);
    }
  }

  async function evaluate({ lawEntry, profile, parameters, calculationDate }) {
    loading.value = true;
    error.value = null;
    try {
      const engine = await getEngine();
      await loadLaw(engine, lawEntry.path);
      registerProfile(engine, profile);
      const result = engine.executeWithTrace(
        lawEntry.id,
        lawEntry.output,
        parameters ?? { bsn: profile.bsn },
        calculationDate,
      );
      lastResult.value = result;
      return result;
    } catch (e) {
      error.value = e?.message || String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return {
    loading,
    error,
    lastResult,
    getDemoIndex,
    getProfile,
    evaluate,
  };
}

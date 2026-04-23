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
  await mod.default({ module_or_path: '/wasm/pkg/regelrecht_engine_bg.wasm' });
  return mod;
}

function formatEngineError(e) {
  if (!e) return 'Onbekende fout';
  if (typeof e === 'string') return e;
  if (e.message) return e.message;
  if (e.error) return e.error;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
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
    let yaml = lawYamlCache.get(lawPath);
    if (!yaml) {
      const resp = await fetch(`/demo-assets/laws/${lawPath}`);
      if (!resp.ok) throw new Error(`law fetch failed (${lawPath}): ${resp.status}`);
      yaml = await resp.text();
      lawYamlCache.set(lawPath, yaml);
    }
    const id = deriveLawId(yaml);
    if (!id) {
      throw new Error(
        `law YAML at ${lawPath} has no $id; engine cannot register it (check the bundling step)`,
      );
    }
    if (!engine.hasLaw || !engine.hasLaw(id)) {
      engine.loadLaw(yaml);
    }
  }

  async function loadLawWithDependencies(engine, lawEntry) {
    for (const dep of lawEntry.dependencies || []) {
      await loadLaw(engine, dep);
    }
    await loadLaw(engine, lawEntry.path);
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
      await loadLawWithDependencies(engine, lawEntry);
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
      error.value = formatEngineError(e);
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

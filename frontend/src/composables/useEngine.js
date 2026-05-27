/**
 * useEngine — singleton WasmEngine instance with lazy initialization.
 *
 * Provides the engine and helpers for loading laws and dependencies.
 */
import { ref } from 'vue';
import { lawUrl } from './corpusUrls.js';

let engineInstance = null;
let initPromise = null;

// Per-law tracking of which traject scope a YAML was loaded under. The
// WASM engine itself only knows law id, so without this every scope
// would see whichever copy happened to be loaded first — switching
// trajects then runs scenarios against a stale dependency. Keyed by
// `lawId`, value is the `trajectRef || ''` the load used.
//
// LRU-capped. In practice `unloadAllLaws()` already clears the map on
// every traject switch, so the map only grows within one traject's
// session. The cap is a safety net for any future code path that
// loads a dependency without going through `unloadAllLaws` on scope
// change. Cap mirrors `lawCache`'s order of magnitude (`useLaw.js`).
const LOADED_SCOPES_MAX = 50;
const loadedScopes = new Map();

function touchLoadedScopes(lawId) {
  if (loadedScopes.has(lawId)) {
    const v = loadedScopes.get(lawId);
    loadedScopes.delete(lawId);
    loadedScopes.set(lawId, v);
  }
  while (loadedScopes.size > LOADED_SCOPES_MAX) {
    const oldest = loadedScopes.keys().next().value;
    loadedScopes.delete(oldest);
  }
}

const ready = ref(false);
const initError = ref(null);

async function initEngine() {
  if (engineInstance) return engineInstance;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    try {
      // WASM pkg lives in public/wasm/pkg/ and is served as a static asset.
      // Use fetch + blob URL to load the JS glue, avoiding Vite's restriction
      // on importing files from /public in source code.
      const jsRes = await fetch('/wasm/pkg/regelrecht_engine.js');
      if (!jsRes.ok) throw new Error(`Failed to fetch WASM JS glue: ${jsRes.status}`);
      const jsText = await jsRes.text();
      const blob = new Blob([jsText], { type: 'application/javascript' });
      const blobUrl = URL.createObjectURL(blob);
      const wasm = await import(/* @vite-ignore */ blobUrl);
      URL.revokeObjectURL(blobUrl);
      await wasm.default('/wasm/pkg/regelrecht_engine_bg.wasm');
      engineInstance = new wasm.WasmEngine();
      ready.value = true;
      return engineInstance;
    } catch (e) {
      initError.value = e;
      throw e;
    }
  })();

  return initPromise;
}

/**
 * Fetch law YAML from the API and load it into the engine. When
 * `trajectRef` is given the read goes through the traject's per-source
 * backends (read-your-writes for in-progress edits); omit it for the
 * global view.
 *
 * If the law was previously loaded under a *different* scope, unload
 * the stale copy first — otherwise scenario runs after a traject
 * switch would evaluate against the previous scope's dependencies.
 */
async function loadDependency(lawId, trajectRef = null) {
  const engine = await initEngine();
  const scope = trajectRef || '';
  if (engine.hasLaw(lawId)) {
    if (loadedScopes.get(lawId) === scope) return;
    engine.unloadLaw(lawId);
  }

  const res = await fetch(lawUrl(trajectRef, lawId));
  if (!res.ok) throw new Error(`Failed to fetch law '${lawId}': ${res.status}`);
  const yaml = await res.text();
  engine.loadLaw(yaml);
  loadedScopes.set(lawId, scope);
  touchLoadedScopes(lawId);
}

/**
 * Load a law YAML string into the engine and remember which scope it
 * came from. Callers that pass a raw YAML body (i.e. an edited law
 * being saved through the editor) should pass the scope they fetched
 * it under so a later `loadDependency` call doesn't think a copy from
 * another scope is still valid.
 *
 * Re-loads (same `lawId`) unload the previous copy first so the
 * engine sees the new YAML — `engine.loadLaw` on a known id is a no-op
 * in the WASM binding.
 */
async function loadLawYaml(yaml, lawId = null, trajectRef = null) {
  const engine = await initEngine();
  if (lawId && engine.hasLaw(lawId)) engine.unloadLaw(lawId);
  const result = engine.loadLaw(yaml);
  if (lawId) {
    loadedScopes.set(lawId, trajectRef || '');
    touchLoadedScopes(lawId);
  }
  return result;
}

/**
 * Unload a law from the engine and forget its scope. Used when a
 * traject switch needs to flush a known dependency so the next
 * resolver call re-fetches from the new scope.
 */
function unloadLaw(lawId) {
  if (!engineInstance) return;
  if (engineInstance.hasLaw(lawId)) engineInstance.unloadLaw(lawId);
  loadedScopes.delete(lawId);
}

/**
 * Unload every law the engine has tracked. Cheaper than rebuilding
 * the engine on every traject switch in the editor, and good enough
 * because the dependency walker re-loads on demand.
 */
function unloadAllLaws() {
  if (!engineInstance) {
    loadedScopes.clear();
    return;
  }
  for (const lawId of loadedScopes.keys()) {
    if (engineInstance.hasLaw(lawId)) engineInstance.unloadLaw(lawId);
  }
  loadedScopes.clear();
}

export function useEngine() {
  return {
    /** Ref<boolean> — true when the WASM engine is ready */
    ready,
    /** Ref<Error|null> — set if init failed */
    initError,
    /** Initialize and return the engine instance */
    initEngine,
    /** Load a dependent law by ID via the API */
    loadDependency,
    /** Load raw YAML into the engine */
    loadLawYaml,
    /** Unload a single law from the engine */
    unloadLaw,
    /** Unload every tracked law — used on traject switch to flush stale deps */
    unloadAllLaws,
    /** Get the engine instance (must be initialized first) */
    getEngine: () => engineInstance,
  };
}

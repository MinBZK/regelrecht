/**
 * useEngine — singleton WasmEngine instance with lazy initialization.
 *
 * Provides the engine and helpers for loading laws and dependencies.
 */
import { ref } from 'vue';
import { lawVersionsUrl } from './corpusUrls.js';
import { apiFetchJson, apiFetchText } from '../lib/apiFetch.js';
import { createLruMap } from '../lib/lruMap.js';

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
const loadedScopes = createLruMap(50, {
  onEvict: (lawId) => {
    // Drop the WASM-side copy alongside the tracking entry — without
    // this the engine retains the law in memory even though our
    // scope-tracking forgot it, defeating the whole "safety net" of
    // the cap. `hasLaw` guards an unloadLaw call that the engine
    // would otherwise panic on if the law is already gone.
    if (engineInstance?.hasLaw(lawId)) {
      engineInstance.unloadLaw(lawId);
    }
  },
});

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
      const jsText = await apiFetchText('/wasm/pkg/regelrecht_engine.js', {
        errorMessage: (status) => `Failed to fetch WASM JS glue: ${status}`,
      });
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
 * Fetch a law's YAML from the API and load it into the engine. When
 * `trajectRef` is given the read goes through the traject's per-source
 * backends (read-your-writes for in-progress edits); omit it for the
 * global view.
 *
 * Loads **every** version of the law (not just today's-best) so the
 * engine's date-aware resolution can pick the one in force on a given
 * calculation date — the same contract the scenario dependency walker
 * uses. This keeps a single "load a law into the engine" semantic: any
 * caller (notes, gherkin steps) that brings a law into the shared engine
 * brings its full version set, so the `engine.hasLaw` skip guard never
 * leaves a law with only one (possibly future-dated) version loaded.
 *
 * If the law was previously loaded under a *different* scope, unload the
 * stale copy first (`unloadLaw` drops all of its versions) — otherwise
 * scenario runs after a traject switch would evaluate against the
 * previous scope's dependencies.
 */
async function loadDependency(lawId, trajectRef = null) {
  const engine = await initEngine();
  const scope = trajectRef || '';
  if (engine.hasLaw(lawId)) {
    if (loadedScopes.get(lawId) === scope) return;
    engine.unloadLaw(lawId);
  }

  const yamls = await apiFetchJson(lawVersionsUrl(trajectRef, lawId), {
    errorMessage: (status) => `Failed to fetch versions of law '${lawId}': ${status}`,
  });
  const versions = Array.isArray(yamls) ? yamls : [];
  for (const versionYaml of versions) engine.loadLaw(versionYaml);
  // Only record the scope once a body is actually loaded, so an empty
  // result (unknown law) stays retryable rather than masking the law as
  // "loaded under this scope".
  if (versions.length > 0) loadedScopes.set(lawId, scope);
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

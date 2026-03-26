/**
 * useEngine — singleton WasmEngine instance with lazy initialization.
 *
 * Provides the engine and helpers for loading laws and dependencies.
 */
import { ref } from 'vue';

let engineInstance = null;
let initPromise = null;

const ready = ref(false);
const initError = ref(null);

async function initEngine() {
  if (engineInstance) return engineInstance;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    try {
      const wasm = await import('../wasm/pkg/regelrecht_engine.js');
      await wasm.default();
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
 * Fetch law YAML from the API and load it into the engine.
 */
async function loadDependency(lawId) {
  const engine = await initEngine();
  if (engine.hasLaw(lawId)) return;

  const res = await fetch(`/api/corpus/laws/${encodeURIComponent(lawId)}`);
  if (!res.ok) throw new Error(`Failed to fetch law '${lawId}': ${res.status}`);
  const yaml = await res.text();
  engine.loadLaw(yaml);
}

/**
 * Load a law YAML string into the engine.
 */
async function loadLawYaml(yaml) {
  const engine = await initEngine();
  return engine.loadLaw(yaml);
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
    /** Get the engine instance (must be initialized first) */
    getEngine: () => engineInstance,
  };
}

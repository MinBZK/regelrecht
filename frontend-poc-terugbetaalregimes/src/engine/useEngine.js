/**
 * useEngine - singleton WasmEngine met lazy initialisatie.
 *
 * Browserpad: haalt de JS-glue op en importeert die via een blob-URL
 * (Vite staat imports uit /public niet toe), zoals de regelrecht-editor doet.
 * Laadt daarna alle wetten uit /laws/index.json in de engine.
 */
import { ref } from 'vue';
import { b } from '../basePad.js';

let engineInstance = null;
let initPromise = null;
let wasmModule = null;

const ready = ref(false);
const initError = ref(null);
const lawIndex = ref([]);

async function fetchText(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Ophalen mislukt (${res.status}): ${url}`);
  return res.text();
}

async function initEngine() {
  if (engineInstance) return engineInstance;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    try {
      const jsText = await fetchText(b('/wasm/pkg/regelrecht_engine.js'));
      const blob = new Blob([jsText], { type: 'application/javascript' });
      const blobUrl = URL.createObjectURL(blob);
      const wasm = await import(/* @vite-ignore */ blobUrl);
      URL.revokeObjectURL(blobUrl);
      await wasm.default(b('/wasm/pkg/regelrecht_engine_bg.wasm'));
      wasmModule = wasm;
      engineInstance = new wasm.WasmEngine();

      const index = JSON.parse(await fetchText(b('/laws/index.json')));
      for (const entry of index) {
        // `entry.path` komt uit het manifest dat copy-assets.js schrijft; dat
        // is relatief, dus ook die gaat langs de basis.
        const yamlText = await fetchText(b(entry.path));
        engineInstance.loadLaw(yamlText);
      }
      lawIndex.value = index;

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
 * Extra, lege engine-instantie (na initEngine). Voor kolommen naast elkaar
 * (huidig recht én een variant op de hoofdthread) heeft elke wetten-set een
 * eigen engine nodig; de aanroeper laadt de YAML's zelf.
 */
async function createEngine() {
  await initEngine();
  return new wasmModule.WasmEngine();
}

export function useEngine() {
  return {
    ready,
    initError,
    lawIndex,
    initEngine,
    createEngine,
    getEngine: () => engineInstance,
  };
}

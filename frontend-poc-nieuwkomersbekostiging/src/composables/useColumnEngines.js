/**
 * useColumnEngines - één WasmEngine per kolom (huidig recht of een variant)
 * op de hoofdthread, voor de casus- en schoolschermen die twee wetten-sets
 * naast elkaar tonen. De engines worden herladen zodra de lawStore-versie
 * verandert (parameter-edit, YAML-edit, variant), zodat ze exact de
 * regelgeving draaien die de beleid-view toont.
 */
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';

const { createEngine } = useEngine();
const { lawYamlsFor, version } = useLawStore();

const engines = new Map(); // key -> { engine, version, promise }

function keyFor(variantId) {
  return variantId ?? '__ist__';
}

/** Engine met de wetten van de kolom geladen; hergebruikt zolang de versie gelijk blijft. */
async function engineFor(variantId = null) {
  const key = keyFor(variantId);
  const current = engines.get(key);
  if (current && current.version === version.value) return current.promise;

  const promise = (async () => {
    const engine = current?.engine ?? (await createEngine());
    for (const lawId of engine.listLaws()) engine.unloadLaw(lawId);
    const laws = await lawYamlsFor(variantId);
    for (const text of laws) engine.loadLaw(text);
    return engine;
  })();
  engines.set(key, { engine: current?.engine ?? null, version: version.value, promise });
  const engine = await promise;
  engines.set(key, { engine, version: version.value, promise });
  return engine;
}

export function useColumnEngines() {
  return { engineFor };
}

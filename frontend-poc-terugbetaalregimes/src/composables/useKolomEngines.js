/**
 * useKolomEngines - één WasmEngine per kolom (huidig recht of een variant) op
 * de hoofdthread, voor de schermen die persona's door meerdere wetsversies
 * halen. De populatiesimulatie draait in workers; hier gaat het om een
 * handjevol persona's, en dan is een engine per kolom eenvoudiger en direct
 * doorklikbaar naar de trace.
 *
 * Engines worden herladen zodra de lawStore-versie verandert (parameter-edit,
 * YAML-edit, andere werkversie), zodat ze exact de regelgeving draaien die de
 * beleidsview toont.
 */
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';
import { KOLOM_BASIS } from './usePopulation.js';

const { createEngine } = useEngine();
const { lawYamlsFor, baseLawYamls, version, werkversie } = useLawStore();

const engines = new Map(); // kolomkey -> { engine, version, promise }

/** De wetten van een kolom; huidig recht is de onbewerkte basis uit main. */
async function lawsForColumn(col) {
  if (col.key === KOLOM_BASIS) {
    // Is huidig recht de werkversie, dan hoort die kolom de bewerkingen te tonen.
    return werkversie.value === null ? lawYamlsFor(null) : baseLawYamls();
  }
  return lawYamlsFor(col.variantId);
}

/** Engine met de wetten van de kolom geladen; hergebruikt zolang de versie gelijk blijft. */
async function engineFor(col) {
  const key = col.key;
  const current = engines.get(key);
  if (current && current.version === version.value) return current.promise;

  const promise = (async () => {
    const engine = current?.engine ?? (await createEngine());
    for (const lawId of engine.listLaws?.() ?? []) engine.unloadLaw(lawId);
    const laws = await lawsForColumn(col);
    for (const text of laws) engine.loadLaw(text);
    return engine;
  })();
  engines.set(key, { engine: current?.engine ?? null, version: version.value, promise });
  const engine = await promise;
  engines.set(key, { engine, version: version.value, promise });
  return engine;
}

/** Engines van kolommen die niet meer getoond worden, opruimen. */
function pruneEngines(actieveKeys) {
  for (const key of [...engines.keys()]) {
    if (!actieveKeys.includes(key)) engines.delete(key);
  }
}

export function useKolomEngines() {
  return { engineFor, pruneEngines };
}

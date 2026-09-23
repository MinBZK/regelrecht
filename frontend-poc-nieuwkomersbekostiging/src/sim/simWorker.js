/**
 * Web worker voor populatiesimulaties.
 *
 * Bezit een eigen WasmEngine: de hoofdthread stuurt de YAML-teksten van de
 * kolom (ist of variant, inclusief bewerkte overlays) mee, zodat de worker
 * exact dezelfde regelgeving draait als het scherm toont. Er draait één
 * worker per kolom, parallel.
 *
 * Berichten in:  { type: 'simulate', laws: string[], records: object[], options: { jaren?, peildata? } }
 * Berichten uit: { type: 'progress', done, total }
 *                { type: 'result', sim }          (resultaat van simulate(), zie simulate.js)
 *                { type: 'error', message }
 */
import { simulate } from './simulate.js';
import { b } from '../basePad.js';

let enginePromise = null;

async function getEngine() {
  if (!enginePromise) {
    enginePromise = (async () => {
      // Tegen self.location, niet self.location.origin: dat laatste gooit het pad
      // weg, en achter het poc-portaal staat de app onder /<slug>/.
      const jsUrl = new URL(b('/wasm/pkg/regelrecht_engine.js'), self.location);
      const wasm = await import(/* @vite-ignore */ jsUrl.href);
      await wasm.default({
        module_or_path: new URL(b('/wasm/pkg/regelrecht_engine_bg.wasm'), self.location).href,
      });
      return new wasm.WasmEngine();
    })();
  }
  return enginePromise;
}

self.onmessage = async (event) => {
  const { type } = event.data;
  if (type !== 'simulate') return;

  try {
    const { laws, records, options } = event.data;
    const engine = await getEngine();

    // Herlaad de volledige set wetten zodat overlays/varianten meegenomen worden.
    for (const lawId of engine.listLaws()) engine.unloadLaw(lawId);
    for (const yamlText of laws) engine.loadLaw(yamlText);

    let lastReport = 0;
    const sim = simulate(engine, records, {
      ...(options ?? {}),
      onProgress: (done, total) => {
        const now = Date.now();
        if (now - lastReport < 100 && done < total) return;
        lastReport = now;
        self.postMessage({ type: 'progress', done, total });
      },
    });
    self.postMessage({ type: 'result', sim });
  } catch (e) {
    self.postMessage({ type: 'error', message: String(e?.message ?? e) });
  }
};

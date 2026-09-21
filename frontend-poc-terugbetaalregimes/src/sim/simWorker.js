/**
 * Web worker voor populatiesimulaties.
 *
 * Bezit een eigen WasmEngine: de hoofdthread stuurt de actuele YAML-teksten
 * (inclusief bewerkte overlays uit de beleid-view) mee, zodat de worker
 * exact dezelfde regelgeving draait als het scherm toont.
 *
 * Berichten in:  { type: 'simulate', laws: string[], records: object[],
 *                  choicesOverride: object|null, options: object }
 * Berichten uit: { type: 'progress', done, total }
 *                { type: 'result', results: [{ record, totals }] }
 *                { type: 'error', message }
 */
import { simulate } from './simulate.js';
import { b } from '../basePad.js';

let enginePromise = null;

async function getEngine() {
  if (!enginePromise) {
    enginePromise = (async () => {
      // Tegen self.location, niet self.location.origin: dat laatste gooit het
      // pad weg, en achter het poc-portaal staat de app onder /<slug>/. Vite
      // vertaalt import.meta.env.BASE_URL ook in een worker-bundle.
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
    const { laws, records, choicesOverride, options } = event.data;
    const engine = await getEngine();

    // Herlaad de volledige set wetten zodat overlays uit de beleid-view
    // meegenomen worden.
    for (const lawId of engine.listLaws()) engine.unloadLaw(lawId);
    for (const yamlText of laws) engine.loadLaw(yamlText);

    const results = [];
    const total = records.length;
    const progressEvery = Math.max(1, Math.floor(total / 50));

    for (let i = 0; i < total; i++) {
      const record = records[i];
      const choices = { ...record.keuzes, ...(choicesOverride ?? {}) };
      const { totals } = simulate(engine, record, choices, options ?? {});
      results.push({ record, totals });
      // De engine cachet resolutie per geregistreerde databron-sleutel (bsn).
      // Zonder deze reset groeit die cache lineair met het aantal records en
      // wordt elke volgende simulatie trager; leegmaken houdt de doorlooptijd
      // per record constant.
      engine.clearDataSources();
      if ((i + 1) % progressEvery === 0) {
        self.postMessage({ type: 'progress', done: i + 1, total });
      }
    }

    self.postMessage({ type: 'result', results });
  } catch (e) {
    self.postMessage({ type: 'error', message: String(e?.message ?? e) });
  }
};

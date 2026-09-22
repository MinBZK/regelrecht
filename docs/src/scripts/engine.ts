/*
 * Fetching the engine and the laws, once per page.
 *
 * Two panels on this site execute laws in the visitor's browser: the landing
 * page's run panel (~/scripts/landing-run.ts) and the scenario runner on
 * /concepts/scenarios (~/scripts/scenario-playground.ts). They ask the engine
 * different questions and draw different things, but they load exactly the same
 * WASM module and the same set of laws, so that lives here and the promise is
 * shared. A visitor who opens the runner after the landing panel has already
 * fetched the engine waits for nothing.
 *
 * What is loaded comes out of the corpus, assembled by script/landing-laws.sh
 * into public/laws at build time. All versions of each law are loaded, not one
 * picked here: the engine holds them side by side and selects on the
 * calculation date, which is what makes the result the same as the one CI
 * produces.
 */

export interface Manifest {
  calculationDate: string;
  scenario: string;
  laws: string[];
}

export interface Loaded {
  engine: any;
  manifest: Manifest;
}

let enginePromise: Promise<Loaded> | null = null;
let featurePromise: Promise<string> | null = null;

/**
 * Load the engine and the laws, at most once.
 *
 * Separate from running so a page can start the download while the visitor is
 * still reading: by the time they press a button, the fetch has usually
 * finished.
 */
export function prepare(base = '/'): Promise<Loaded> {
  if (!enginePromise) {
    enginePromise = load(base).catch((err) => {
      // Never cache a failure: a flaky network should not leave a panel
      // permanently unable to run.
      enginePromise = null;
      throw err;
    });
  }
  return enginePromise;
}

async function load(base: string): Promise<Loaded> {
  const manifest: Manifest = await fetch(`${base}laws/manifest.json`).then((r) => {
    if (!r.ok) throw new Error(`manifest: ${r.status}`);
    return r.json();
  });

  const wasm = await import(/* @vite-ignore */ `${base}wasm/pkg/regelrecht_engine.js`);
  await wasm.default(`${base}wasm/pkg/regelrecht_engine_bg.wasm`);

  const engine = new wasm.WasmEngine();

  const laws = await Promise.all(
    manifest.laws.map((p) =>
      fetch(`${base}${p}`).then((r) => {
        if (!r.ok) throw new Error(`${p}: ${r.status}`);
        return r.text();
      }),
    ),
  );

  for (const yaml of laws) engine.loadLaw(yaml);

  return { engine, manifest };
}

/**
 * The scenario file from the corpus, for the caller that runs it as it stands.
 *
 * Separate from `prepare`, and fetched only on demand, because only one of the
 * two panels wants it. The landing page executes the corpus scenario itself; the
 * runner on /concepts/scenarios executes whatever is in its editor and ships its
 * own copy of the text in the page. Loading it inside `prepare` cost every
 * visitor to that page a 26 KB fetch whose result was dropped on the floor.
 *
 * Callers that want both should ask for them together, so the fetch still runs
 * beside the engine's rather than after it.
 */
export function corpusScenario(base = '/'): Promise<string> {
  if (!featurePromise) {
    featurePromise = prepare(base)
      .then(({ manifest }) =>
        fetch(`${base}laws/${manifest.scenario}`).then((r) => {
          if (!r.ok) throw new Error(`scenario: ${r.status}`);
          return r.text();
        }),
      )
      .catch((err) => {
        featurePromise = null;
        throw err;
      });
  }
  return featurePromise;
}

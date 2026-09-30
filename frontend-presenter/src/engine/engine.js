/**
 * The engine in the browser: the WASM that `just wasm-build` makes, served by
 * the dev server under /wasm/pkg (server/presenterApi.js).
 *
 * One engine per deck, because a deck can replace a corpus law with its own
 * variant, and that variant must not leak into another deck's slides. Laws
 * are loaded on demand, every version of each, and runs on one engine go one
 * at a time: a run clears the data sources and registers its own, so two
 * interleaved runs would read each other's case.
 */
import * as yaml from 'js-yaml';

let wasmPromise = null;

/** The wasm-bindgen module, initialised once. */
function loadWasm() {
  wasmPromise ??= (async () => {
    // Vite does not import from /public-style paths, so the glue comes in as
    // text and is imported from a blob URL (as frontend-demo does).
    const res = await fetch('/wasm/pkg/regelrecht_engine.js');
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body.error ?? `De engine kon niet geladen worden (${res.status})`);
    }
    // The binary is fetched here rather than by the glue, so a missing .wasm
    // next to a present .js gets the same "build it first" message.
    const bin = await fetch('/wasm/pkg/regelrecht_engine_bg.wasm');
    if (!bin.ok) {
      const body = await bin.json().catch(() => ({}));
      throw new Error(body.error ?? `De engine kon niet geladen worden (${bin.status})`);
    }
    const url = URL.createObjectURL(new Blob([await res.text()], { type: 'text/javascript' }));
    try {
      const wasm = await import(/* @vite-ignore */ url);
      await wasm.default({ module_or_path: bin });
      return wasm;
    } finally {
      URL.revokeObjectURL(url);
    }
  })();
  wasmPromise.catch(() => (wasmPromise = null));
  return wasmPromise;
}

const engines = new Map();

/**
 * The engine of a deck: `{ engine, loadLaw, units, failures, exclusive }`.
 * - At the start every law of the main corpus is loaded, in every version,
 *   then the deck's own law YAMLs: what the Rust BDD runner does, so a law
 *   the scenario does not name (one that implements or overrides the law
 *   under test) is there all the same.
 * - `loadLaw(id)` loads a law from outside the main corpus when a scenario
 *   names one (corpus-poc, PRESENTER_CORPUS).
 * - `failures` lists the versions the engine refused, with their law id.
 * - `units` maps `law/output` to the unit its YAML declares, for display.
 * - `exclusive(fn)` runs fn when no other run on this engine is busy.
 */
export function deckEngine(deck) {
  if (!engines.has(deck)) {
    const p = create(deck);
    // A failed start (engine not built yet) is retried on the next run.
    p.catch(() => engines.delete(deck));
    engines.set(deck, p);
  }
  return engines.get(deck);
}

/** Forget a deck's engine, so the next run reloads its laws (after a change on disk). */
export function resetDeckEngine(deck) {
  // Not freed: a run may still be busy on the old engine. It is garbage once
  // that run ends; a reset happens only when a file changes, so the leak of an
  // unfreed wasm-bindgen object per change is small.
  engines.delete(deck);
}

async function create(deck) {
  const wasm = await loadWasm();
  const engine = new wasm.WasmEngine();
  const units = new Map();
  const failures = [];

  const load = (versions) => {
    for (const v of versions) {
      try {
        engine.loadLaw(v.text);
        collectUnits(units, v.text);
      } catch (e) {
        failures.push({ law: v.law, source: v.source, message: String(typeof e === 'string' ? e : e?.message ?? e) });
      }
    }
  };
  const fetchJson = async (url) => {
    const res = await fetch(url);
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body.error ?? `${url}: ${res.status}`);
    return body;
  };

  load((await fetchJson(`/api/laws?${new URLSearchParams({ deck })}`)).versions);

  // A failed fetch is not remembered: the next run tries again.
  const loading = new Map();
  function loadLaw(id) {
    if (!loading.has(id)) {
      const p = fetchJson(`/api/laws/${encodeURIComponent(id)}?${new URLSearchParams({ deck })}`).then((b) => load(b.versions));
      p.catch(() => loading.delete(id));
      loading.set(id, p);
    }
    return loading.get(id);
  }

  let queue = Promise.resolve();
  const exclusive = (fn) => {
    const next = queue.then(fn, fn);
    queue = next.catch(() => {});
    return next;
  };

  return { engine, loadLaw, units, failures, exclusive };
}

/** Record the declared unit of every output in a law YAML, as `law/output`. */
export function collectUnits(units, text) {
  let doc;
  try {
    doc = yaml.load(text);
  } catch {
    return;
  }
  for (const article of doc?.articles ?? []) {
    for (const o of article?.machine_readable?.execution?.output ?? []) {
      const unit = o?.type_spec?.unit;
      if (unit) units.set(`${doc.$id}/${o.name}`, unit);
    }
  }
}

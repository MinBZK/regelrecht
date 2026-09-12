/**
 * The demo's engine: the regelrecht WASM engine with every demo law loaded,
 * persona data registered as law-scoped sources, and citizen corrections
 * (claims) layered on top at a higher priority.
 *
 * Singleton: the whole workspace shares one engine instance.
 */
import { ref } from 'vue';
import {
  collectKeyValues,
  lawShape,
  materialiseAll,
  tablesFromProfiles,
} from '../data/materialize.js';

let engineInstance = null;
let initPromise = null;
/** The wasm-bindgen module once initialised; a second engine is constructed from it. */
let wasmModule = null;
export const engineReady = ref(false);
export const engineError = ref(null);

const REGISTER_PRIORITY = 10;
const CLAIMS_PRIORITY = 100;
export const CLAIMS_SOURCE = 'correcties';

async function initEngine() {
  if (engineInstance) return engineInstance;
  if (initPromise) return initPromise;
  initPromise = (async () => {
    try {
      // The WASM glue lives in public/wasm/pkg; import it through a blob URL to
      // sidestep Vite's rule against importing from /public (same as the editor).
      const res = await fetch('/wasm/pkg/regelrecht_engine.js');
      if (!res.ok) throw new Error(`WASM-glue niet gevonden (${res.status})`);
      const blob = new Blob([await res.text()], { type: 'application/javascript' });
      const blobUrl = URL.createObjectURL(blob);
      const wasm = await import(/* @vite-ignore */ blobUrl);
      URL.revokeObjectURL(blobUrl);
      await wasm.default('/wasm/pkg/regelrecht_engine_bg.wasm');
      wasmModule = wasm;
      engineInstance = new wasm.WasmEngine();
      engineReady.value = true;
      return engineInstance;
    } catch (e) {
      engineError.value = e;
      throw e;
    }
  })();
  return initPromise;
}

/**
 * Laws the engine refused to load: `{id, path, message}` per law version. The
 * loader type-checks every law (RFC-037), so a refused law is a law with a
 * finding, and it is simply not there for the portal, the graph and the
 * scenario runner. That must be visible, not a line in the console: the views
 * show these in a banner and the scenario runner names the refusal instead of
 * "law not found".
 */
export const loadFailures = ref([]);

function loadLaws(engine, corpus) {
  const failures = [];
  for (const law of corpus.laws) {
    try {
      engine.loadLaw(law.text);
    } catch (e) {
      const message = typeof e === 'string' ? e : e?.error ?? e?.message ?? JSON.stringify(e);
      console.warn(`Wet ${law.id} (${law.path}) kon niet geladen worden:`, e);
      failures.push({ id: law.id, path: law.path, message: String(message) });
    }
  }
  // Both engines load the same corpus; the second run says nothing new.
  if (failures.length || loadFailures.value.length === 0) loadFailures.value = failures;
}

/** The load failure of a law by id, or null when the engine holds it. */
export function loadFailureFor(lawId) {
  return loadFailures.value.find((f) => f.id === lawId) ?? null;
}

/** Load every law version of the corpus into the engine (idempotent). */
let lawsLoaded = false;
export async function prepareEngine(corpus) {
  const engine = await initEngine();
  if (!lawsLoaded) {
    loadLaws(engine, corpus);
    lawsLoaded = true;
  }
  return engine;
}

/**
 * A second engine for the scenario runner, created on first use with every
 * corpus law loaded and no persona data at all.
 *
 * Scenario steps register their own data sources and the runner clears them
 * between runs. Done on the portal's engine, that threw away the persona data
 * after every run and forced a full re-registration, which bumps `dataVersion`
 * and makes every kept-alive tile re-evaluate its law: one scenario run cost
 * dozens of law evaluations elsewhere. On its own engine a run touches nothing
 * the portal sees.
 */
let scenarioEngine = null;
let scenarioPromise = null;
export async function prepareScenarioEngine(corpus) {
  if (scenarioEngine) return scenarioEngine;
  if (scenarioPromise) return scenarioPromise;
  scenarioPromise = (async () => {
    try {
      await initEngine();
      const t0 = performance.now();
      const engine = new wasmModule.WasmEngine();
      loadLaws(engine, corpus);
      console.debug(`[scenario-engine] tweede engine met ${engine.lawCount()} wetten geladen in ${(performance.now() - t0).toFixed(0)}ms`);
      scenarioEngine = engine;
      return engine;
    } catch (e) {
      // Do not cache the failure: the next run tries again.
      scenarioPromise = null;
      throw e;
    }
  })();
  return scenarioPromise;
}

/** Every parameter name any demo law declares; these are the candidate keys. */
function allParameterNames(corpus) {
  const names = new Set();
  for (const law of corpus.laws) for (const p of lawShape(law.doc).parameters) names.add(p);
  return [...names];
}

/**
 * Materialise all persona data and register it, one scoped source per
 * (law, organisation, key field). Called once at start-up and again when the
 * reference date changes (a few bindings select on the year).
 */
export function registerPersonaData(engine, corpus, referenceDate, cases = [], claims = []) {
  engine.clearDataSources();
  // A persona's answers to a law's application form (approved claims) count as
  // that law's parameters while its register rows are looked up: the terrace
  // location decides which BGT row applies.
  const paramsFor = (keyField, keyValue, lawId) => {
    const out = {};
    for (const c of claims) if (c.lawId === lawId && c.keyField === keyField && c.keyValue === keyValue) out[c.input] = c.newValue;
    return out;
  };
  const rowsFor = tablesFromProfiles(corpus.profiles);
  const allTables = [];
  const seen = new Set();
  const collectTables = (byService) => {
    for (const byTable of Object.values(byService ?? {})) {
      for (const rows of Object.values(byTable ?? {})) {
        if (!seen.has(rows)) {
          seen.add(rows);
          allTables.push(rows);
        }
      }
    }
  };
  collectTables(corpus.profiles.globalServices);
  for (const profile of Object.values(corpus.profiles.profiles ?? {})) collectTables(profile.sources);

  const keyValues = collectKeyValues(allTables, allParameterNames(corpus));
  for (const bsn of Object.keys(corpus.profiles.profiles ?? {})) {
    if (!keyValues.bsn.includes(bsn)) keyValues.bsn.push(bsn);
  }
  const lawsById = {};
  for (const [id, law] of corpus.latestById) lawsById[id] = law.doc;
  const register = (sources) => {
    for (const { law, service, keyField, records } of sources) {
      try {
        engine.registerDataSourceForLaw(law, service, keyField, records, REGISTER_PRIORITY);
      } catch (e) {
        console.warn(`Databron ${service} voor ${law} niet geregistreerd:`, e);
      }
    }
  };

  // Pass 1: everything that follows from parameters and register rows alone.
  register(materialiseAll(lawsById, corpus.bindings, rowsFor, keyValues, { referencedate: referenceDate, cases, paramsFor }));

  // Pass 2: a few bindings select on a cross-law input (`adres: $vestigingsadres`,
  // the address the KVK law derives). With pass 1 registered, the engine can
  // answer those; re-materialise with that resolver and replace the sources.
  const cache = new Map();
  const resolveRef = (lawId, inputName, params) => {
    const doc = lawsById[lawId];
    const input = (doc?.articles ?? [])
      .flatMap((a) => a.machine_readable?.execution?.input ?? [])
      .find((i) => i.name === inputName);
    const src = input?.source;
    if (!src?.regulation) return undefined;
    const callParams = {};
    for (const [name, ref] of Object.entries(src.parameters ?? {})) {
      const v = typeof ref === 'string' && ref.startsWith('$') ? params[ref.slice(1)] : ref;
      if (v === undefined || v === null) return undefined;
      callParams[name] = v;
    }
    const key = `${src.regulation}#${src.output ?? inputName}|${JSON.stringify(callParams)}`;
    if (cache.has(key)) return cache.get(key);
    let value;
    try {
      const result = engine.execute(src.regulation, src.output ?? inputName, callParams, referenceDate);
      value = result?.outputs?.[src.output ?? inputName];
    } catch {
      value = undefined;
    }
    cache.set(key, value);
    return value;
  };
  const sources = materialiseAll(lawsById, corpus.bindings, rowsFor, keyValues, { referencedate: referenceDate, cases, resolveRef, paramsFor });
  engine.clearDataSources();
  register(sources);
  return sources;
}

/**
 * Register approved citizen corrections as a higher-priority scoped source per
 * law, replacing the previous set.
 *
 * A record never carries `undefined`: the WASM boundary would read it as an
 * absence (`null`), turning "not supplied" into "there is none" (RFC-036). A
 * claim without a value is simply not in the record, so the input stays
 * unknown.
 *
 * @param {Array<{lawId: string, keyField: string, keyValue: string, input: string, newValue: any}>} claims
 */
export function registerClaims(engine, claims) {
  engine.removeDataSource(CLAIMS_SOURCE);
  const grouped = new Map();
  for (const claim of claims) {
    if (claim.newValue === undefined) continue;
    const key = JSON.stringify([claim.lawId, claim.keyField]);
    if (!grouped.has(key)) grouped.set(key, new Map());
    const records = grouped.get(key);
    if (!records.has(claim.keyValue)) records.set(claim.keyValue, { [claim.keyField]: claim.keyValue });
    records.get(claim.keyValue)[claim.input] = claim.newValue;
  }
  for (const [key, records] of grouped) {
    const [lawId, keyField] = JSON.parse(key);
    engine.registerDataSourceForLaw(lawId, CLAIMS_SOURCE, keyField, [...records.values()], CLAIMS_PRIORITY);
  }
}

/**
 * Evaluate a law for the given parameters with a full trace.
 *
 * @returns {{ok: true, outputs: object, trace: object, traceText: string, resolvedInputs: object}
 *   | {ok: false, error: string}}
 */
export function evaluateLaw(engine, lawEntry, params, referenceDate, outputs = null) {
  const names = outputs ?? lawEntry.outputs;
  if (names.length === 0) return { ok: false, error: 'Deze wet heeft geen uitvoer.' };
  try {
    const result = engine.executeMultipleWithTrace(lawEntry.id, names, params, referenceDate);
    return {
      ok: true,
      outputs: result.outputs ?? {},
      trace: result.trace ?? null,
      traceText: result.trace_text ?? '',
      resolvedInputs: result.resolved_inputs ?? {},
      raw: result,
    };
  } catch (e) {
    // The engine throws either a string or an object {error, trace}.
    const message = typeof e === 'string' ? e : e?.error ?? e?.message ?? JSON.stringify(e);
    return { ok: false, error: String(message), trace: e?.trace ?? null };
  }
}

/**
 * Eén stap in de levensloop van een besluit (RFC-007, RFC-008).
 *
 * Waar `evaluateLaw` een wet doorrekent en klaar is, loopt dit de procedure af
 * die de Awb aan dit soort besluit geeft: het vuurt de haken die bij elke fase
 * horen, en stopt bij de eerste fase waarvan het de gegevens niet heeft. Zo
 * komt de bezwaartermijn als datum uit de wet in plaats van als vaste tekst uit
 * het scherm.
 *
 * De engine bewaart niets. `state` is `null` bij de eerste stap en daarna wat
 * de vorige stap teruggaf; de demo bewaart dat bij de zaak. Die verdeling is de
 * kern van RFC-008: de engine blijft een zuivere functie per fase, de
 * orkestratielaag houdt het besluit vast.
 *
 * Een wet die in geen procedure zit rekent gewoon door en is meteen klaar, dus
 * dit is voor elke wet veilig aan te roepen.
 *
 * @returns {{ok: true, complete: boolean, outputs: object, state: object|null,
 *   pendingInputs: string[], currentStage: string|null} | {ok: false, error: string}}
 */
export function executeStage(engine, lawEntry, outputName, state, params, referenceDate) {
  try {
    const result = engine.executeStage(lawEntry.id, outputName, state ?? null, params, referenceDate);
    return {
      ok: true,
      complete: !!result.complete,
      outputs: result.outputs ?? {},
      state: result.state ?? null,
      pendingInputs: result.pending_inputs ?? [],
      currentStage: result.current_stage ?? null,
    };
  } catch (e) {
    const message = typeof e === 'string' ? e : e?.error ?? e?.message ?? JSON.stringify(e);
    return { ok: false, error: String(message) };
  }
}

export function useDemoEngine() {
  return { initEngine, prepareEngine, prepareScenarioEngine, registerPersonaData, registerClaims, evaluateLaw, executeStage, engineReady, engineError, loadFailures, loadFailureFor };
}

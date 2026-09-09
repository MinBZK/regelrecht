/**
 * Run the laws over a synthetic population.
 *
 * The generated register rows replace the personas' data in the engine for
 * the duration of the run (the caller restores them afterwards), every portal
 * law of the audience is evaluated per subject through the same materialiser
 * and engine the portal uses, and the outcomes are summarised. Law constants
 * can be overridden for the run: the law is reloaded with changed
 * `definitions` and put back when the run ends.
 *
 * Long runs yield to the event loop between chunks so the progress bar moves
 * and the page stays responsive.
 */
import { collectKeyValues, lawShape, materialiseAll } from '../data/materialize.js';
import { fieldSpec, isAmountSpec } from '../data/format.js';
import { evaluateLaw } from '../engine/useDemoEngine.js';
import { applyOverrides, effectiveOverrides } from './lawParameters.js';
import { generateBusinesses, generateCitizens, rowsForTables, templatesFromProfiles } from './population.js';
import { describePopulation, summariseLaw } from './stats.js';

const REGISTER_PRIORITY = 10;
const FORM_PRIORITY = 100;
const FORM_SOURCE = 'simulatie-opgaven';
const CHUNK = 5;

const IDENTITY = new Set(['bsn', 'kvk_nummer']);

/** The laws the simulation runs for an audience: the portal's laws, minus those needing parameters the population cannot supply. */
export function simulationLaws(corpus, kind, isLawEnabled = () => true) {
  const wanted = kind === 'ondernemers' ? 'BUSINESS' : 'CITIZEN';
  const laws = [...corpus.latestById.values()].filter((law) => law.discoverable === wanted && isLawEnabled(law));
  const known = new Set([...IDENTITY, ...FORM_PARAMETERS]);
  const runnable = [];
  const skipped = [];
  for (const law of laws) {
    const params = lawShape(law.doc).parameters;
    const missing = params.filter((p) => !known.has(p));
    if (missing.length) skipped.push({ law, missing });
    else runnable.push(law);
  }
  runnable.sort((a, b) => a.name.localeCompare(b.name));
  return { runnable, skipped };
}

/** Application-form parameters the business generator supplies (see population.js). */
const FORM_PARAMETERS = [
  'aangevraagde_categorie',
  'activiteiten',
  'bereidt_of_serveert_voedsel',
  'terras_locatie',
  'terras_oppervlakte',
  'obstakelvrije_ruimte',
  'seizoen',
  'gewenste_openingstijd',
  'gewenste_sluitingstijd_doordeweeks',
  'gewenste_sluitingstijd_weekend',
  'activiteitsdatum',
  'activiteitsstarttijd',
];

function primaryOutputName(corpus, law) {
  const cfg = corpus.config?.dashboard_outputs ?? {};
  return cfg[`${law.service}/${law.law_path}`] ?? null;
}

/** The outcome of one evaluation reduced to what the statistics need. */
export function reduceOutcome(law, primaryName, evaluation) {
  if (!evaluation.ok) return { ok: false, met: null, amount: null, amountName: null, error: evaluation.error };
  const outputs = evaluation.outputs ?? {};
  const met = 'voldoet_aan_voorwaarden' in outputs ? outputs.voldoet_aan_voorwaarden !== false && outputs.voldoet_aan_voorwaarden !== null : null;
  // The amount shown for a law: its configured primary output, or else the
  // first monetary output. A count or a rate is not an amount to average.
  let name = primaryName && outputs[primaryName] !== undefined ? primaryName : null;
  if (!name) {
    name = Object.keys(outputs).find((k) => k !== 'voldoet_aan_voorwaarden' && typeof outputs[k] === 'number' && isAmountSpec(fieldSpec(law.doc, k))) ?? null;
  }
  let amount = null;
  if (name && typeof outputs[name] === 'number') {
    amount = isAmountSpec(fieldSpec(law.doc, name)) ? outputs[name] / 100 : outputs[name];
  }
  return { ok: true, met, amount, amountName: name, error: null, outputs };
}

function sleep(ms = 0) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * @param {object} args
 * @param {object} args.engine       the WASM engine
 * @param {object} args.corpus       loaded corpus
 * @param {'burgers'|'ondernemers'} args.kind
 * @param {object} args.params       population parameters (see population.js defaults)
 * @param {Record<string, Record<string, number>>} args.overrides  law id -> definition -> value
 * @param {string} args.referenceDate
 * @param {(done: number, total: number) => void} [args.onProgress]
 * @param {{ cancelled: boolean }} [args.signal]
 * @param {(law: object) => boolean} [args.isLawEnabled]
 */
export async function runSimulation({ engine, corpus, kind, params, overrides = {}, referenceDate, onProgress, signal, isLawEnabled }) {
  const started = performance.now();
  const templateRow = templatesFromProfiles(corpus.profiles);
  const population = kind === 'ondernemers' ? generateBusinesses(params, referenceDate, templateRow) : generateCitizens(params, referenceDate, templateRow);
  const { runnable: laws, skipped } = simulationLaws(corpus, kind, isLawEnabled);

  // Law constants for this run.
  const applied = {};
  for (const law of laws) {
    const eff = effectiveOverrides(law.doc, overrides[law.id]);
    if (Object.keys(eff).length) {
      engine.loadLaw(applyOverrides(law.doc, eff));
      applied[law.id] = eff;
    }
  }

  const results = [];
  const primaries = new Map(laws.map((law) => [law.id, primaryOutputName(corpus, law)]));
  try {
    // Register data: the generated rows, plus the shared tables every persona has.
    const shared = {};
    for (const [service, byTable] of Object.entries(corpus.profiles.globalServices ?? {})) shared[service] = byTable;
    const rowsFor = rowsForTables(shared, population.tables);
    const lawsById = {};
    for (const [id, law] of corpus.latestById) lawsById[id] = law.doc;
    // Every law is keyed on one of its parameters; besides bsn and kvk_nummer
    // that is an address (wet_bag), a year (CBS) and the like. Collect the
    // values every parameter name takes in the generated and shared tables,
    // exactly as the persona registration does, so those laws get records too.
    const parameterNames = new Set();
    for (const doc of Object.values(lawsById)) for (const p of lawShape(doc).parameters) parameterNames.add(p);
    const allTables = [shared, population.tables].flatMap((set) => Object.values(set).flatMap((byTable) => Object.values(byTable)));
    const keyValues = collectKeyValues(allTables, [...parameterNames]);
    for (const [key, values] of Object.entries(population.keyValues)) {
      keyValues[key] = [...new Set([...(keyValues[key] ?? []), ...values])];
    }
    // A business's application-form answers (terrace location and size) are
    // known here, unlike on the portal; bindings that select on them get them.
    const paramsFor = (keyField, keyValue) => (keyField === 'kvk_nummer' ? population.formValues?.[keyValue] ?? {} : {});
    engine.clearDataSources();
    const register = (sources) => {
      for (const { law, service, keyField, records } of sources) {
        try {
          engine.registerDataSourceForLaw(law, service, keyField, records, REGISTER_PRIORITY);
        } catch (e) {
          console.warn(`Simulatie: databron ${service} voor ${law} niet geregistreerd:`, e);
        }
      }
    };
    register(materialiseAll(lawsById, corpus.bindings, rowsFor, keyValues, { referencedate: referenceDate, paramsFor }));
    // Second pass for bindings that select on a cross-law input (the KVK address).
    const cache = new Map();
    const resolveRef = (lawId, inputName, callerParams) => {
      const doc = lawsById[lawId];
      const input = (doc?.articles ?? []).flatMap((a) => a.machine_readable?.execution?.input ?? []).find((i) => i.name === inputName);
      const src = input?.source;
      if (!src?.regulation) return undefined;
      const callParams = {};
      for (const [name, ref] of Object.entries(src.parameters ?? {})) {
        const v = typeof ref === 'string' && ref.startsWith('$') ? callerParams[ref.slice(1)] : ref;
        if (v === undefined || v === null) return undefined;
        callParams[name] = v;
      }
      const key = `${src.regulation}#${src.output ?? inputName}|${JSON.stringify(callParams)}`;
      if (cache.has(key)) return cache.get(key);
      let value;
      try {
        value = engine.execute(src.regulation, src.output ?? inputName, callParams, referenceDate)?.outputs?.[src.output ?? inputName];
      } catch {
        value = undefined;
      }
      cache.set(key, value);
      return value;
    };
    const sources = materialiseAll(lawsById, corpus.bindings, rowsFor, keyValues, { referencedate: referenceDate, resolveRef, paramsFor });
    engine.clearDataSources();
    register(sources);
    // Form answers (`kind: claim` inputs) as a higher-priority source.
    const grouped = new Map();
    for (const c of population.claims) {
      const key = JSON.stringify([c.lawId, c.keyField]);
      if (!grouped.has(key)) grouped.set(key, new Map());
      const records = grouped.get(key);
      if (!records.has(c.keyValue)) records.set(c.keyValue, { [c.keyField]: c.keyValue });
      records.get(c.keyValue)[c.input] = c.value;
    }
    for (const [key, records] of grouped) {
      const [lawId, keyField] = JSON.parse(key);
      engine.registerDataSourceForLaw(lawId, FORM_SOURCE, keyField, [...records.values()], FORM_PRIORITY);
    }

    // Evaluate.
    const total = population.subjects.length;
    for (let i = 0; i < total; i += 1) {
      if (signal?.cancelled) break;
      const subject = population.subjects[i];
      const form = population.formValues?.[subject.id] ?? {};
      const outcome = { subject, laws: {} };
      for (const law of laws) {
        const declared = lawShape(law.doc).parameters;
        const callParams = {};
        for (const p of declared) {
          if (p === 'bsn') callParams.bsn = subject.bsn ?? form.bsn ?? null;
          else if (p === 'kvk_nummer') callParams.kvk_nummer = subject.kvk_nummer ?? null;
          else callParams[p] = form[p] ?? null;
        }
        outcome.laws[law.id] = reduceOutcome(law, primaries.get(law.id), evaluateLaw(engine, law, callParams, referenceDate));
      }
      results.push(outcome);
      if (i % CHUNK === CHUNK - 1) {
        onProgress?.(i + 1, total);
        await sleep(0);
      }
    }
    onProgress?.(results.length, total);
  } finally {
    // Put the laws back the way the corpus has them; the caller re-registers persona data.
    for (const lawId of Object.keys(applied)) {
      const law = corpus.latestById.get(lawId);
      if (law) engine.loadLaw(law.text);
    }
    engine.clearDataSources();
  }

  const lawIds = laws.map((l) => l.id);
  return {
    id: `run-${Date.now().toString(36)}`,
    kind,
    params: JSON.parse(JSON.stringify(params)),
    overrides: applied,
    referenceDate,
    cancelled: !!signal?.cancelled,
    laws: laws.map((law) => ({ id: law.id, name: law.name, service: law.service, law_path: law.law_path, primaryName: primaries.get(law.id) })),
    skipped: skipped.map(({ law, missing }) => ({ id: law.id, name: law.name, missing })),
    subjects: population.subjects,
    results,
    summary: Object.fromEntries(lawIds.map((id) => [id, summariseLaw(results, id)])),
    population: describePopulation(kind, population.subjects),
    durationMs: Math.round(performance.now() - started),
  };
}

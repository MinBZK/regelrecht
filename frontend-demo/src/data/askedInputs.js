/**
 * The questions a regeling has for the citizen: inputs no register holds
 * (`kind: claim` in the bindings, such as the rent) and the parameters the law
 * declares beyond the persona's identity (an application form: terrace size,
 * whether food is served). Both are answered the same way, as a self-declared
 * value that becomes a claim on the law; until answered they are unknown.
 *
 * Which question comes next is what the engine says is missing (RFC-036): an
 * unknown outcome carries the facts nobody supplied, each with the law that
 * needed it and why it is missing (`no_data`: a register input without a
 * value; `not_passed`: an optional parameter the caller left out). The
 * portal asks for exactly those, in the order the engine reached them.
 */
import { missingFacts } from '@regelrecht/frontend-shared';
import { fieldSpec } from './format.js';

const IDENTITY = new Set(['bsn', 'kvk_nummer']);

/**
 * @param {object} corpus
 * @param {object} law            LawEntry
 * @param {(lawId: string, input: string) => object|null} claimFor
 * @returns {Array<{name: string, spec: object|null, claim: object|null, isParameter: boolean, required: boolean}>}
 */
export function askedInputsFor(corpus, law, claimFor) {
  const out = [];
  const bindings = corpus?.bindings?.[law.id] ?? {};
  for (const [name, b] of Object.entries(bindings)) {
    if (b.kind === 'claim') out.push({ name, spec: fieldSpec(law.doc, name), claim: claimFor(law.id, name), isParameter: false, required: false });
  }
  for (const article of law.doc?.articles ?? []) {
    for (const p of article.machine_readable?.execution?.parameters ?? []) {
      if (IDENTITY.has(p.name) || out.some((o) => o.name === p.name)) continue;
      // `required` defaults to true (schema v0.5.8): a required parameter the
      // caller omits is an error, not an unknown, so it has to be asked first.
      out.push({ name: p.name, spec: p, claim: claimFor(law.id, p.name), isParameter: true, required: p.required !== false });
    }
  }
  return out;
}

/**
 * The parameters for an evaluation: the identity plus every answered form
 * parameter. An unanswered parameter is left out, not passed as null: null
 * would tell the law "there is none", while the truth is that nobody has said
 * yet (the engine then reports it as not passed).
 */
export function evaluationParamsFor(personaParams, asked) {
  const params = { ...personaParams };
  for (const a of asked) if (a.isParameter && a.claim) params[a.name] = a.claim.newValue;
  return params;
}

/** How a question is answered, from its declared type or the shape of its value. */
export function inputKind(spec, value = null) {
  if (spec?.type === 'boolean' || typeof value === 'boolean') return 'boolean';
  if (spec?.type === 'amount' || spec?.type_spec?.unit === 'eurocent') return 'amount';
  if (spec?.type === 'number') return 'number';
  if (spec?.type === 'date') return 'date';
  if (spec?.type === 'array' || spec?.type === 'object') return 'json';
  return 'text';
}

/** Parse a form answer into the value the law expects. Returns undefined when empty or invalid. */
export function parseAnswer(kind, raw) {
  const s = String(raw ?? '').trim();
  if (s === '') return undefined;
  switch (kind) {
    case 'amount': {
      const n = Number(s.replace(/\./g, '').replace(',', '.'));
      return Number.isFinite(n) ? Math.round(n * 100) : undefined;
    }
    case 'number': {
      const n = Number(s.replace(',', '.'));
      return Number.isFinite(n) ? n : undefined;
    }
    case 'boolean':
      return s === 'true' ? true : s === 'false' ? false : undefined;
    case 'enum':
      return /^-?\d+(\.\d+)?$/.test(s) ? Number(s) : s;
    case 'json':
      try {
        return JSON.parse(s);
      } catch {
        return undefined;
      }
    default:
      return s;
  }
}

/**
 * The key an answer is stored under for a law: the identity that law is run
 * for. A business law (declares `kvk_nummer`) keys on the KVK number, so a law
 * that later asks it cross-law with only the KVK number finds the answer.
 */
export function claimKeyFor(law, params) {
  const declared = new Set((law.doc?.articles ?? []).flatMap((a) => (a.machine_readable?.execution?.parameters ?? []).map((p) => p.name)));
  if (declared.has('kvk_nummer') && params.kvk_nummer) return { keyField: 'kvk_nummer', keyValue: params.kvk_nummer };
  return { keyField: 'bsn', keyValue: params.bsn ?? params.kvk_nummer };
}

/**
 * The facts this law's outcome says are missing, in the order the engine
 * reached them, de-duplicated: `{ law, name, kind }` per fact.
 */
export function missingFactsOf(evaluation) {
  const out = [];
  for (const value of Object.values(evaluation?.outputs ?? {})) {
    for (const fact of missingFacts(value)) {
      if (!out.some((f) => f.law === fact.law && f.name === fact.name && f.kind === fact.kind)) out.push(fact);
    }
  }
  return out;
}

/**
 * The questions to ask now, just in time: the unanswered inputs and form
 * parameters the outcome names as missing for this law, in the order the
 * engine reached them. A register input is asked when the outcome misses it
 * as `no_data` and the citizen is the one who can supply it (a `kind: claim`
 * binding); a form parameter when the outcome misses it as `not_passed`. A
 * required parameter is asked before anything else: without it the engine
 * cannot run at all, so no outcome could name it.
 */
export function nextQuestions(asked, evaluation, lawId) {
  const unanswered = asked.filter((a) => !a.claim);
  if (!unanswered.length) return [];
  const required = unanswered.filter((a) => a.isParameter && a.required);
  const missing = missingFactsOf(evaluation).filter((f) => f.law === lawId);
  const reached = unanswered.filter((a) => missing.some((f) => f.name === a.name && f.kind === (a.isParameter ? 'not_passed' : 'no_data')));
  reached.sort((a, b) => missing.findIndex((f) => f.name === a.name) - missing.findIndex((f) => f.name === b.name));
  return [...required, ...reached.filter((a) => !required.includes(a))];
}

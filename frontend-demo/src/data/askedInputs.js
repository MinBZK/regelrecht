/**
 * The questions a regeling has for the citizen: inputs no register holds
 * (`kind: claim` in the bindings, such as the rent) and the parameters the law
 * declares beyond the persona's identity (an application form: terrace size,
 * whether food is served). Both are answered the same way, as a self-declared
 * value that becomes a claim on the law; until answered they are unknown.
 */
import { fieldSpec } from './format.js';
import { leafValues, lineageFromTrace } from './lineage.js';

const IDENTITY = new Set(['bsn', 'kvk_nummer']);

/**
 * @param {object} corpus
 * @param {object} law            LawEntry
 * @param {(lawId: string, input: string) => object|null} claimFor
 * @returns {Array<{name: string, spec: object|null, claim: object|null, isParameter: boolean}>}
 */
export function askedInputsFor(corpus, law, claimFor) {
  const out = [];
  const bindings = corpus?.bindings?.[law.id] ?? {};
  for (const [name, b] of Object.entries(bindings)) {
    if (b.kind === 'claim') out.push({ name, spec: fieldSpec(law.doc, name), claim: claimFor(law.id, name), isParameter: false });
  }
  for (const article of law.doc?.articles ?? []) {
    for (const p of article.machine_readable?.execution?.parameters ?? []) {
      if (IDENTITY.has(p.name) || out.some((o) => o.name === p.name)) continue;
      out.push({ name: p.name, spec: p, claim: claimFor(law.id, p.name), isParameter: true });
    }
  }
  return out;
}

/** The parameters for an evaluation: the identity plus every answered form parameter (unanswered ones as null). */
export function evaluationParamsFor(personaParams, asked) {
  const params = { ...personaParams };
  for (const a of asked) if (a.isParameter) params[a.name] = a.claim ? a.claim.newValue : null;
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
 * The questions to ask now, just in time: the unanswered inputs the engine
 * actually reached in the last evaluation (in the order it reached them) and
 * the unanswered form parameters. An input the calculation never touched is
 * not asked. The engine re-runs after every answer, so the next question is
 * whatever it runs into next.
 */
export function nextQuestions(asked, evaluation, lawId, params) {
  const unanswered = asked.filter((a) => !a.claim);
  if (!unanswered.length) return [];
  const reached = evaluation?.trace
    ? leafValues(lineageFromTrace(evaluation.trace, lawId, params)).filter((n) => n.law === lawId && n.value === null).map((n) => n.name)
    : [];
  const inputs = unanswered.filter((a) => !a.isParameter && reached.includes(a.name)).sort((a, b) => reached.indexOf(a.name) - reached.indexOf(b.name));
  const parameters = unanswered.filter((a) => a.isParameter);
  return [...inputs, ...parameters];
}

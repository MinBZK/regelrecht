import { describe, expect, it } from 'vitest';
import { askedInputsFor, claimKeyFor, evaluationParamsFor, inputKind, nextQuestions, parseAnswer } from './askedInputs.js';

const law = {
  id: 'terras',
  doc: {
    articles: [
      {
        machine_readable: {
          execution: {
            parameters: [{ name: 'kvk_nummer', type: 'string' }, { name: 'terras_oppervlakte', type: 'number', required: false }],
            input: [{ name: 'huurprijs', type: 'amount', source: {} }],
          },
        },
      },
    ],
  },
};
const corpus = { bindings: { terras: { huurprijs: { kind: 'claim', service: 'TOESLAGEN' }, adres: { kind: 'table', service: 'KVK' } } } };

describe('askedInputsFor', () => {
  it('lists claim inputs and non-identity parameters with their claims', () => {
    const claimFor = (lawId, name) => (name === 'huurprijs' ? { newValue: 65000 } : null);
    const asked = askedInputsFor(corpus, law, claimFor);
    expect(asked.map((a) => a.name)).toEqual(['huurprijs', 'terras_oppervlakte']);
    expect(asked[0]).toMatchObject({ isParameter: false, claim: { newValue: 65000 } });
    expect(asked[0].spec.type).toBe('amount');
    expect(asked[1]).toMatchObject({ isParameter: true, claim: null });
  });

  it('builds evaluation parameters from the answered form parameters', () => {
    const asked = askedInputsFor(corpus, law, (lawId, name) => (name === 'terras_oppervlakte' ? { newValue: 18 } : null));
    expect(evaluationParamsFor({ bsn: '1', kvk_nummer: '2' }, asked)).toEqual({ bsn: '1', kvk_nummer: '2', terras_oppervlakte: 18 });
    const none = askedInputsFor(corpus, law, () => null);
    expect(evaluationParamsFor({ kvk_nummer: '2' }, none)).toEqual({ kvk_nummer: '2', terras_oppervlakte: null });
  });
});

describe('inputKind / parseAnswer', () => {
  it('derives the control from the spec', () => {
    expect(inputKind({ type: 'amount' })).toBe('amount');
    expect(inputKind({ type: 'number', type_spec: { unit: 'eurocent' } })).toBe('amount');
    expect(inputKind({ type: 'boolean' })).toBe('boolean');
    expect(inputKind({ type: 'date' })).toBe('date');
    expect(inputKind({ type: 'array' })).toBe('json');
    expect(inputKind(null, 'x')).toBe('text');
  });

  it('parses euros to eurocent, Dutch decimals, booleans and JSON', () => {
    expect(parseAnswer('amount', '650,50')).toBe(65050);
    expect(parseAnswer('amount', '1.250')).toBe(125000);
    expect(parseAnswer('number', '18,5')).toBe(18.5);
    expect(parseAnswer('boolean', 'true')).toBe(true);
    expect(parseAnswer('json', '["a"]')).toEqual(['a']);
    expect(parseAnswer('json', '{')).toBeUndefined();
    expect(parseAnswer('amount', '')).toBeUndefined();
    expect(parseAnswer('text', ' voor ')).toBe('voor');
  });
});

describe('nextQuestions / claimKeyFor', () => {
  const resolve = (name, result) => ({ node_type: 'resolve', name, result, resolve_type: 'DATA_SOURCE', message: 'Resolving from SOURCE correcties: null', children: [] });
  const asked = [
    { name: 'huurprijs', claim: null, isParameter: false },
    { name: 'servicekosten', claim: null, isParameter: false },
    { name: 'subsidiabele_servicekosten', claim: null, isParameter: false },
    { name: 'terras_locatie', claim: null, isParameter: true },
  ];

  it('asks only what the engine reached, in that order, then the form parameters', () => {
    const evaluation = { ok: true, trace: { node_type: 'article', children: [resolve('servicekosten', null), resolve('huurprijs', null), resolve('inkomen', 2500000)] } };
    expect(nextQuestions(asked, evaluation, 'terras', { bsn: '1' }).map((a) => a.name)).toEqual(['servicekosten', 'huurprijs', 'terras_locatie']);
  });

  it('skips answered inputs and asks nothing without unanswered ones', () => {
    const answered = asked.map((a) => ({ ...a, claim: { newValue: 1 } }));
    expect(nextQuestions(answered, null, 'terras', {})).toEqual([]);
    expect(nextQuestions(asked, null, 'terras', {}).map((a) => a.name)).toEqual(['terras_locatie']);
  });

  it('keys answers on the KVK number for a business law', () => {
    expect(claimKeyFor(law, { bsn: '1', kvk_nummer: '2' })).toEqual({ keyField: 'kvk_nummer', keyValue: '2' });
    const citizenLaw = { doc: { articles: [{ machine_readable: { execution: { parameters: [{ name: 'bsn' }] } } }] } };
    expect(claimKeyFor(citizenLaw, { bsn: '1', kvk_nummer: '2' })).toEqual({ keyField: 'bsn', keyValue: '1' });
  });
});

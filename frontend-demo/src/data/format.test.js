import { describe, expect, it } from 'vitest';
import { fieldSpec, formatDate, formatValue, humanize, isAmountSpec, numericImpact } from './format.js';

const DOC = {
  articles: [
    {
      machine_readable: {
        execution: {
          parameters: [{ name: 'bsn', type: 'string' }],
          input: [{ name: 'leeftijd', type: 'number', type_spec: { unit: 'years' } }],
          output: [{ name: 'hoogte_toeslag', type: 'amount', type_spec: { unit: 'eurocent', precision: 0 } }],
        },
      },
    },
  ],
};

describe('fieldSpec', () => {
  it('finds outputs, inputs and parameters by name', () => {
    expect(fieldSpec(DOC, 'hoogte_toeslag').type).toBe('amount');
    expect(fieldSpec(DOC, 'leeftijd').type_spec.unit).toBe('years');
    expect(fieldSpec(DOC, 'bsn').type).toBe('string');
    expect(fieldSpec(DOC, 'onbekend')).toBeNull();
    expect(fieldSpec(null, 'x')).toBeNull();
  });
});

describe('formatValue', () => {
  it('renders eurocent amounts as euros', () => {
    expect(formatValue(165412, fieldSpec(DOC, 'hoogte_toeslag'))).toBe('€\u00a01.654,12');
    expect(formatValue(3610, { type: 'number', type_spec: { unit: 'eurocent' } })).toBe('€\u00a036,10');
  });

  it('renders units, booleans and unknowns in Dutch', () => {
    expect(formatValue(null)).toBe('onbekend');
    expect(formatValue(undefined)).toBe('onbekend');
    expect(formatValue(true)).toBe('Ja');
    expect(formatValue(false)).toBe('Nee');
    expect(formatValue(35, fieldSpec(DOC, 'leeftijd'))).toBe('35 jaar');
    expect(formatValue(12.5, { type_spec: { unit: 'percentage' } })).toBe('12,5%');
  });

  it('renders dates, snake_case strings and collections', () => {
    expect(formatValue('2025-03-01')).toBe('1 maart 2025');
    expect(formatValue('ALLEENSTAANDE_OUDER')).toBe('ALLEENSTAANDE OUDER');
    expect(formatValue([])).toBe('geen');
    expect(formatValue(['a', 'b'])).toBe('a, b');
    expect(formatValue([{ x: 1 }])).toBe('1 item');
    expect(formatValue({ straat: 'Kade', nummer: 1, plaats: null })).toBe('Kade 1');
  });
});

describe('helpers', () => {
  it('isAmountSpec accepts the amount type and the eurocent unit', () => {
    expect(isAmountSpec({ type: 'amount' })).toBe(true);
    expect(isAmountSpec({ type: 'number', type_spec: { unit: 'eurocent' } })).toBe(true);
    expect(isAmountSpec({ type: 'number' })).toBe(false);
    expect(isAmountSpec(null)).toBe(false);
  });

  it('numericImpact scales eurocents to euros and ignores non-numbers', () => {
    expect(numericImpact(250000, { type: 'amount' })).toBe(2500);
    expect(numericImpact(4, { type: 'number' })).toBe(4);
    expect(numericImpact('x', { type: 'amount' })).toBe(0);
  });

  it('humanize turns an identifier into a label', () => {
    expect(humanize('hoogte_toeslag')).toBe('Hoogte toeslag');
    expect(humanize('')).toBe('');
  });

  it('formatDate leaves an unparsable value alone', () => {
    expect(formatDate('geen datum')).toBe('geen datum');
  });
});

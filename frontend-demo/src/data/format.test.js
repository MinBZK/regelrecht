import { describe, expect, it } from 'vitest';
import { fieldSpec, formatDate, formatMissing, formatValue, humanize, isAmountSpec, numericImpact, verdictOf } from './format.js';

const UNKNOWN = { __unknown: true, missing: [{ law: 'zorgtoeslagwet', name: 'huurprijs', kind: 'no_data' }, { law: 'wet_inkomstenbelasting', name: 'spaargeld', kind: 'no_data' }] };

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

  it('renders units, booleans, absence and unknowns in Dutch', () => {
    // null is an absence the data states; the engine's Unknown is a fact nobody has.
    expect(formatValue(null)).toBe('geen');
    expect(formatValue(UNKNOWN)).toBe('onbekend');
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

describe('formatMissing / verdictOf', () => {
  it('names the missing facts, with the law when it is another one', () => {
    expect(formatMissing(UNKNOWN, { ownLaw: 'zorgtoeslagwet', lawName: (id) => (id === 'wet_inkomstenbelasting' ? 'Wet IB' : id) })).toBe('ontbreekt: huurprijs, spaargeld (Wet IB)');
    expect(formatMissing(UNKNOWN)).toBe('ontbreekt: huurprijs (zorgtoeslagwet), spaargeld (wet_inkomstenbelasting)');
    expect(formatMissing(null)).toBe('');
    expect(formatMissing(42)).toBe('');
  });

  it('reads the verdict without ever taking an unknown for a yes', () => {
    expect(verdictOf({ voldoet_aan_voorwaarden: true })).toBe(true);
    expect(verdictOf({ voldoet_aan_voorwaarden: false })).toBe(false);
    expect(verdictOf({ voldoet_aan_voorwaarden: UNKNOWN })).toBe('unknown');
    expect(verdictOf({ voldoet_aan_voorwaarden: null })).toBe(false);
    expect(verdictOf({ bedrag: 1 })).toBeNull();
    expect(verdictOf(null)).toBeNull();
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

  it('humanize schrijft een afkorting als afkorting', () => {
    // Zonder dit werd het "Agp vergunning vereist", wat als woord leest.
    expect(humanize('agp_vergunning_vereist')).toBe('AGP vergunning vereist');
    expect(humanize('heeft_haccp_verplichting')).toBe('Heeft HACCP verplichting');
    expect(humanize('kvk_nummer')).toBe('KvK nummer');
    // Een afkorting vooraan houdt haar eigen schrijfwijze.
    expect(humanize('bsn')).toBe('BSN');
    expect(humanize('ww_uitkering_per_maand')).toBe('WW uitkering per maand');
    // Een woord dat toevallig op een afkorting lijkt blijft ongemoeid.
    expect(humanize('wonen_in_nederland')).toBe('Wonen in nederland');
  });

  it('formatDate leaves an unparsable value alone', () => {
    expect(formatDate('geen datum')).toBe('geen datum');
  });
});

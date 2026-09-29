import { afterEach, describe, expect, it } from 'vitest';
import { describeCell, describeRecords, describeSteps, describeValue, groupItems } from './scenarioSteps.js';
import { adoptLocale } from '../i18n/index.js';

const AMOUNT = { type: 'amount', type_spec: { unit: 'eurocent' } };

afterEach(() => adoptLocale('nl'));

describe('describeValue', () => {
  it('shows a stated absence as "geen", never as the literal token', () => {
    expect(describeValue(null)).toEqual({ text: 'geen', quiet: 'none' });
    expect(describeValue([])).toEqual({ text: 'geen', quiet: 'none' });
  });

  it('keeps "not stated" apart from "none" (RFC-036)', () => {
    expect(describeValue(undefined)).toEqual({ text: 'niet opgegeven', quiet: 'not_stated' });
  });

  it('formats an amount by its spec, and leaves an identifier ungrouped', () => {
    expect(describeValue(209692, AMOUNT).text).toBe('€ 2.096,92');
    expect(describeValue(999993653).text).toBe('999993653');
    expect(describeValue(20.5).text).toBe('20,5');
  });

  it('marks zero as quiet but still says 0', () => {
    expect(describeValue(0)).toEqual({ text: '0', quiet: 'zero' });
  });

  it('shows dates, booleans and a list of records in words', () => {
    expect(describeValue('2008-01-01').text).toBe('1 januari 2008');
    expect(describeValue(true).text).toBe('Ja');
    expect(describeValue([{ bsn: '1', geboortedatum: '2020-05-01' }]).text).toBe('BSN: 1, Geboortedatum: 1 mei 2020');
  });

  it('shows a record in one cell with its keys, the absent fields too', () => {
    expect(describeValue({ gebied: null, max_oppervlakte: null })).toEqual({ text: 'Gebied: geen, Max oppervlakte: geen', quiet: null });
  });

  it('keeps a code as written, underscores and all', () => {
    expect(describeValue('ONBEPAALDE_TIJD_REGULIER').text).toBe('ONBEPAALDE_TIJD_REGULIER');
  });

  it('follows the language', () => {
    adoptLocale('en');
    expect(describeValue(null).text).toBe('none');
    expect(describeValue(undefined).text).toBe('not stated');
  });
});

describe('describeCell', () => {
  it('reads the literal cell text the way the runner does', () => {
    expect(describeCell('null').quiet).toBe('none');
    expect(describeCell('[]').quiet).toBe('none');
    expect(describeCell('   ').quiet).toBe('not_stated');
    expect(describeCell(' NEDERLAND ').text).toBe('NEDERLAND');
  });
});

describe('describeRecords', () => {
  const table = [
    ['bsn', 'geboortedatum', 'partner_bsn', 'kinderen_gegevens', 'loon', 'adres'],
    ['999993653', '2008-01-01', 'null', '[]', '0', ''],
  ];

  it('titles a record by its key and gathers the quiet fields by kind', () => {
    const [record] = describeRecords(table, { key: 'bsn', specFor: (name) => (name === 'loon' ? AMOUNT : null) });
    expect(record.title).toEqual({ label: 'BSN', text: '999993653' });
    expect(record.fields).toEqual([{ name: 'geboortedatum', label: 'Geboortedatum', text: '1 januari 2008' }]);
    expect(record.none).toEqual(['Partner BSN', 'Kinderen gegevens']);
    expect(record.zero).toEqual(['Loon']);
    expect(record.notStated).toEqual(['Adres']);
  });

  it('has no title without a key', () => {
    expect(describeRecords(table)[0].title).toBeNull();
    expect(describeRecords([['a']])).toEqual([]);
  });
});

describe('describeSteps', () => {
  const steps = [
    { keyword: 'Given', text: 'the calculation date is "2025-02-01"' },
    { keyword: 'And', text: 'parameter "bsn" is "999993653"' },
    {
      keyword: 'And',
      text: 'the following "RvIG" data with key "bsn" for law "wet_brp":',
      dataTable: [
        ['bsn', 'geboortedatum'],
        ['999993653', '2008-01-01'],
      ],
    },
    { keyword: 'When', text: 'I evaluate outputs "is_verzekerde_zorgtoeslag, hoogte_toeslag" of "zorgtoeslagwet"' },
    { keyword: 'Then', text: 'output "is_verzekerde_zorgtoeslag" is true' },
    { keyword: 'And', text: 'output "hoogte_toeslag" equals 210821' },
    { keyword: 'And', text: 'output "partner" is unknown for lack of "partner_bsn"' },
    { keyword: 'And', text: 'something the grammar does not know' },
  ];
  const options = {
    lawName: (id) => ({ wet_brp: 'Wet BRP', zorgtoeslagwet: 'Zorgtoeslagwet' })[id] ?? id,
    serviceName: (code) => ({ RvIG: 'Rijksdienst voor Identiteitsgegevens' })[code] ?? code,
    specFor: (law, field) => (law === 'zorgtoeslagwet' && field === 'hoogte_toeslag' ? AMOUNT : null),
  };

  it('turns every step into a readable item, keeping its position', () => {
    const items = describeSteps(steps, options);
    expect(items.map((i) => i.index)).toEqual([0, 1, 2, 3, 4, 5, 6, 7]);
    expect(items[0]).toMatchObject({ kind: 'setting', label: 'Peildatum', text: '1 februari 2025' });
    expect(items[1]).toMatchObject({ kind: 'setting', label: 'BSN', text: '999993653' });
    expect(items[2]).toMatchObject({
      kind: 'source',
      org: 'RvIG',
      title: 'Gegevens van Rijksdienst voor Identiteitsgegevens',
      subtitle: 'Voor Wet BRP, per BSN',
    });
    expect(items[3]).toMatchObject({ kind: 'evaluate', title: 'Zorgtoeslagwet', outputs: ['Is verzekerde zorgtoeslag', 'Hoogte toeslag'] });
    expect(items[4]).toMatchObject({ kind: 'expect', label: 'Is verzekerde zorgtoeslag', text: 'Ja' });
    // The spec comes from the law the When step evaluated.
    expect(items[5]).toMatchObject({ kind: 'expect', label: 'Hoogte toeslag', text: '€ 2.108,21' });
    expect(items[6]).toMatchObject({ kind: 'expect', text: 'onbekend, want partner BSN ontbreekt' });
    expect(items[7]).toMatchObject({ kind: 'raw', matched: false });
  });

  it('types an expected "contains" value once, as the runner does', () => {
    const [item] = describeSteps([{ keyword: 'Then', text: 'output "codes" contains "A_1"' }], options);
    expect(item).toMatchObject({ kind: 'expect', label: 'Codes', text: 'bevat A_1' });
  });

  it('groups settings, data and each evaluation with its expectations, in order', () => {
    const blocks = groupItems(describeSteps([...steps.slice(0, 6), steps[3], steps[4]], options));
    expect(blocks.map((b) => b.kind)).toEqual(['settings', 'data', 'check', 'check']);
    expect(blocks[0].rows).toHaveLength(2);
    expect(blocks[2].expects.map((e) => e.index)).toEqual([4, 5]);
    expect(blocks[3].expects.map((e) => e.index)).toEqual([7]);
  });

  it('spreads a parameters table into setting rows that point at its step', () => {
    const [block] = groupItems(
      describeSteps([{ keyword: 'Given', text: 'the following parameters:', dataTable: [['bsn', '1'], ['jaar', '']] }], options),
    );
    expect(block.rows).toEqual([
      { icon: 'tag', label: 'BSN', text: '1', index: 0 },
      { icon: 'tag', label: 'Jaar', text: 'niet opgegeven', index: 0 },
    ]);
  });
});

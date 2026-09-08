import { describe, expect, it } from 'vitest';
import {
  collectKeyValues,
  keyFieldsFor,
  lawShape,
  materialiseAll,
  materialiseRecord,
  tablesFromProfiles,
} from './materialize.js';

const LAW = {
  $id: 'test_wet',
  articles: [
    {
      number: '1',
      machine_readable: {
        execution: {
          parameters: [{ name: 'bsn', type: 'string' }, { name: 'aanvraag_bedrag', type: 'amount' }],
          input: [
            { name: 'geboortedatum', type: 'date', source: {} },
            { name: 'inkomen', type: 'amount', source: {} },
            { name: 'kinderen', type: 'array', source: {} },
            { name: 'partner_inkomen', type: 'amount', source: {} },
            { name: 'eigen_verklaring', type: 'boolean', source: {} },
          ],
        },
      },
    },
  ],
};

const BINDINGS = {
  geboortedatum: {
    kind: 'table',
    service: 'RvIG',
    table: 'personen',
    field: 'geboortedatum',
    select_on: [{ name: 'bsn', value: '$bsn' }],
  },
  inkomen: {
    kind: 'table',
    service: 'BELASTINGDIENST',
    table: 'inkomen',
    field: 'bedrag',
    select_on: [{ name: 'bsn', value: '$bsn' }],
  },
  kinderen: {
    kind: 'table',
    service: 'RvIG',
    table: 'kinderen',
    field: 'kind_bsn',
    select_on: [{ name: 'ouder_bsn', value: '$bsn' }],
  },
  // Selects on another input of the same law: resolved after `partner_bsn`
  // would be, here it is missing so the value stays unknown.
  partner_inkomen: {
    kind: 'table',
    service: 'BELASTINGDIENST',
    table: 'inkomen',
    field: 'bedrag',
    select_on: [{ name: 'bsn', value: '$partner_bsn' }],
  },
  eigen_verklaring: { kind: 'claim', service: 'TOESLAGEN' },
};

const TABLES = {
  'RvIG personen': [{ bsn: '100000001', geboortedatum: '1990-01-01' }],
  'BELASTINGDIENST inkomen': [{ bsn: '100000001', bedrag: 2500000 }],
  'RvIG kinderen': [
    { ouder_bsn: '100000001', kind_bsn: '200000001' },
    { ouder_bsn: '100000001', kind_bsn: '200000002' },
    { ouder_bsn: '100000009', kind_bsn: '200000009' },
  ],
};
const rowsFor = (service, table) => TABLES[`${service} ${table}`] ?? [];

describe('lawShape', () => {
  it('reads parameters and input types across articles', () => {
    const shape = lawShape(LAW);
    expect(shape.id).toBe('test_wet');
    expect(shape.parameters).toEqual(['bsn', 'aanvraag_bedrag']);
    expect(shape.inputTypes.inkomen).toBe('amount');
    expect(shape.inputTypes.kinderen).toBe('array');
  });
});

describe('keyFieldsFor', () => {
  it('keys on the parameter the bindings select on', () => {
    expect(keyFieldsFor(lawShape(LAW), BINDINGS)).toEqual(['bsn']);
  });

  it('falls back to the first parameter without any selecting binding', () => {
    expect(keyFieldsFor(lawShape(LAW), { eigen_verklaring: BINDINGS.eigen_verklaring })).toEqual(['bsn']);
  });

  it('prefers bsn on a tie', () => {
    const shape = { id: 'x', parameters: ['kvk_nummer', 'bsn'], inputTypes: {} };
    const bindings = {
      a: { kind: 'table', select_on: [{ name: 'kvk_nummer', value: '$kvk_nummer' }] },
      b: { kind: 'table', select_on: [{ name: 'bsn', value: '$bsn' }] },
    };
    expect(keyFieldsFor(shape, bindings)[0]).toBe('bsn');
  });
});

describe('materialiseRecord', () => {
  const shape = lawShape(LAW);

  it('projects the selected row per input and remembers the owning service', () => {
    const { record, sources } = materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor);
    expect(record.geboortedatum).toBe('1990-01-01');
    expect(record.inkomen).toBe(2500000);
    expect(sources.inkomen).toBe('BELASTINGDIENST');
  });

  it('collects every matching row for an array input', () => {
    const { record } = materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor);
    expect(record.kinderen).toEqual(['200000001', '200000002']);
  });

  it('treats a missing amount as zero and anything else as unknown', () => {
    const { record } = materialiseRecord(shape, BINDINGS, { bsn: '999' }, rowsFor);
    expect(record.inkomen).toBe(0);
    expect(record.geboortedatum).toBeNull();
    expect(record.kinderen).toEqual([]);
  });

  it('leaves a claim-only input and an unresolvable selector unknown', () => {
    const { record } = materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor);
    expect(record.eigen_verklaring).toBeNull();
    expect(record.partner_inkomen).toBe(0); // amount without a row
  });

  it('asks the resolver for a cross-law input used as selector', () => {
    const bindings = {
      partner_inkomen: { ...BINDINGS.partner_inkomen, select_on: [{ name: 'bsn', value: '$partner_bsn' }] },
    };
    const resolveRef = (lawId, name) => (lawId === 'test_wet' && name === 'partner_bsn' ? '100000001' : undefined);
    const { record } = materialiseRecord(shape, bindings, { bsn: '5' }, rowsFor, { resolveRef });
    expect(record.partner_inkomen).toBe(2500000);
  });

  it('reads kind: cases from the demo case store', () => {
    const bindings = {
      kinderen: {
        kind: 'cases',
        service: 'GEMEENTE',
        select_on: [
          { name: 'status', value: 'DECIDED' },
          { name: 'bsn', value: '$bsn' },
        ],
      },
    };
    const cases = [
      { status: 'DECIDED', bsn: '100000001', id: 'a' },
      { status: 'SUBMITTED', bsn: '100000001', id: 'b' },
      { status: 'DECIDED', bsn: '2', id: 'c' },
    ];
    const { record } = materialiseRecord(shape, bindings, { bsn: '100000001' }, rowsFor, { cases });
    expect(record.kinderen.map((c) => c.id)).toEqual(['a']);
  });
});

describe('materialiseAll', () => {
  it('groups records per law, service and key field', () => {
    const out = materialiseAll({ test_wet: LAW }, { test_wet: BINDINGS }, rowsFor, { bsn: ['100000001', '999'] });
    const rvig = out.find((s) => s.service === 'RvIG');
    expect(rvig).toMatchObject({ law: 'test_wet', keyField: 'bsn' });
    expect(rvig.records).toHaveLength(2);
    expect(rvig.records[0]).toMatchObject({ bsn: '100000001', geboortedatum: '1990-01-01' });
    expect(out.map((s) => s.service).sort()).toEqual(['BELASTINGDIENST', 'RvIG', 'TOESLAGEN']);
  });

  it('skips laws that are not loaded and keys without values', () => {
    expect(materialiseAll({}, { test_wet: BINDINGS }, rowsFor, { bsn: ['1'] })).toEqual([]);
    expect(materialiseAll({ test_wet: LAW }, { test_wet: BINDINGS }, rowsFor, {})).toEqual([]);
  });
});

describe('tablesFromProfiles / collectKeyValues', () => {
  const profiles = {
    globalServices: { CBS: { levensverwachting: [{ jaar: 2025, waarde: 20.5 }] } },
    profiles: {
      '100000001': { sources: { RvIG: { personen: [{ bsn: '100000001' }] } } },
      '999999990': { sources: { RvIG: { personen: [{ bsn: '999999990' }] }, KVK: { inschrijvingen: [{ kvk_nummer: '85234567' }] } } },
    },
  };

  it('concatenates the rows of every persona per table', () => {
    const rowsFor = tablesFromProfiles(profiles);
    expect(rowsFor('RvIG', 'personen')).toHaveLength(2);
    expect(rowsFor('CBS', 'levensverwachting')).toHaveLength(1);
    expect(rowsFor('KVK', 'onbekend')).toEqual([]);
  });

  it('collects the distinct key values as strings', () => {
    const rowsFor = tablesFromProfiles(profiles);
    const all = [rowsFor('RvIG', 'personen'), rowsFor('KVK', 'inschrijvingen')];
    expect(collectKeyValues(all, ['bsn', 'kvk_nummer'])).toEqual({
      bsn: ['100000001', '999999990'],
      kvk_nummer: ['85234567'],
    });
  });
});

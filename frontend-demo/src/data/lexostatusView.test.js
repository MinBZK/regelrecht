import { describe, expect, it } from 'vitest';
import { derivationRows, filterRows, policyArticles, readingGrams, readingRows, readsPerCase } from './lexostatusView.js';

describe('de lexostatussen van een cel', () => {
  it('leest een lexostatus per zaak alleen als haar enige invoer root is', () => {
    expect(readsPerCase({ inputs: ['root'] })).toBe(true);
    expect(readsPerCase({ inputs: [] })).toBe(false);
    expect(readsPerCase({ inputs: ['root', 'jaar'] })).toBe(false);
    expect(readsPerCase({})).toBe(false);
  });

  it('geeft het filter per kenmerk, met de invoer bij een $-waarde', () => {
    expect(filterRows({ type: 'submission', root: '$root', stage: null })).toEqual([
      { key: 'type', value: 'submission', input: null },
      { key: 'root', value: '$root', input: 'root' },
    ]);
    expect(filterRows(undefined)).toEqual([]);
  });

  it('geeft per afleiding de regel, wat zij leest en de rechtsgrond', () => {
    const rows = derivationRows({
      a: { field: 'x', legal_basis: ['w#1 lid 2'] },
      b: { moment: 'effective_at' },
      c: { filled: 'y', legal_basis: [] },
      d: { sum: 'z', legal_basis: ['w#3'] },
      e: { period: 'year' },
      f: {},
    });
    expect(rows).toEqual([
      { name: 'a', rule: 'field', of: 'x', legalBasis: ['w#1 lid 2'] },
      { name: 'b', rule: 'moment', of: 'effective_at', legalBasis: [] },
      { name: 'c', rule: 'filled', of: 'y', legalBasis: [] },
      { name: 'd', rule: 'sum', of: 'z', legalBasis: ['w#3'] },
      { name: 'e', rule: 'period', of: 'year', legalBasis: [] },
      { name: 'f', rule: null, of: null, legalBasis: [] },
    ]);
  });

  it('geeft de artikelen van een beleid met de uitvoer zoals het regelwerk die beschrijft', () => {
    const doc = {
      articles: [{ number: '1', machine_readable: { execution: { output: [{ name: 'bedrag', description: 'Het bedrag.' }] } } }],
    };
    const description = { name: 'beleid', articles: [{ number: '1', outputs: ['bedrag', 'hulp'] }, { number: '2', outputs: [] }] };
    expect(policyArticles(description, doc)).toEqual([
      {
        number: '1',
        provision: 'beleid#1',
        outputs: [
          { name: 'bedrag', description: 'Het bedrag.' },
          { name: 'hulp', description: '' },
        ],
      },
      { number: '2', provision: 'beleid#2', outputs: [] },
    ]);
  });

  it('leest een uitvoer van een beleid als de declaratie in dat beleid', () => {
    const doc = {
      articles: [
        {
          number: '1',
          machine_readable: {
            execution: {
              output: [
                { name: 'bedrag', type: 'amount', type_spec: { unit: 'eurocent' } },
                { name: 'jaar_van_beleid', type: 'number' },
              ],
            },
          },
        },
      ],
    };
    const corpus = { lawById: (id) => (id === 'beleid' ? { doc } : null) };
    const reading = {
      values: {
        bedrag: { value: 12345, provenance: { source: 'lexostatus', article: 'beleid#1' } },
        jaar: { value: 2025, provenance: { source: 'lexostatus', lexostatus: 'aanvraag' } },
        jaar_van_beleid: { value: 2025, provenance: { source: 'lexostatus', article: 'beleid#1' } },
      },
      grams: [],
    };
    const rows = readingRows(reading, { jaar: { name: 'jaar', type: 'number' } }, corpus);
    expect(rows.map((r) => [r.name, r.article])).toEqual([
      ['bedrag', 'beleid#1'],
      ['jaar', null],
      ['jaar_van_beleid', 'beleid#1'],
    ]);
    expect(rows[0].text).toContain('123,45');
    expect(rows[1].text).toBe('2025');
    expect(rows[2].text).toBe('2025');
  });

  it('zet bij elke gram van een lezing zijn plaats in de kroniek', () => {
    const entries = [
      { gram: { id: 'a', name: 'aanvraag' }, position: 1 },
      { gram: { id: 'b', name: 'besluit' }, position: 2 },
    ];
    expect(readingGrams(['b', 'x'], entries)).toEqual([
      { id: 'b', position: 2, name: 'besluit' },
      { id: 'x', position: null, name: null },
    ]);
  });
});

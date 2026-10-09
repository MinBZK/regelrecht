import { describe, expect, it } from 'vitest';
import {
  derivationRows,
  filterRows,
  givesText,
  gramKind,
  listText,
  originText,
  periodsOf,
  policyArticles,
  readerOf,
  readersNoun,
  readingGrams,
  readingInputs,
  readingPeriods,
  readingRows,
  readsPerCase,
  sourceNoun,
  sourceText,
  usesOf,
  valueRows,
} from './lexostatusView.js';

describe('de lexostatussen van een cel', () => {
  it('leest een lexostatus per zaak als haar invoer root is, en hooguit haar periode', () => {
    expect(readsPerCase({ inputs: ['root'] })).toBe(true);
    expect(readsPerCase({ inputs: [] })).toBe(false);
    expect(readsPerCase({ inputs: ['root', 'jaar'] })).toBe(false);
    expect(readsPerCase({ inputs: ['root', 'jaar'], period: 'jaar' })).toBe(true);
    expect(readsPerCase({ inputs: ['jaar'], period: 'jaar' })).toBe(false);
    expect(readsPerCase({})).toBe(false);
  });

  it('leest per periode van de zaak als de lexostatus per periode leest', () => {
    const grams = [{ period: null }, { period: { unit: 'year', value: 2026 } }, {}, { period: { unit: 'year', value: 2025 } }, { period: { unit: 'year', value: 2026 } }];
    expect(periodsOf(grams)).toEqual([2025, 2026]);
    expect(periodsOf(undefined)).toEqual([]);
    expect(readingPeriods({ period: 'jaar' }, grams)).toEqual([2025, 2026]);
    expect(readingPeriods({}, grams)).toEqual([null]);
    expect(readingInputs({ period: 'jaar' }, 'a1', 2025)).toEqual({ root: 'a1', jaar: 2025 });
    expect(readingInputs({}, 'a1', 2025)).toEqual({ root: 'a1' });
    expect(readingInputs({ period: 'jaar' }, 'a1')).toEqual({ root: 'a1' });
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
      { gram: { id: 'b', name: 'besluit', effective_at: '2025-01-02T09:00:00+01:00' }, position: 2 },
    ];
    expect(readingGrams(['b', 'x'], entries)).toEqual([
      { id: 'b', position: 2, name: 'besluit', date: '2025-01-02' },
      { id: 'x', position: null, name: null, date: null },
    ]);
  });
});

describe('een lexostatus in gewone woorden', () => {
  // Een cel met een aanvraag, een besluit dat haar leest en een uitvoering:
  // verzonnen namen, zodat de test laat zien dat de woorden uit de
  // beschrijving komen en niet uit de demo.
  const aanvraag = {
    kind: 'configuration',
    name: 'melding',
    inputs: ['root'],
    reduction: {
      chronicle: 'k',
      filter: { type: 'submission', subtype: 'melding', root: '$root' },
      pick: 'latest',
      derivations: {
        nummer: { field: 'nummer', legal_basis: ['wet#3 lid 1'] },
        ontvangen_op: { moment: 'effective_at', legal_basis: [] },
      },
    },
    read_by: [{ event: 'hulp_verleend', stream: 's', stage: 'HULP' }],
  };
  const betaald = {
    kind: 'configuration',
    name: 'betaald',
    inputs: ['root', 'jaar'],
    period: 'jaar',
    reduction: {
      chronicle: 'k',
      filter: { type: 'executogram', event: 'termijn_betaald', root: '$root', period: '$jaar' },
      pick: 'all',
      derivations: { betaald_totaal: { sum: 'betaald_bedrag', legal_basis: ['wet#9'] } },
    },
    read_by: [],
  };
  const docs = {
    wet: {
      articles: [
        { number: '2', machine_readable: { execution: { parameters: [{ name: 'nummer' }] } } },
        { number: '5', machine_readable: { execution: { parameters: [{ name: 'ontvangen_op' }, { name: 'nummer' }] } } },
      ],
    },
    beleid: {
      articles: [{ number: '1', machine_readable: { execution: { output: [{ name: 'schatting', description: 'Wat de aanvrager verwacht.' }] } } }],
    },
  };
  const lawDoc = (id) => docs[id] ?? null;
  const shape = { type: 'decretogram', establishes: 'wet#2', fields: [{ name: 'a', declared_by: 'wet#2' }, { name: 'b', declared_by: 'wet#5' }] };
  const reader = readerOf(aanvraag.read_by[0], shape);

  it('kent de soort van een gram', () => {
    expect([gramKind('submission'), gramKind('decretogram'), gramKind('executogram'), gramKind('x')]).toEqual(['submission', 'decision', 'execution', null]);
  });

  it('noemt wie haar leest bij naam, met de artikelen die aan die gebeurtenis meedoen', () => {
    expect(reader).toEqual({ event: 'hulp_verleend', label: 'Hulp verleend', kind: 'decision', articles: ['wet#2', 'wet#5'] });
    expect(readerOf({ event: 'x' }, null)).toEqual({ event: 'x', label: 'X', kind: null, articles: [] });
  });

  it('somt op zoals in een zin', () => {
    expect(listText([])).toBe('');
    expect(listText(['a'])).toBe('a');
    expect(listText(['a', 'b', 'c'])).toBe('a, b en c');
  });

  it('noemt de lezers naar hun soort', () => {
    expect(readersNoun([reader])).toBe('het besluit');
    expect(readersNoun([reader, reader])).toBe('de besluiten');
    expect(readersNoun([reader, { kind: 'execution' }])).toBe('het besluit en de uitvoering');
    expect(readersNoun([])).toBe('');
  });

  it('noemt de bron naar de soort gram, of de gebeurtenis als het filter die noemt', () => {
    expect(sourceNoun(aanvraag.reduction.filter)).toBe('de melding');
    expect(sourceNoun({ type: 'decretogram' })).toBe('het besluit');
    expect(sourceNoun(betaald.reduction.filter, { each: true })).toBe('elke “Termijn betaald”');
  });

  it('zegt wat zij geeft en aan wie', () => {
    expect(givesText(aanvraag, [reader])).toBe('Geeft het besluit 2 gegevens uit de melding');
    expect(givesText(betaald, [])).toBe('1 gegeven uit “Termijn betaald”, dat nog niets leest');
    expect(givesText({ kind: 'policy', articles: [{ outputs: ['a', 'b'] }] }, [reader])).toBe('Geeft het besluit 2 gegevens uit de kroniek van de zaak');
  });

  it('zegt waar de cel het haalt', () => {
    expect(sourceText(aanvraag, 'Dienst X')).toBe('Dienst X haalt dit uit haar eigen kroniek: de melding van deze zaak, de laatste versie die geldt.');
    expect(sourceText(betaald, 'Dienst X')).toBe('Dienst X haalt dit uit haar eigen kroniek: elke “Termijn betaald” van deze zaak, per jaar, allemaal, voor zover ze gelden.');
    expect(sourceText({ kind: 'policy' }, 'Dienst X')).toBe('Dienst X leest dit volgens haar beleid:');
  });

  it('zegt per gegeven waar het vandaan komt', () => {
    const filter = aanvraag.reduction.filter;
    expect(originText({ rule: 'field', of: 'nummer' }, filter)).toBe('ingevuld in de melding');
    expect(originText({ rule: 'field', of: 'x' }, { type: 'decretogram' })).toBe('zoals vastgelegd in het besluit');
    expect(originText({ rule: 'moment', of: 'effective_at' }, filter)).toBe('de dag waarop de melding binnenkwam');
    expect(originText({ rule: 'filled', of: 'handtekening' }, filter)).toBe('of handtekening is ingevuld in de melding');
    expect(originText({ rule: 'sum', of: 'betaald_bedrag' }, betaald.reduction.filter)).toBe('Betaald bedrag, opgeteld over elke “Termijn betaald”');
    expect(originText(null, null, 'Wat de aanvrager verwacht.')).toBe('Wat de aanvrager verwacht.');
  });

  it('vindt de parameter met dezelfde naam in de artikelen van wie haar leest', () => {
    expect(usesOf('nummer', [reader], lawDoc)).toEqual([
      { provision: 'wet#2', readers: ['Hulp verleend'] },
      { provision: 'wet#5', readers: ['Hulp verleend'] },
    ]);
    expect(usesOf('ontvangen_op', [reader], lawDoc)).toEqual([{ provision: 'wet#5', readers: ['Hulp verleend'] }]);
    expect(usesOf('onbekend', [reader], lawDoc)).toEqual([]);
  });

  it('geeft per gegeven een regel voor de tabel', () => {
    expect(valueRows(aanvraag, [reader], lawDoc)).toEqual([
      { name: 'nummer', label: 'Nummer', origin: 'ingevuld in de melding', uses: usesOf('nummer', [reader], lawDoc), basis: ['wet#3 lid 1'] },
      { name: 'ontvangen_op', label: 'Ontvangen op', origin: 'de dag waarop de melding binnenkwam', uses: usesOf('ontvangen_op', [reader], lawDoc), basis: [] },
    ]);
    const policy = { kind: 'policy', name: 'beleid', articles: [{ number: '1', outputs: ['schatting'] }] };
    expect(valueRows(policy, [], lawDoc)).toEqual([{ name: 'schatting', label: 'Schatting', origin: 'Wat de aanvrager verwacht.', uses: [], basis: ['beleid#1'] }]);
  });
});

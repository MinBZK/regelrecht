import { describe, expect, it } from 'vitest';
import {
  givesText,
  gramKind,
  listText,
  originText,
  periodsOf,
  readerOf,
  readingGrams,
  readingInputs,
  readingPeriods,
  readingRows,
  readsPerCase,
  shapeText,
  sourceText,
  titleText,
  typeText,
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
  // Een cel met een melding, een besluit dat haar leest en een artikel van
  // het beleid: verzonnen namen, zodat de test laat zien dat de woorden uit
  // de beschrijving komen en niet uit de demo.
  const melding = {
    kind: 'submission',
    name: 'melding',
    provision: 'wet#1',
    event: 'melding_ontvangen',
    chronicle: 'k',
    inputs: ['root'],
    fields: [
      { name: 'nummer', type: 'string', legal_basis: ['wet#3 lid 1'], declared_by: 'wet#1' },
      { name: 'ontvangen_op', type: 'date', legal_basis: ['wet#4'], declared_by: 'wet#1', moment: true },
    ],
    read_by: [{ event: 'hulp_verleend', stream: 's', stage: 'HULP' }],
  };
  const betaald = {
    kind: 'policy',
    name: 'betaald',
    provision: 'beleid#2a',
    policy: 'beleid',
    article: '2a',
    inputs: ['root', 'jaar'],
    period: 'jaar',
    outputs: ['hulp', 'betaald_totaal'],
    fields: [{ name: 'betaald_totaal', type: 'amount', unit: 'eurocent', legal_basis: ['beleid#2a'], declared_by: 'beleid#2a', description: 'Wat er is betaald.' }],
    read_by: [],
  };
  const docs = {
    wet: {
      articles: [
        { number: '2', machine_readable: { execution: { parameters: [{ name: 'nummer' }] } } },
        { number: '5', machine_readable: { execution: { parameters: [{ name: 'ontvangen_op' }, { name: 'nummer' }] } } },
      ],
    },
  };
  const lawDoc = (id) => docs[id] ?? null;
  const shape = { type: 'decretogram', establishes: 'wet#2', fields: [{ name: 'a', declared_by: 'wet#2' }, { name: 'b', declared_by: 'wet#5' }] };
  const reader = readerOf(melding.read_by[0], shape);

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

  it('zegt wat zij is: wat er in de aanvraag staat, of de gegevens van het artikel', () => {
    expect(titleText(melding)).toBe('Wat er in de melding staat');
    expect(titleText(betaald)).toBe('Betaald totaal');
    expect(titleText({ ...betaald, fields: [...betaald.fields, { name: 'dag_van_betalen' }] })).toBe('Betaald totaal en dag van betalen');
  });

  it('zegt wat zij geeft en aan wie', () => {
    expect(givesText(melding, [reader])).toBe('Geeft “Hulp verleend” 2 gegevens: nummer en ontvangen op.');
    expect(givesText(betaald, [])).toBe('1 gegeven: betaald totaal. Nog niets leest het.');
  });

  it('geeft de vorm als klein schema, met type en eenheid', () => {
    expect(typeText({ type: 'amount', unit: 'eurocent' })).toBe('bedrag in eurocent');
    expect(typeText({})).toBe('onbekend type');
    expect(shapeText(melding)).toBe('{ nummer: tekst, ontvangen_op: datum }');
    expect(shapeText(betaald)).toBe('{ betaald_totaal: bedrag in eurocent }');
    expect(shapeText({})).toBe('{ }');
  });

  it('zegt hoe de cel haar afleidt: uit de wet, of met het beleid', () => {
    expect(sourceText(melding, 'Dienst X')).toBe('Afgeleid uit de wet: wat de besluiten vragen en de melding bevat, zoals de wet de melding beschrijft.');
    expect(sourceText(betaald, 'Dienst X')).toBe('Een artikel in het beleid van Dienst X, dat haar kroniek als register leest.');
  });

  it('zegt per gegeven hoe de cel het afleidt', () => {
    expect(originText(melding, melding.fields[0])).toBe('ingevuld in de melding');
    expect(originText(melding, melding.fields[1])).toBe('de dag waarop de melding binnenkwam');
    expect(originText(betaald, betaald.fields[0])).toBe('Wat er is betaald.');
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
    expect(valueRows(melding, [reader], lawDoc)).toEqual([
      { name: 'nummer', label: 'Nummer', type: 'tekst', origin: 'ingevuld in de melding', uses: usesOf('nummer', [reader], lawDoc), read: true, basis: ['wet#3 lid 1'] },
      { name: 'ontvangen_op', label: 'Ontvangen op', type: 'datum', origin: 'de dag waarop de melding binnenkwam', uses: usesOf('ontvangen_op', [reader], lawDoc), read: true, basis: ['wet#4'] },
    ]);
    expect(valueRows(betaald, [], lawDoc)).toEqual([
      { name: 'betaald_totaal', label: 'Betaald totaal', type: 'bedrag in eurocent', origin: 'Wat er is betaald.', uses: [], read: true, basis: ['beleid#2a'] },
    ]);
  });

  // Een aanvraag geeft alles wat de wet erin declareert, ook wat geen besluit
  // leest; de cel markeert wat er gelezen wordt (`read`).
  const volledig = {
    ...melding,
    fields: [
      { ...melding.fields[0], read: true },
      { name: 'naam', type: 'string', legal_basis: ['wet#3 lid 2'], declared_by: 'wet#3', read: false },
      { name: 'gevraagd', type: 'string', legal_basis: ['wet#3 lid 3'], declared_by: 'wet#3', fixed: 'wet#2', read: false },
      { name: 'zonder_grond', type: 'string', legal_basis: [], declared_by: 'wet#1', read: false },
      { ...melding.fields[1], read: true },
    ],
  };

  it('telt bij een volledige aanvraag apart wat geen besluit leest', () => {
    expect(givesText(volledig, [reader])).toBe('Geeft “Hulp verleend” 2 gegevens: nummer en ontvangen op. Daarnaast 3 gegevens die geen besluit hier leest.');
    expect(givesText({ ...volledig, fields: volledig.fields.slice(0, 2) }, [reader])).toBe('Geeft “Hulp verleend” 1 gegeven: nummer. Daarnaast 1 gegeven dat geen besluit hier leest.');
  });

  it('zegt bij het gevraagde besluit dat de cel het invult', () => {
    expect(originText(volledig, volledig.fields[2])).toBe('vult de cel in: het besluit dat op de melding wordt genomen');
  });

  it('markeert per regel of de cel het gegeven hier leest, en laat een ontbrekende grondslag leeg', () => {
    const rows = valueRows(volledig, [reader], lawDoc);
    expect(rows.map((r) => [r.name, r.read])).toEqual([
      ['nummer', true],
      ['naam', false],
      ['gevraagd', false],
      ['zonder_grond', false],
      ['ontvangen_op', true],
    ]);
    expect(rows[3].basis).toEqual([]);
  });
});

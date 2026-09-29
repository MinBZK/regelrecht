import { describe, expect, it } from 'vitest';
import { bedragTekst, bronStatusTekst, herkomstRijen, herkomstTekst, routesUit, routeTekst, uitkomstTekst, waardeTekst } from './tekst.js';

describe('waardeTekst', () => {
  it('schrijft ja/nee, leeg en null uit', () => {
    expect(waardeTekst(true)).toBe('ja');
    expect(waardeTekst(false)).toBe('nee');
    expect(waardeTekst(undefined)).toBe('');
    expect(waardeTekst(null)).toBe('geen (null)');
  });

  it('laat tekst staan en zet de rest om naar JSON', () => {
    expect(waardeTekst('abc')).toBe('abc');
    expect(waardeTekst(12)).toBe('12');
    expect(waardeTekst({ a: 1 })).toBe('{"a":1}');
  });
});

describe('herkomstTekst', () => {
  // Eén geval per variant van Herkomst in packages/cel/src/synthese.rs, zoals
  // serde hem serialiseert (tag `source`, snake_case).
  const varianten = [
    [{ source: 'own', lexostatus: 'aanvraag' }, 'eigen lexostatus aanvraag'],
    [
      { source: 'cell', cell: 'register', lexostatus: 'inschrijving', transport: 'http' },
      'cel register, lexostatus inschrijving (http)',
    ],
    [
      { source: 'per_row', lexostatus: 'aanvraag', field: 'leden' },
      'per regel uit leden van eigen lexostatus aanvraag',
    ],
    [{ source: 'handler' }, 'behandelaar (formulier van de handeling)'],
    [{ source: 'state_at_decision' }, 'stand bij besluit'],
    [{ source: 'state_at_decision', stage: 'BEKENDMAKING' }, 'stand bij besluit (ontstaat pas in stage BEKENDMAKING)'],
    [{ source: 'choice' }, 'keuze van de aanvrager (portaal)'],
  ];

  it.each(varianten)('%o', (herkomst, tekst) => {
    expect(herkomstTekst(herkomst)).toBe(tekst);
  });

  it('valt voor elke bekende variant niet terug op JSON', () => {
    for (const [herkomst] of varianten) {
      expect(herkomstTekst(herkomst)).not.toBe(JSON.stringify(herkomst));
    }
  });

  it('toont een onbekende variant als JSON', () => {
    expect(herkomstTekst({ source: 'nieuw' })).toBe('{"source":"nieuw"}');
  });
});

describe('herkomstRijen', () => {
  it('geeft per parameter naam, waarde en bron', () => {
    const rijen = herkomstRijen({ bedrag: 10, ok: true }, {
      bedrag: { source: 'handler' },
      ok: { source: 'choice' },
    });
    expect(rijen).toEqual([
      { naam: 'bedrag', waarde: '10', bron: 'behandelaar (formulier van de handeling)' },
      { naam: 'ok', waarde: 'ja', bron: 'keuze van de aanvrager (portaal)' },
    ]);
  });

  it('is leeg zonder herkomst', () => {
    expect(herkomstRijen(undefined, undefined)).toEqual([]);
  });
});

describe('bedragTekst en uitkomstTekst', () => {
  const tekst = (t) => t.replace(/\s/g, ' ');

  it('schrijft een bedrag in de eenheid van de regeling', () => {
    expect(tekst(bedragTekst(1913600, 'eurocent'))).toBe('€ 19.136,00');
    expect(tekst(bedragTekst(0, 'eurocent'))).toBe('€ 0,00');
    expect(tekst(bedragTekst(19136, 'euro'))).toBe('€ 19.136,00');
    expect(bedragTekst(5, 'punten')).toBe('5 punten');
    expect(bedragTekst(5)).toBe('5');
    expect(bedragTekst(null, 'eurocent')).toBe('geen (null)');
  });

  it('kent het type van een uitkomst uit de regeling, niet uit haar naam', () => {
    const bedrag = { type: 'amount', unit: 'eurocent' };
    expect(tekst(uitkomstTekst(100, bedrag))).toBe('€ 1,00');
    // Een naam die op een bedrag lijkt, zonder type: een getal.
    expect(uitkomstTekst(100, undefined)).toBe('100');
    expect(uitkomstTekst(100, { type: 'number' })).toBe('100');
    expect(uitkomstTekst(true, { type: 'boolean' })).toBe('ja');
  });
});

describe('waardeTekst en een onbekende waarde', () => {
  it('noemt wat er mist', () => {
    const onbekend = { __unknown: true, missing: [{ law: 'w', name: 'inkomen', kind: 'parameter' }] };
    expect(waardeTekst(onbekend)).toBe('onbekend (mist inkomen)');
    expect(waardeTekst({ __unknown: true })).toBe('onbekend');
  });
});

describe('route van de reductie (experiment A)', () => {
  it('noemt de engine met de regeling, en de DSL met de reden', () => {
    expect(routeTekst(null)).toBe('');
    expect(routeTekst({ route: 'engine', regulation: 'lexostatus_x', duration_us: 1500 })).toBe(
      'reductie via de engine (lexostatus_x, 1.50 ms)',
    );
    expect(routeTekst({ route: 'dsl', reason: 'een lijst' })).toBe('reductie via de DSL (bewust: een lijst)');
    expect(routeTekst({ route: 'runtime', reason: 'de stand' })).toBe('door de runtime zelf (de stand)');
  });

  it('zet de route achter de herkomst, per eigen lexostatus en per bron', () => {
    const antwoord = {
      lexostatuses: [{ name: 'aanvraag', reduction: { route: 'engine', regulation: 'lexostatus_aanvraag' } }],
      sources: [{ cell: 'register', lexostatus: 'stand', reduction: { route: 'dsl' } }],
    };
    const rijen = herkomstRijen(
      { a: 1, b: true, c: 2 },
      {
        a: { source: 'own', lexostatus: 'aanvraag' },
        b: { source: 'cell', cell: 'register', lexostatus: 'stand', transport: 'internal' },
        c: { source: 'handler' },
      },
      routesUit(antwoord),
    );
    expect(rijen.map((r) => r.bron)).toEqual([
      'eigen lexostatus aanvraag; reductie via de engine (lexostatus_aanvraag)',
      'cel register, lexostatus stand (internal); reductie via de DSL',
      'behandelaar (formulier van de handeling)',
    ]);
  });
});

describe('bronStatusTekst', () => {
  it('zegt de status van een bron in woorden', () => {
    expect(bronStatusTekst('queried')).toBe('bevraagd');
    expect(bronStatusTekst('unreachable')).toBe('onbereikbaar');
    expect(bronStatusTekst('error')).toBe('fout');
    expect(bronStatusTekst('not_queried')).toBe('niet bevraagd');
    expect(bronStatusTekst('iets_nieuws')).toBe('iets nieuws');
  });
});

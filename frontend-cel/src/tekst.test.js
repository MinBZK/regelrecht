import { describe, expect, it } from 'vitest';
import { bedragTekst, herkomstRijen, herkomstTekst, uitkomstTekst, waardeTekst } from './tekst.js';

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
  // serde hem serialiseert (tag `bron`, snake_case).
  const varianten = [
    [{ bron: 'eigen', lexostatus: 'aanvraag' }, 'eigen lexostatus aanvraag'],
    [
      { bron: 'cel', cel: 'register', lexostatus: 'inschrijving', transport: 'http' },
      'cel register, lexostatus inschrijving (http)',
    ],
    [
      { bron: 'per_regel', lexostatus: 'aanvraag', veld: 'leden' },
      'per regel uit leden van eigen lexostatus aanvraag',
    ],
    [{ bron: 'behandelaar' }, 'behandelaar (formulier van de handeling)'],
    [{ bron: 'stand_bij_besluit' }, 'stand bij besluit'],
    [{ bron: 'stand_bij_besluit', stage: 'BEKENDMAKING' }, 'stand bij besluit (ontstaat pas in stage BEKENDMAKING)'],
    [{ bron: 'keuze' }, 'keuze van de aanvrager (portaal)'],
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
    expect(herkomstTekst({ bron: 'nieuw' })).toBe('{"bron":"nieuw"}');
  });
});

describe('herkomstRijen', () => {
  it('geeft per parameter naam, waarde en bron', () => {
    const rijen = herkomstRijen({ bedrag: 10, ok: true }, {
      bedrag: { bron: 'behandelaar' },
      ok: { bron: 'keuze' },
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
    const bedrag = { type: 'amount', eenheid: 'eurocent' };
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

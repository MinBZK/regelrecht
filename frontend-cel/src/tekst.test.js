import { describe, expect, it } from 'vitest';
import { herkomstRijen, herkomstTekst, waardeTekst } from './tekst.js';

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
    [{ bron: 'behandelaar' }, 'behandelaar (besluitformulier)'],
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
      { naam: 'bedrag', waarde: '10', bron: 'behandelaar (besluitformulier)' },
      { naam: 'ok', waarde: 'ja', bron: 'keuze van de aanvrager (portaal)' },
    ]);
  });

  it('is leeg zonder herkomst', () => {
    expect(herkomstRijen(undefined, undefined)).toEqual([]);
  });
});

import { describe, expect, it } from 'vitest';
import { addressOf, applicationValues, eventsForLaw, gramsOfCase, momentOn, provisionLabel, fieldText } from './chronolex.js';

const cell = {
  id: 'toeslagen',
  events: [
    { name: 'aanvraag_ontvangen', establishes: 'awir#15', chronicle: 'toeslagen' },
    { name: 'zorgtoeslag_toegekend', establishes: 'zorgtoeslagwet#2', chronicle: 'toeslagen' },
  ],
};
const zorgtoeslag = {
  $id: 'zorgtoeslagwet',
  articles: [{ number: '2', machine_readable: { execution: { produces: { decides_on: ['awir#15'] } } } }],
};

describe('eventsForLaw', () => {
  it('vindt het besluit en de aanvraag waarop het wordt genomen', () => {
    const found = eventsForLaw([cell], zorgtoeslag);
    expect(found.decision.name).toBe('zorgtoeslag_toegekend');
    expect(found.application.name).toBe('aanvraag_ontvangen');
  });

  it('geeft niets voor een wet zonder besluit in een kroniek', () => {
    expect(eventsForLaw([cell], { $id: 'huurtoeslag', articles: [] })).toBeNull();
    expect(eventsForLaw([cell], null)).toBeNull();
  });
});

describe('applicationValues', () => {
  const shape = {
    fields: [
      { name: 'bsn' },
      { name: 'aangevraagd_berekeningsjaar' },
      { name: 'adres_aanvrager' },
      { name: 'gevraagde_beschikking', fixed: 'zorgtoeslagwet#2' },
      { name: 'onbekend_veld' },
    ],
  };

  it('vult in wat de demo weet en laat de rest weg', () => {
    expect(applicationValues(shape, { bsn: '999100001', name: 'M', date: '2025-03-04' })).toEqual({
      bsn: '999100001',
      aangevraagd_berekeningsjaar: 2025,
    });
  });
});

describe('addressOf', () => {
  it('maakt één regel van het woonadres', () => {
    const persona = {
      sources: {
        RvIG: {
          verblijfplaats: [
            { type: 'WOONADRES', straat: 'Meeuwenlaan', huisnummer: '28', postcode: '1021HS', woonplaats: 'Amsterdam' },
          ],
        },
      },
    };
    expect(addressOf(persona)).toBe('Meeuwenlaan 28, 1021HS Amsterdam');
    expect(addressOf({})).toBeUndefined();
  });
});

describe('momentOn', () => {
  it('zet de tijd van nu op de gevraagde datum, met de tijdzone van de browser', () => {
    const m = momentOn('2025-03-04', new Date(2026, 9, 6, 9, 5, 7));
    expect(m).toMatch(/^2025-03-04T09:05:07[+-]\d\d:\d\d$/);
  });
});

describe('gramsOfCase', () => {
  it('geeft de aanvraag en wat ernaar verwijst', () => {
    const grams = [
      { id: 'a' },
      { id: 'b', refers_to: { on_application: 'a' } },
      { id: 'c', refers_to: { on_application: 'x' } },
    ];
    expect(gramsOfCase(grams, 'a').map((g) => g.id)).toEqual(['a', 'b']);
    expect(gramsOfCase(grams, null)).toEqual([]);
  });
});

describe('provisionLabel', () => {
  it('noemt de wet bij naam en het artikel met lid', () => {
    const corpus = { lawById: (id) => (id === 'algemene_wet_bestuursrecht' ? { name: 'Algemene wet bestuursrecht' } : null) };
    expect(provisionLabel(corpus, 'algemene_wet_bestuursrecht#4:13 lid 1')).toBe('Algemene wet bestuursrecht, art. 4:13 lid 1');
    expect(provisionLabel(corpus, 'onbekend#3')).toBe('onbekend, art. 3');
  });
});

describe('fieldText', () => {
  it('laat een jaartal een jaartal', () => {
    expect(fieldText('aangevraagd_berekeningsjaar', 2026)).toBe('2026');
  });

  it('noemt een bepaling bij naam', () => {
    const corpus = { lawById: () => ({ name: 'Zorgtoeslag' }) };
    expect(fieldText('gevraagde_beschikking', 'zorgtoeslagwet#2', null, corpus)).toBe('Zorgtoeslag, art. 2');
  });
});

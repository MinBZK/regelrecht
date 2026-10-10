import { describe, expect, it } from 'vitest';
import { applicationValues, eventsForLaw, gramsOfCase, momentOn, provisionLabel, fieldText } from './chronolex.js';

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
      { name: 'naam_aanvrager' },
      { name: 'adres_aanvrager' },
      { name: 'gevraagde_beschikking', fixed: 'zorgtoeslagwet#2' },
      { name: 'onbekend_veld' },
    ],
  };
  const configured = {
    bsn: '$bsn',
    aangevraagd_berekeningsjaar: '$reference_year',
    naam_aanvrager: 'M. de Vries',
    gevraagde_beschikking: 'iets anders',
    niet_gevraagd: 'x',
  };
  const tokens = { bsn: '999100001', reference_year: 2025, reference_date: '2025-03-04' };

  it('vult per veld van de wet de geconfigureerde waarde in, met de tokens opgelost', () => {
    expect(applicationValues(shape, configured, tokens)).toEqual({
      bsn: '999100001',
      aangevraagd_berekeningsjaar: 2025,
      naam_aanvrager: 'M. de Vries',
    });
  });

  it('laat een veld weg zonder waarde, en een onbekend token ook', () => {
    expect(applicationValues(shape, { bsn: '$onbekend' }, tokens)).toEqual({});
    expect(applicationValues(shape, undefined, tokens)).toEqual({});
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
  const corpus = { lawById: () => ({ name: 'Zorgtoeslag' }) };

  it('laat een geheel getal zonder eenheid een getal, zonder groepering', () => {
    expect(fieldText(2026, { type: 'number' })).toBe('2026');
  });

  it('noemt een vastgezet veld bij naam van de bepaling', () => {
    expect(fieldText('zorgtoeslagwet#2', { type: 'string', fixed: 'zorgtoeslagwet#2' }, null, corpus)).toBe('Zorgtoeslag, art. 2');
  });

  it('kijkt niet naar de naam van het veld', () => {
    expect(fieldText('zorgtoeslagwet#2', { type: 'string' })).toBe(fieldText('zorgtoeslagwet#2'));
  });

  it('volgt de eenheid uit de declaratie van de wet', () => {
    expect(fieldText(157731, { type: 'number' }, { type: 'amount', type_spec: { unit: 'eurocent' } })).toBe('€\u00a01.577,31');
  });
});

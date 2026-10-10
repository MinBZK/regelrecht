import { describe, expect, it } from 'vitest';
import { applicationValues, cellsOfService, eventsForLaw, gramsOfCase, momentOn, provisionLabel, provisionTarget, externalArticleUrl, fieldText } from './chronolex.js';

const cell = {
  id: 'toeslagen',
  events: [
    { name: 'aanvraag_ontvangen', establishes: 'awir#15', chronicle: 'toeslagen' },
    { name: 'voorschot_verleend', establishes: 'zorgtoeslagwet#2', chronicle: 'toeslagen' },
    { name: 'zorgtoeslag_toegekend', establishes: 'zorgtoeslagwet#2', chronicle: 'toeslagen' },
  ],
};
const zorgtoeslag = {
  $id: 'zorgtoeslagwet',
  articles: [{ number: '2', machine_readable: { execution: { produces: { decides_on: ['awir#15'] } } } }],
};

describe('eventsForLaw', () => {
  it('vindt de besluiten en de aanvraag waarop ze worden genomen', () => {
    // De Zorgtoeslagwet in de procedure van de Awir neemt twee besluiten op
    // dezelfde aanvraag: het voorschot en de toekenning.
    const found = eventsForLaw([cell], zorgtoeslag);
    expect(found.decisions.map((d) => d.name)).toEqual(['voorschot_verleend', 'zorgtoeslag_toegekend']);
    expect(found.application.name).toBe('aanvraag_ontvangen');
  });

  it('geeft niets voor een wet zonder besluit in een kroniek', () => {
    expect(eventsForLaw([cell], { $id: 'huurtoeslag', articles: [] })).toBeNull();
    expect(eventsForLaw([cell], null)).toBeNull();
  });
});

describe('cellsOfService', () => {
  it('geeft elke cel die een wet van de organisatie uitvoert, één keer', () => {
    const law = (id, service, doc) => ({ id, service, doc });
    const corpus = {
      cells: [cell],
      latestById: new Map([
        ['zorgtoeslagwet', law('zorgtoeslagwet', 'toeslagen', zorgtoeslag)],
        ['nog_een', law('nog_een', 'toeslagen', zorgtoeslag)],
        ['huurtoeslag', law('huurtoeslag', 'toeslagen', { $id: 'huurtoeslag', articles: [] })],
        ['elders', law('elders', 'ander', zorgtoeslag)],
      ]),
    };
    expect(cellsOfService(corpus, 'toeslagen')).toEqual([cell]);
    expect(cellsOfService(corpus, 'niemand')).toEqual([]);
    expect(cellsOfService(null, 'toeslagen')).toEqual([]);
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

  it('geeft ook wat via een andere gram van de zaak verwijst, in de volgorde van de kroniek', () => {
    const grams = [
      { id: 'a' },
      { id: 't', refers_to: { voorschot: 'v' } },
      { id: 'v', refers_to: { on_application: 'a' } },
      { id: 'u', refers_to: { voorschot: 'w' } },
    ];
    expect(gramsOfCase(grams, 'a').map((g) => g.id)).toEqual(['a', 't', 'v']);
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

  it('leest zonder declaratie een bedrag naar het type van het veld', () => {
    // Het geschatte inkomen vraagt Awir 16, niet het besluitartikel.
    expect(fieldText(2200000, { type: 'amount' })).toBe('€\u00a022.000,00');
  });
});

describe('waar een bepaling in de demo staat', () => {
  const corpus = { lawById: (id) => (id === 'awb' ? { id } : null) };
  it('wijst een lid naar zijn artikel', () => {
    expect(provisionTarget(corpus, 'awb#4:13 lid 1')).toEqual({ lawId: 'awb', article: '4:13' });
  });
  it('wijst een wet zonder artikel naar de wet', () => {
    expect(provisionTarget(corpus, 'awb')).toEqual({ lawId: 'awb', article: null });
  });
  it('geeft niets voor een wet die het corpus niet kent', () => {
    expect(provisionTarget(corpus, 'onbekend#1')).toBeNull();
  });
  it('wijst een artikel dat niet in de demo staat naar wetten.overheid.nl', () => {
    const doc = { url: 'https://wetten.overheid.nl/BWBR0018472/2025-01-01', articles: [{ number: '15' }] };
    const withDoc = { lawById: (id) => (id === 'awir' ? { id, doc } : null) };
    expect(provisionTarget(withDoc, 'awir#15 lid 1')).toEqual({ lawId: 'awir', article: '15' });
    expect(provisionTarget(withDoc, 'awir#13')).toEqual({ lawId: 'awir', article: '13', external: 'https://wetten.overheid.nl/jci1.3:c:BWBR0018472&artikel=13' });
    const noUrl = { lawById: () => ({ doc: { articles: [] } }) };
    expect(provisionTarget(noUrl, 'awir#13')).toEqual({ lawId: 'awir', article: null });
  });
  it('laat een anker in de url van de wet weg en neemt bwb_id als die er is', () => {
    const anchored = { url: 'https://wetten.overheid.nl/BWBR0018472/2025-01-01#Artikel19', articles: [{ number: '19' }] };
    expect(externalArticleUrl(anchored, '13')).toBe('https://wetten.overheid.nl/jci1.3:c:BWBR0018472&artikel=13');
    expect(externalArticleUrl({ bwb_id: 'BWBR0002320', url: 'https://example.org' }, '21e', '2025-06-01')).toBe(
      'https://wetten.overheid.nl/jci1.3:c:BWBR0002320&artikel=21e&z=2025-06-01&g=2025-06-01',
    );
    expect(externalArticleUrl({ url: 'https://example.org/wet' }, '1')).toBeNull();
  });
  it('neemt de versie van de wet die op de peildatum geldt, en de tekst van die dag', () => {
    const v2025 = { id: 'awir', valid_from: '2025-01-01', doc: { bwb_id: 'BWBR0018472', valid_from: '2025-01-01', articles: [{ number: '15' }] } };
    const v2026 = { id: 'awir', valid_from: '2026-01-01', doc: { bwb_id: 'BWBR0018472', valid_from: '2026-01-01', articles: [{ number: '15' }, { number: '13' }] } };
    const versions = { lawById: () => v2026, lawOn: (id, date) => (date < '2026-01-01' ? v2025 : v2026) };
    // In 2025 staat art. 13 niet in de demo: naar wetten.overheid.nl, de tekst van die dag.
    expect(provisionTarget(versions, 'awir#13', '2025-07-01')).toEqual({
      lawId: 'awir',
      article: '13',
      external: 'https://wetten.overheid.nl/jci1.3:c:BWBR0018472&artikel=13&z=2025-07-01&g=2025-07-01',
    });
    // In 2026 wel: binnen de demo.
    expect(provisionTarget(versions, 'awir#13', '2026-02-01')).toEqual({ lawId: 'awir', article: '13' });
  });
});

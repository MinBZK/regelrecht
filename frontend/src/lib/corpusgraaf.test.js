import { describe, it, expect } from 'vitest';
import { bouwGraaf, outputsVan, openTermsVan } from './corpusgraaf.js';

// Het repo-corpus is schoon (clean=23, alle klassen 0), dus de classificatie
// is daar niet op te bewijzen. Deze fixtures zijn casus-agnostisch (bouwplan
// §7): verzonnen namen, geen bedragen, geen sector- of organisatietermen.

function wet(lawId, { validFrom = '2025-01-01', laag = 'WET', artikelen = [] } = {}) {
  return { lawId, doc: { $id: lawId, regulatory_layer: laag, valid_from: validFrom, articles: artikelen } };
}

function artikel(number, machine_readable) {
  return { number, machine_readable };
}

/** Een artikel dat één output produceert. */
function produceert(number, naam) {
  return artikel(number, { execution: { output: [{ name: naam }] } });
}

/** Een artikel dat een cross-law binding legt via input.source. */
function bindt(number, { regulation = null, output }) {
  return artikel(number, { execution: { input: [{ name: 'x', source: { regulation, output } }] } });
}

describe('outputsVan', () => {
  it('leest zowel execution.output[].name als actions[].output', () => {
    const doc = {
      articles: [
        artikel('1', { execution: { output: [{ name: 'a' }] } }),
        artikel('2', { execution: { actions: [{ output: 'b' }] } }),
      ],
    };
    expect([...outputsVan(doc)].sort()).toEqual(['a', 'b']);
  });
});

describe('openTermsVan', () => {
  it('indexeert open_terms per artikelnummer', () => {
    const doc = { articles: [artikel('3', { open_terms: [{ id: 'nadere_regels' }] })] };
    expect([...openTermsVan(doc).get('3')]).toEqual(['nadere_regels']);
  });
});

describe('bouwGraaf — klasse clean', () => {
  it('telt een resolveerbare cross-law binding als clean en maakt er een rand van', () => {
    const g = bouwGraaf([
      wet('wet_a', { artikelen: [bindt('1', { regulation: 'wet_b', output: 'bedrag' })] }),
      wet('wet_b', { artikelen: [produceert('1', 'bedrag')] }),
    ]);
    expect(g.telling.clean).toBe(1);
    expect(g.bevindingen).toEqual([]);
    expect(g.randen).toEqual([
      { van: 'wet_a', naar: 'wet_b', soort: 'source', integriteit: 'clean', label: 'bedrag' },
    ]);
  });

  it('negeert een data-registry-binding (source zonder regulation en output)', () => {
    const g = bouwGraaf([
      { lawId: 'wet_a', doc: { $id: 'wet_a', articles: [artikel('1', { execution: { input: [{ name: 'x', source: {} }] } })] } },
    ]);
    expect(g.telling.clean).toBe(0);
    expect(g.randen).toEqual([]);
  });
});

describe('bouwGraaf — klasse dangling', () => {
  it('markeert een binding naar een output die de doelwet niet produceert', () => {
    const g = bouwGraaf([
      wet('wet_a', { artikelen: [bindt('2', { regulation: 'wet_b', output: 'bestaat_niet' })] }),
      wet('wet_b', { artikelen: [produceert('1', 'iets_anders')] }),
    ]);
    expect(g.telling.dangling).toBe(1);
    expect(g.bevindingen[0]).toMatchObject({ klasse: 'dangling', lawId: 'wet_a', artikel: '2' });
    expect(g.randen[0].integriteit).toBe('dangling');
  });

  it('markeert een intra-law verwijzing naar een niet-bestaande eigen output', () => {
    const g = bouwGraaf([wet('wet_a', { artikelen: [bindt('1', { output: 'nergens' })] })]);
    expect(g.telling.dangling).toBe(1);
    expect(g.bevindingen[0].tekst).toContain('intra-law');
  });
});

// De belangrijkste: de engine kent geen `source` op een Parameter, dus deze
// binding bestaat bij uitvoering niet. Hem als rand tekenen zou samenhang
// suggereren die er niet is.
describe('bouwGraaf — klasse misplaced', () => {
  it('meldt een source onder parameters en maakt er GEEN rand van', () => {
    const g = bouwGraaf([
      {
        lawId: 'wet_a',
        doc: {
          $id: 'wet_a',
          articles: [artikel('1', { execution: { parameters: [{ name: 'p', source: { regulation: 'wet_b', output: 'bedrag' } }] } })],
        },
      },
      wet('wet_b', { artikelen: [produceert('1', 'bedrag')] }),
    ]);
    expect(g.telling.misplaced).toBe(1);
    expect(g.randen).toEqual([]);
    // De beoogde bestemming blijft wel vermeld, anders is de bevinding niet op te lossen.
    expect(g.bevindingen[0]).toMatchObject({ klasse: 'misplaced', naar: 'wet_b' });
  });
});

describe('bouwGraaf — klasse plain-param', () => {
  it('meldt een parameter die een regeling noemt zonder source', () => {
    const g = bouwGraaf([
      {
        lawId: 'wet_a',
        doc: {
          $id: 'wet_a',
          articles: [artikel('1', { execution: { parameters: [{ name: 'p', description: 'Conceptueel, komt later uit een andere regeling' }] } })],
        },
      },
    ]);
    expect(g.telling['plain-param']).toBe(1);
  });

  it('meldt een gewone parameter niet', () => {
    const g = bouwGraaf([
      {
        lawId: 'wet_a',
        doc: { $id: 'wet_a', articles: [artikel('1', { execution: { parameters: [{ name: 'p', description: 'Een gewone invoerwaarde' }] } })] },
      },
    ]);
    expect(g.telling['plain-param']).toBe(0);
  });
});

describe('bouwGraaf — klasse impl-dangling', () => {
  it('markeert een implements naar een onbekende wet', () => {
    const g = bouwGraaf([
      wet('regeling_a', { artikelen: [artikel('1', { implements: [{ law: 'wet_bestaat_niet', article: '2', open_term: 't' }] })] }),
    ]);
    expect(g.telling['impl-dangling']).toBe(1);
    expect(g.bevindingen[0].tekst).toContain('onbekende wet');
  });

  it('markeert een implements naar een open_term die het doelartikel niet declareert', () => {
    const g = bouwGraaf([
      wet('regeling_a', { artikelen: [artikel('1', { implements: [{ law: 'wet_b', article: '2', open_term: 'anders' }] })] }),
      wet('wet_b', { artikelen: [artikel('2', { open_terms: [{ id: 'nadere_regels' }] })] }),
    ]);
    expect(g.telling['impl-dangling']).toBe(1);
    expect(g.bevindingen[0].tekst).toContain('declareert open_term');
  });

  it('telt een kloppende implements als clean', () => {
    const g = bouwGraaf([
      wet('regeling_a', { artikelen: [artikel('1', { implements: [{ law: 'wet_b', article: '2', open_term: 'nadere_regels' }] })] }),
      wet('wet_b', { artikelen: [artikel('2', { open_terms: [{ id: 'nadere_regels' }] })] }),
    ]);
    expect(g.telling.clean).toBe(1);
    expect(g.telling['impl-dangling']).toBe(0);
    expect(g.randen[0]).toMatchObject({ soort: 'implements', integriteit: 'clean' });
  });
});

// RFC-003's temporele filter matcht een ongedateerde implements op élke
// rekendatum, en overschrijft daarmee stil de juiste versie.
describe('bouwGraaf — klasse impl-no-date', () => {
  it('markeert een regeling met implements maar zonder valid_from', () => {
    const g = bouwGraaf([
      {
        lawId: 'regeling_a',
        doc: { $id: 'regeling_a', articles: [artikel('1', { implements: [{ law: 'wet_b', article: '2', open_term: 'nadere_regels' }] })] },
      },
      wet('wet_b', { artikelen: [artikel('2', { open_terms: [{ id: 'nadere_regels' }] })] }),
    ]);
    expect(g.telling['impl-no-date']).toBe(1);
  });
});

describe('bouwGraaf — knopen', () => {
  it('markeert een wet zonder inkomende rand als niet-aangeroepen', () => {
    const g = bouwGraaf([
      wet('wet_a', { artikelen: [bindt('1', { regulation: 'wet_b', output: 'bedrag' })] }),
      wet('wet_b', { artikelen: [produceert('1', 'bedrag')] }),
      wet('wet_los'),
    ]);
    const perId = Object.fromEntries(g.knopen.map((k) => [k.lawId, k]));
    expect(perId.wet_b.nietAangeroepen).toBe(false);
    expect(perId.wet_a.nietAangeroepen).toBe(true); // niemand roept wet_a aan
    expect(perId.wet_los.nietAangeroepen).toBe(true);
  });

  // Een rand naar het niets moet zichtbaar zijn, anders verdwijnt de fout
  // achter een graaf die compleet oogt.
  it('maakt een knoop voor een doelwet dat niet in het corpus staat', () => {
    const g = bouwGraaf([wet('wet_a', { artikelen: [bindt('1', { regulation: 'wet_ontbreekt', output: 'x' })] })]);
    const ontbreekt = g.knopen.find((k) => k.lawId === 'wet_ontbreekt');
    expect(ontbreekt.aanwezig).toBe(false);
    expect(g.telling.dangling).toBe(1);
  });

  it('een intra-law rand telt niet als inkomende verwijzing', () => {
    const g = bouwGraaf([
      wet('wet_a', { artikelen: [produceert('1', 'eigen'), bindt('2', { output: 'eigen' })] }),
    ]);
    expect(g.knopen[0].nietAangeroepen).toBe(true);
  });
});

describe('bouwGraaf — determinisme', () => {
  it('geeft dezelfde uitvoer ongeacht de volgorde van de wetten', () => {
    const a = wet('wet_a', { artikelen: [bindt('1', { regulation: 'wet_b', output: 'bedrag' })] });
    const b = wet('wet_b', { artikelen: [produceert('1', 'bedrag')] });
    expect(bouwGraaf([a, b])).toEqual(bouwGraaf([b, a]));
  });
});

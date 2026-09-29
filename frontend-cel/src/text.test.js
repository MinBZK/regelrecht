import { describe, expect, it } from 'vitest';
import {
  amountText,
  outputText,
  provenanceRows,
  provenanceText,
  routesFrom,
  routeText,
  sourceStatusText,
  valueText,
} from './text.js';

describe('valueText', () => {
  it('writes out yes/no, empty and null', () => {
    expect(valueText(true)).toBe('ja');
    expect(valueText(false)).toBe('nee');
    expect(valueText(undefined)).toBe('');
    expect(valueText(null)).toBe('geen (null)');
  });

  it('keeps text as is and turns the rest into JSON', () => {
    expect(valueText('abc')).toBe('abc');
    expect(valueText(12)).toBe('12');
    expect(valueText({ a: 1 })).toBe('{"a":1}');
  });
});

describe('provenanceText', () => {
  // One case per variant of Herkomst in packages/cel/src/synthese.rs, as
  // serde serializes it (tag `source`, snake_case).
  const variants = [
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

  it.each(variants)('%o', (provenance, text) => {
    expect(provenanceText(provenance)).toBe(text);
  });

  it('does not fall back to JSON for any known variant', () => {
    for (const [provenance] of variants) {
      expect(provenanceText(provenance)).not.toBe(JSON.stringify(provenance));
    }
  });

  it('shows an unknown variant as JSON', () => {
    expect(provenanceText({ source: 'nieuw' })).toBe('{"source":"nieuw"}');
  });
});

describe('provenanceRows', () => {
  it('gives name, value and source per parameter', () => {
    const rows = provenanceRows({ bedrag: 10, ok: true }, {
      bedrag: { source: 'handler' },
      ok: { source: 'choice' },
    });
    expect(rows).toEqual([
      { name: 'bedrag', value: '10', source: 'behandelaar (formulier van de handeling)' },
      { name: 'ok', value: 'ja', source: 'keuze van de aanvrager (portaal)' },
    ]);
  });

  it('is empty without provenance', () => {
    expect(provenanceRows(undefined, undefined)).toEqual([]);
  });
});

describe('amountText and outputText', () => {
  const text = (t) => t.replace(/\s/g, ' ');

  it('writes an amount in the unit of the regulation', () => {
    expect(text(amountText(1913600, 'eurocent'))).toBe('€ 19.136,00');
    expect(text(amountText(0, 'eurocent'))).toBe('€ 0,00');
    expect(text(amountText(19136, 'euro'))).toBe('€ 19.136,00');
    expect(amountText(5, 'punten')).toBe('5 punten');
    expect(amountText(5)).toBe('5');
    expect(amountText(null, 'eurocent')).toBe('geen (null)');
  });

  it('takes the type of an output from the regulation, not from its name', () => {
    const amount = { type: 'amount', unit: 'eurocent' };
    expect(text(outputText(100, amount))).toBe('€ 1,00');
    // A name that looks like an amount, without a type: a number.
    expect(outputText(100, undefined)).toBe('100');
    expect(outputText(100, { type: 'number' })).toBe('100');
    expect(outputText(true, { type: 'boolean' })).toBe('ja');
  });
});

describe('valueText and an unknown value', () => {
  it('names what is missing', () => {
    const unknown = { __unknown: true, missing: [{ law: 'w', name: 'inkomen', kind: 'parameter' }] };
    expect(valueText(unknown)).toBe('onbekend (mist inkomen)');
    expect(valueText({ __unknown: true })).toBe('onbekend');
  });
});

describe('route of the reduction (experiment A)', () => {
  it('names the engine with the regulation, and the DSL with the reason', () => {
    expect(routeText(null)).toBe('');
    expect(routeText({ route: 'engine', regulation: 'lexostatus_x', duration_us: 1500 })).toBe(
      'reductie via de engine (lexostatus_x, 1.50 ms)',
    );
    expect(routeText({ route: 'dsl', reason: 'een lijst' })).toBe('reductie via de DSL (bewust: een lijst)');
    expect(routeText({ route: 'runtime', reason: 'de stand' })).toBe('door de runtime zelf (de stand)');
  });

  it('puts the route after the provenance, per own lexostatus and per source', () => {
    const response = {
      lexostatuses: [{ name: 'aanvraag', reduction: { route: 'engine', regulation: 'lexostatus_aanvraag' } }],
      sources: [{ cell: 'register', lexostatus: 'stand', reduction: { route: 'dsl' } }],
    };
    const rows = provenanceRows(
      { a: 1, b: true, c: 2 },
      {
        a: { source: 'own', lexostatus: 'aanvraag' },
        b: { source: 'cell', cell: 'register', lexostatus: 'stand', transport: 'internal' },
        c: { source: 'handler' },
      },
      routesFrom(response),
    );
    expect(rows.map((r) => r.source)).toEqual([
      'eigen lexostatus aanvraag; reductie via de engine (lexostatus_aanvraag)',
      'cel register, lexostatus stand (internal); reductie via de DSL',
      'behandelaar (formulier van de handeling)',
    ]);
  });
});

describe('sourceStatusText', () => {
  it('says the status of a source in words', () => {
    expect(sourceStatusText('queried')).toBe('bevraagd');
    expect(sourceStatusText('unreachable')).toBe('onbereikbaar');
    expect(sourceStatusText('error')).toBe('fout');
    expect(sourceStatusText('not_queried')).toBe('niet bevraagd');
    expect(sourceStatusText('iets_nieuws')).toBe('iets nieuws');
  });
});

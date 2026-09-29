import { describe, expect, it } from 'vitest';
import { bedragTekst } from './tekst.js';
import { besluitKop, indeling, soortTekst, statusTekst } from './zaak.js';

const h = (naam, extra = {}) => ({
  name: naam,
  label: naam,
  kind: { kind: 'fact' },
  form: [],
  available: true,
  recorded: 0,
  decision: null,
  ...extra,
});

// Een zaak zoals de runtime haar geeft: twee besluiten, een vervolg en een
// betaling op het eerste, een besluit dat nog kan, en een feit van de zaak.
const zaak = {
  decisions: [
    { id: 'b1', action: 'voorschot', effective_at: '2025-03-12T00:00:00+01:00' },
    { id: 'b2', action: 'wijzigen', amends: 'b1' },
  ],
  actions: [
    h('voorschot', { kind: { kind: 'decision' }, stage: 'BESLUIT', recorded: 1, available: false }),
    h('bekendmaken', { kind: { kind: 'follow_up', decision: 'voorschot', procedure: 'p' }, stage: 'BEKENDMAKING', decision: 'b1' }),
    h('betalen', {
      decision: 'b1',
      form: [{ name: 'bedrag', type: 'amount', unit: 'eurocent' }],
      types: { nog_te_betalen: { type: 'amount', unit: 'eurocent' } },
      trial: { outputs: { nog_te_betalen: 1200 } },
    }),
    h('wijzigen', { kind: { kind: 'decision' }, stage: 'BESLUIT', decision: 'b2', recorded: 1 }),
    h('terugvorderen', { kind: { kind: 'decision' }, stage: 'BESLUIT' }),
    h('aanvulling_vragen'),
  ],
};

describe('indeling', () => {
  it('zet elke handeling bij het besluit waarop zij handelt', () => {
    const d = indeling(zaak);
    expect(d.besluiten.map((b) => b.handelingen.map((x) => x.name))).toEqual([
      ['voorschot', 'bekendmaken', 'betalen'],
      ['wijzigen'],
    ]);
    expect(d.overig.map((x) => x.name)).toEqual(['terugvorderen', 'aanvulling_vragen']);
  });

  it('geeft de betaalstand per besluit, in de eenheid van de regeling', () => {
    const d = indeling(zaak);
    expect(d.besluiten[0].betaalstand).toEqual([
      { sleutel: 'betalennog_te_betalen', handeling: 'betalen', naam: 'nog_te_betalen', waarde: bedragTekst(1200, 'eurocent') },
    ]);
    expect(d.besluiten[1].betaalstand).toEqual([]);
  });

  it('kent een zaak zonder besluiten', () => {
    expect(indeling({ actions: [h('a')] }).overig.map((x) => x.name)).toEqual(['a']);
    expect(indeling(null).besluiten).toEqual([]);
  });
});

describe('teksten', () => {
  it('noemt het besluit, de dag en wat het wijzigt', () => {
    expect(besluitKop(zaak.decisions[0])).toBe('besluit b1, genomen op 2025-03-12');
    expect(besluitKop(zaak.decisions[1])).toBe('besluit b2, wijzigt besluit b1');
  });

  it('zegt of een handeling kan', () => {
    expect(statusTekst(zaak.actions[0])).toBe('vastgelegd');
    expect(statusTekst(zaak.actions[1])).toBe('kan');
    expect(statusTekst({ ...zaak.actions[1], available: false })).toBe('nog niet');
    expect(statusTekst({ ...zaak.actions[2], recorded: 2 })).toBe('kan (2 keer vastgelegd)');
  });

  it('noemt de soort van een handeling in het Nederlands', () => {
    expect(soortTekst(zaak.actions[0])).toBe('besluit, stage BESLUIT');
    expect(soortTekst(zaak.actions[1])).toBe('vervolg, stage BEKENDMAKING');
    expect(soortTekst(zaak.actions[2])).toBe('feit');
  });
});

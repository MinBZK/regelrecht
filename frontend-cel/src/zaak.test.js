import { describe, expect, it } from 'vitest';
import { euroTekst } from './tekst.js';
import { besluitKop, indeling, statusTekst } from './zaak.js';

const h = (naam, extra = {}) => ({
  naam,
  label: naam,
  soort: { soort: 'feit' },
  formulier: [],
  beschikbaar: true,
  vastgelegd: 0,
  besluit: null,
  ...extra,
});

// Een zaak zoals de runtime haar geeft: twee besluiten, een vervolg en een
// betaling op het eerste, een besluit dat nog kan, en een feit van de zaak.
const zaak = {
  besluiten: [
    { besluitkenmerk: 'z/1', handeling: 'voorschot', op_moment: '2025-03-12T00:00:00+01:00' },
    { besluitkenmerk: 'z/2', handeling: 'wijzigen', wijzigt: 'z/1' },
  ],
  handelingen: [
    h('voorschot', { soort: { soort: 'besluit' }, stage: 'BESLUIT', vastgelegd: 1, beschikbaar: false }),
    h('bekendmaken', { soort: { soort: 'vervolg' }, stage: 'BEKENDMAKING', besluit: 'z/1' }),
    h('betalen', {
      besluit: 'z/1',
      formulier: [{ naam: 'bedrag', type: 'bedrag' }],
      proef: { uitkomsten: { nog_te_betalen: 1200 } },
    }),
    h('wijzigen', { soort: { soort: 'besluit' }, stage: 'BESLUIT', besluit: 'z/2', vastgelegd: 1 }),
    h('terugvorderen', { soort: { soort: 'besluit' }, stage: 'BESLUIT' }),
    h('aanvulling_vragen'),
  ],
};

describe('indeling', () => {
  it('zet elke handeling bij het besluit waarop zij handelt', () => {
    const d = indeling(zaak);
    expect(d.besluiten.map((b) => b.handelingen.map((x) => x.naam))).toEqual([
      ['voorschot', 'bekendmaken', 'betalen'],
      ['wijzigen'],
    ]);
    expect(d.overig.map((x) => x.naam)).toEqual(['terugvorderen', 'aanvulling_vragen']);
  });

  it('geeft de betaalstand per besluit, in euro', () => {
    const d = indeling(zaak);
    expect(d.besluiten[0].betaalstand).toEqual([
      { sleutel: 'betalennog_te_betalen', handeling: 'betalen', naam: 'nog_te_betalen', waarde: euroTekst(1200) },
    ]);
    expect(d.besluiten[1].betaalstand).toEqual([]);
  });

  it('kent een zaak zonder besluiten', () => {
    expect(indeling({ handelingen: [h('a')] }).overig.map((x) => x.naam)).toEqual(['a']);
    expect(indeling(null).besluiten).toEqual([]);
  });
});

describe('teksten', () => {
  it('noemt het besluit, de dag en wat het wijzigt', () => {
    expect(besluitKop(zaak.besluiten[0])).toBe('besluit z/1, genomen op 2025-03-12');
    expect(besluitKop(zaak.besluiten[1])).toBe('besluit z/2, wijzigt besluit z/1');
  });

  it('zegt of een handeling kan', () => {
    expect(statusTekst(zaak.handelingen[0])).toBe('vastgelegd');
    expect(statusTekst(zaak.handelingen[1])).toBe('kan');
    expect(statusTekst({ ...zaak.handelingen[1], beschikbaar: false })).toBe('nog niet');
    expect(statusTekst({ ...zaak.handelingen[2], vastgelegd: 2 })).toBe('kan (2 keer vastgelegd)');
  });
});

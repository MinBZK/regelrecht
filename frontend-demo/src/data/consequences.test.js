import { describe, expect, it } from 'vitest';
import { consequencesOf } from './consequences.js';

const party = {
  cell: 'kas',
  view: 'account',
  data: { service: 'KAS', table: 'rekeningen', number: 'nr', opening_balance: 'begin', blocked: 'dicht' },
  gram: { account: 'nr', amount: 'bedrag', credited: 'bij', date: 'datum', reason: 'reden', reference: 'kenmerk' },
};
const cells = [{ id: 'kas', recordingActor: 'kas', events: [{ name: 'bijgeschreven', chronicle: 'k' }] }];
const sources = { KAS: { rekeningen: [{ nr: 'NL00TEST1', begin: 100, dicht: false }] } };
const grams = [{ id: 'g1', name: 'bijgeschreven', chronicle: 'k', recorded_at: '2025-01-01T10:00:00+01:00', fields: { nr: 'NL00TEST1', bedrag: 50, bij: 50, datum: '2025-01-01' } }];

describe('consequencesOf', () => {
  it('geeft per partij wat haar view uit de gegevens en de kroniek leest', () => {
    const [kas] = consequencesOf([party], { sources, grams, cells, serviceOf: (id) => (id === 'kas' ? 'KAS' : null) });
    expect(kas.cell).toBe('kas');
    expect(kas.view).toBe('account');
    expect(kas.service).toBe('KAS');
    expect(kas.data.balance).toBe(150);
  });

  it('geeft geen gegevens voor een persona zonder rekening of een view die de demo niet kent', () => {
    expect(consequencesOf([party], { sources: {}, grams, cells })[0].data).toBeNull();
    expect(consequencesOf([{ cell: 'verzekeraar', view: 'polis' }], { sources, grams, cells })[0]).toEqual({ cell: 'verzekeraar', view: 'polis', service: null, data: null });
  });

  it('geeft niets zonder configuratie', () => {
    expect(consequencesOf(undefined, { sources, grams, cells })).toEqual([]);
  });
});

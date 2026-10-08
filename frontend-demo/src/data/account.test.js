import { describe, expect, it } from 'vitest';
import { accountOf } from './account.js';

const config = {
  cell: 'bank',
  data: { service: 'BANK', table: 'rekeningen', number: 'rekeningnummer', opening_balance: 'beginsaldo', blocked: 'geblokkeerd' },
  gram: { account: 'rekeningnummer', amount: 'bedrag', credited: 'bijgeschreven_bedrag', date: 'uitvoerdatum', reason: 'reden', reference: 'betaalkenmerk' },
};
const cells = [
  { id: 'bank', recordingActor: 'fictieve_bank', events: [{ name: 'overboeking_bijgeschreven', chronicle: 'rekeningen' }] },
  { id: 'toeslagen', recordingActor: 'belastingdienst_toeslagen', events: [{ name: 'betaalopdracht_gegeven', chronicle: 'toeslagen' }] },
];
const sources = { BANK: { rekeningen: [{ rekeningnummer: 'NL00TEST0123456789', beginsaldo: 10000, geblokkeerd: false }] } };
const bankGram = (id, date, credited, extra = {}) => ({
  id,
  name: credited ? 'overboeking_bijgeschreven' : 'overboeking_geweigerd',
  chronicle: 'rekeningen',
  recorded_at: `${date}T11:00:00+01:00`,
  fields: { rekeningnummer: 'NL00TEST0123456789', bedrag: 500, bijgeschreven_bedrag: credited ? 500 : 0, uitvoerdatum: date, reden: credited ? '' : 'rekening geblokkeerd', betaalkenmerk: 'o1', ...extra },
});

describe('accountOf', () => {
  it('adds what the bank credited to the opening balance, newest first', () => {
    const grams = [
      { id: 'o1', name: 'betaalopdracht_gegeven', chronicle: 'toeslagen', fields: {} },
      bankGram('b1', '2025-01-01', true),
      bankGram('b2', '2025-02-01', false),
      bankGram('b3', '2025-03-01', true, { rekeningnummer: 'NL00TEST0000000000' }),
    ];
    const account = accountOf(config, sources, grams, cells);
    expect(account.number).toBe('NL00TEST0123456789');
    expect(account.blocked).toBe(false);
    expect(account.opening).toBe(10000);
    expect(account.balance).toBe(10500);
    expect(account.transactions.map((t) => [t.id, t.credited, t.reason])).toEqual([
      ['b2', 0, 'rekening geblokkeerd'],
      ['b1', 500, null],
    ]);
    expect(account.transactions[1].payer).toBe('belastingdienst_toeslagen');
  });

  it('is null without an account or a configuration', () => {
    expect(accountOf(config, {}, [], cells)).toBeNull();
    expect(accountOf(null, sources, [], cells)).toBeNull();
  });
});

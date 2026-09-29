import { describe, expect, it } from 'vitest';
import { amountText } from './text.js';
import { caseLayout, decisionHeading, kindText, statusText } from './case.js';

const action = (name, extra = {}) => ({
  name,
  label: name,
  kind: { kind: 'fact' },
  form: [],
  available: true,
  recorded: 0,
  decision: null,
  ...extra,
});

// A case as the runtime gives it: two decisions, a follow-up and a payment on
// the first, a decision that can still be taken, and a fact of the case.
const caseData = {
  decisions: [
    { id: 'b1', action: 'voorschot', effective_at: '2025-03-12T00:00:00+01:00' },
    { id: 'b2', action: 'wijzigen', amends: 'b1' },
  ],
  actions: [
    action('voorschot', { kind: { kind: 'decision' }, stage: 'BESLUIT', recorded: 1, available: false }),
    action('bekendmaken', { kind: { kind: 'follow_up', decision: 'voorschot', procedure: 'p' }, stage: 'BEKENDMAKING', decision: 'b1' }),
    action('betalen', {
      decision: 'b1',
      form: [{ name: 'bedrag', type: 'amount', unit: 'eurocent' }],
      types: { nog_te_betalen: { type: 'amount', unit: 'eurocent' } },
      trial: { outputs: { nog_te_betalen: 1200 } },
    }),
    action('wijzigen', { kind: { kind: 'decision' }, stage: 'BESLUIT', decision: 'b2', recorded: 1 }),
    action('terugvorderen', { kind: { kind: 'decision' }, stage: 'BESLUIT' }),
    action('aanvulling_vragen'),
  ],
};

describe('caseLayout', () => {
  it('puts each action with the decision it acts on', () => {
    const layout = caseLayout(caseData);
    expect(layout.decisions.map((d) => d.ownActions.map((a) => a.name))).toEqual([
      ['voorschot', 'bekendmaken', 'betalen'],
      ['wijzigen'],
    ]);
    expect(layout.other.map((a) => a.name)).toEqual(['terugvorderen', 'aanvulling_vragen']);
  });

  it('gives the payment status per decision, in the unit of the regulation', () => {
    const layout = caseLayout(caseData);
    expect(layout.decisions[0].paymentStatus).toEqual([
      { key: 'betalennog_te_betalen', action: 'betalen', name: 'nog_te_betalen', value: amountText(1200, 'eurocent') },
    ]);
    expect(layout.decisions[1].paymentStatus).toEqual([]);
  });

  it('handles a case without decisions', () => {
    expect(caseLayout({ actions: [action('a')] }).other.map((a) => a.name)).toEqual(['a']);
    expect(caseLayout(null).decisions).toEqual([]);
  });
});

describe('texts', () => {
  it('names the decision, the day and what it amends', () => {
    expect(decisionHeading(caseData.decisions[0])).toBe('besluit b1, genomen op 2025-03-12');
    expect(decisionHeading(caseData.decisions[1])).toBe('besluit b2, wijzigt besluit b1');
  });

  it('says whether an action can be taken', () => {
    expect(statusText(caseData.actions[0])).toBe('vastgelegd');
    expect(statusText(caseData.actions[1])).toBe('kan');
    expect(statusText({ ...caseData.actions[1], available: false })).toBe('nog niet');
    expect(statusText({ ...caseData.actions[2], recorded: 2 })).toBe('kan (2 keer vastgelegd)');
  });

  it('names the kind of an action in Dutch', () => {
    expect(kindText(caseData.actions[0])).toBe('besluit, stage BESLUIT');
    expect(kindText(caseData.actions[1])).toBe('vervolg, stage BEKENDMAKING');
    expect(kindText(caseData.actions[2])).toBe('feit');
  });
});

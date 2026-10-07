import { describe, expect, it } from 'vitest';
import { amountTotals, citizenRows, momentView } from './chronicleView.js';

const corpus = { lawById: () => null };
const fields = {
  termijnbedrag: { name: 'termijnbedrag', type: 'amount' },
  terug: { name: 'terug', type: 'amount' },
  uiterste: { name: 'uiterste', type: 'date' },
  voldoet: { name: 'voldoet', type: 'boolean' },
};
const fieldsOf = () => fields;

describe('wat er uit de kroniek betaald is', () => {
  it('telt per bedragveld op over de uitvoeringen', () => {
    const grams = [{ fields: { termijnbedrag: 15409 } }, { fields: { termijnbedrag: 15410 } }];
    const [total] = amountTotals(grams, fieldsOf, corpus);
    expect(total.name).toBe('termijnbedrag');
    expect(total.value).toBe(30819);
    expect(total.count).toBe(2);
  });

  it('telt niets zonder uitvoeringen', () => {
    expect(amountTotals([], fieldsOf, corpus)).toEqual([]);
  });
});

describe('wat een burger van een besluit ziet', () => {
  it('toont bedragen, ook nul, en datums met een waarde, maar geen andere velden', () => {
    const gram = { fields: { terug: 0, uiterste: '2026-10-15', voldoet: true, termijnbedrag: 100 } };
    expect(citizenRows(gram, fields, corpus).map((r) => r.name)).toEqual(['terug', 'uiterste', 'termijnbedrag']);
    expect(citizenRows({ fields: { uiterste: null } }, fields, corpus)).toEqual([]);
  });
});

describe('een moment als regel', () => {
  it('zegt van een uitvoering wat de cel dan zou vastleggen', () => {
    const m = momentView({ kind: 'execution', name: 'termijn_betaald', date: '2025-02-01', gram: { fields: { voldoet: true } } }, { corpus, fieldsOf });
    expect(m.text).toBe('Termijn betaald');
    expect(m.value).not.toBe('');
  });

  it('noemt het eind van een periode met haar waarde', () => {
    const m = momentView({ kind: 'period_end', name: 'x', date: '2025-12-31', period: { unit: 'year', value: 2025 } }, { corpus, fieldsOf });
    expect(m.text).toContain('2025');
  });

  it('onderscheidt een datum van een genomen besluit van een die nog komt', () => {
    const m = { kind: 'law', name: 'uiterste', date: '2026-10-15', event: 'toegekend', provision: 'wet#19' };
    const given = momentView(m, { corpus, fieldsOf, decided: () => true });
    const expected = momentView(m, { corpus, fieldsOf, decided: () => false });
    expect(given.supporting).not.toBe(expected.supporting);
  });
});

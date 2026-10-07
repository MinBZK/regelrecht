import { describe, expect, it } from 'vitest';
import { citizenRows, inputRows, lexostatusRows, momentView } from './chronicleView.js';

const corpus = { lawById: () => null };
const fields = {
  termijnbedrag: { name: 'termijnbedrag', type: 'amount' },
  terug: { name: 'terug', type: 'amount' },
  uiterste: { name: 'uiterste', type: 'date' },
  voldoet: { name: 'voldoet', type: 'boolean' },
};
const fieldsOf = () => fields;

describe('wat de cel als ontvangen leest', () => {
  it('leest elke waarde als het veld waaruit de cel haar afleidt, zonder zelf op te tellen', () => {
    const rows = lexostatusRows({ uitbetaald: 30819 }, { uitbetaald: fields.termijnbedrag }, corpus);
    expect(rows).toHaveLength(1);
    expect(rows[0].name).toBe('uitbetaald');
    expect(rows[0].value).toBe(30819);
    expect(rows[0].text).toMatch(/308,19/);
  });

  it('geeft niets voor niets', () => {
    expect(lexostatusRows({}, {}, corpus)).toEqual([]);
    expect(lexostatusRows(null, null, corpus)).toEqual([]);
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

describe('de invoer van een besluit', () => {
  it('leest een geheel getal zonder declaratie als getal en houdt de herkomst', () => {
    const gram = { inputs: { jaar: { value: 2025, provenance: { source: 'decision' } } } };
    expect(inputRows(gram, corpus)).toEqual([{ name: 'jaar', value: 2025, provenance: { source: 'decision' }, text: '2025' }]);
  });
});

describe('de invoer uit een andere wet', () => {
  it('leest een invoer naar de declaratie in een wet waarop de gram rust', () => {
    const doc = { articles: [{ machine_readable: { execution: { parameters: [{ name: 'inkomen', type: 'amount' }] } } }] };
    const withLaw = { lawById: (id) => (id === 'andere' ? { doc } : null) };
    const gram = { regulation: 'eigen', legal_basis: ['eigen#1', 'andere#16'], inputs: { inkomen: { value: 2200000, provenance: null } } };
    expect(inputRows(gram, withLaw)[0].text).toMatch(/22[.,]000/);
  });
});

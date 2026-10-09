import { describe, expect, it } from 'vitest';
import { caseOfSubject, exOfficioToTake, followingDue, followingOutcome, isClockDecision, withPeriod } from './following.js';

describe('followingDue', () => {
  it('neemt een besluit op de dag van het beleid, niet ervoor', () => {
    expect(followingDue({ day: '2026-04-15', dates: {} }, '2026-04-14')).toBe(false);
    expect(followingDue({ day: '2026-04-15', dates: {} }, '2026-04-15')).toBe(true);
  });
  it('zonder dag van het beleid op de datums uit het dossier, en zonder datums niet', () => {
    expect(followingDue({ day: null, dates: { datum: '2026-04-15' } }, '2026-04-15')).toBe(true);
    expect(followingDue({ day: null, dates: { datum: '2026-04-15' } }, '2026-04-14')).toBe(false);
    expect(followingDue({ day: null, dates: {} }, '2030-01-01')).toBe(false);
  });
  it('wacht op de dag van het beleid als de stroom er een aanwijst, ook met datums uit het dossier', () => {
    expect(followingDue({ day: null, decidedOn: 'beleid#3', dates: { datum: '2026-04-15' } }, '2026-05-01')).toBe(false);
  });
});

describe('followingOutcome', () => {
  it('legt een afwijzing vast als besluit, zodat het volgende jaar volgt', () => {
    expect(followingOutcome(false)).toBe('refusal');
  });
  it('legt een toekenning vast, ook bij een besluit zonder oordeel', () => {
    expect(followingOutcome(true)).toBe('grant');
    expect(followingOutcome(null)).toBe('grant');
  });
  it('legt niets vast als de wet nog niet kan beslissen', () => {
    expect(followingOutcome('unknown')).toBe('undecided');
  });
});

describe('withPeriod', () => {
  it('zet de periode van de cel in de parameter die haar geeft', () => {
    const inputs = { a: { value: 1 } };
    expect(withPeriod(inputs, 'berekeningsjaar', { unit: 'year', value: 2026 })).toEqual({
      a: { value: 1 },
      berekeningsjaar: { value: 2026, provenance: { source: 'period' } },
    });
    expect(inputs).toEqual({ a: { value: 1 } });
  });
  it('laat de parameters zoals ze zijn zonder periode of parameter', () => {
    const inputs = { a: { value: 1 } };
    expect(withPeriod(inputs, null, { value: 2026 })).toBe(inputs);
    expect(withPeriod(inputs, 'jaar', null)).toBe(inputs);
  });
});

describe('isClockDecision', () => {
  const own = new Set(['aanvraag_ontvangen', 'voorschot_verleend']);
  const onApplication = { type: 'decretogram', refers_to: { op: { required: true, to: 'awir#15' } } };
  it('neemt een besluit met een dag van de houder op de aanvraag', () => {
    expect(isClockDecision({ name: 'terugvordering', decided_on: 'beleid#6' }, onApplication, 'awir#15', own)).toBe(true);
  });
  it('niet een besluit van de levensloop, zonder dag, of zonder verwijzing naar de aanvraag', () => {
    expect(isClockDecision({ name: 'voorschot_verleend', decided_on: 'beleid#4' }, onApplication, 'awir#15', own)).toBe(false);
    expect(isClockDecision({ name: 'terugvordering' }, onApplication, 'awir#15', own)).toBe(false);
    expect(isClockDecision({ name: 'terugvordering', decided_on: 'beleid#6' }, { ...onApplication, refers_to: {} }, 'awir#15', own)).toBe(false);
    expect(isClockDecision({ name: 'nabetaling', decided_on: 'beleid#6' }, { ...onApplication, type: 'executogram' }, 'awir#15', own)).toBe(false);
  });
});

describe('exOfficioToTake', () => {
  it('neemt alleen wat een dag op of vóór vandaag, een periode en een parameter heeft', () => {
    const due = [
      { bsn: 'a', day: '2026-04-15', period: { value: 2025 }, periodParameter: 'belastingjaar' },
      { bsn: 'b', day: '2026-04-16', period: { value: 2025 }, periodParameter: 'belastingjaar' },
      { bsn: 'c', day: null, period: { value: 2025 }, periodParameter: 'belastingjaar' },
      { bsn: 'd', day: '2026-04-01', period: null, periodParameter: 'belastingjaar' },
    ];
    expect(exOfficioToTake(due, '2026-04-15').map((d) => d.bsn)).toEqual(['a']);
  });
});

describe('caseOfSubject', () => {
  const statusOf = (c) => c.status;
  it('slaat een ingetrokken zaak over', () => {
    const cases = [
      { id: 1, bsn: 'a', applicationGramId: 'g1', status: 'WITHDRAWN' },
      { id: 2, bsn: 'a', applicationGramId: 'g2', status: 'DECIDED' },
    ];
    expect(caseOfSubject(cases, 'a', statusOf)?.id).toBe(2);
  });
  it('geeft null zonder open zaak met een aanvraag in een kroniek', () => {
    const cases = [
      { id: 1, bsn: 'a', applicationGramId: 'g1', status: 'WITHDRAWN' },
      { id: 2, bsn: 'a', applicationGramId: null, status: 'DECIDED' },
    ];
    expect(caseOfSubject(cases, 'a', statusOf)).toBeNull();
  });
});

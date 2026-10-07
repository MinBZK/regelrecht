import { describe, expect, it } from 'vitest';
import {
  addDays,
  comingDates,
  decisionDue,
  executionDays,
  executionStart,
  fixedDates,
  monthStart,
  monthsAfter,
  nextExecution,
  nextMoment,
  pendingExecutionDays,
  periodEnd,
  referredGram,
} from './moments.js';

describe('datums', () => {
  it('telt dagen en maanden over de grens van een jaar', () => {
    expect(addDays('2024-12-31', 1)).toBe('2025-01-01');
    expect(addDays('2024-02-28', 1)).toBe('2024-02-29');
    expect(monthStart('2024-11-20')).toBe('2024-11-01');
    expect(monthStart('2024-11-20', 2)).toBe('2025-01-01');
    expect(monthsAfter('2024-11-20', 3)).toEqual(['2024-12-01', '2025-01-01', '2025-02-01']);
  });

  it('geeft per maand die de klok passeert één dag, niet eerder dan het begin', () => {
    expect(executionDays('2024-11-20', '2025-02-01')).toEqual(['2024-11-20', '2024-12-01', '2025-01-01', '2025-02-01']);
    expect(executionDays('2025-03-10', '2025-03-10')).toEqual(['2025-03-10']);
    expect(executionDays('2025-03-11', '2025-03-10')).toEqual([]);
  });

  it('kent het eind van een kalenderjaar en niets anders', () => {
    expect(periodEnd({ unit: 'year', value: 2025 })).toBe('2025-12-31');
    expect(periodEnd({ unit: 'month', value: 3 })).toBeNull();
    expect(periodEnd(null)).toBeNull();
  });
});

describe('het volgende moment', () => {
  it('is het vroegste moment na de peildatum, niet een moment dat al voorbij is', () => {
    const moments = [{ date: '2025-01-01' }, { date: '2024-12-01' }, { date: '2024-11-20' }];
    expect(nextMoment(moments, '2024-11-20')).toEqual({ date: '2024-12-01' });
    expect(nextMoment(moments, '2025-01-01')).toBeNull();
  });
});

const voorschot = { id: 'v', stage: 'VOORSCHOT', establishes: 'wet#2', effective_at: '2024-11-20T14:00:00+01:00' };
const toekenning = { id: 't', stage: 'TOEKENNING', establishes: 'wet#2', effective_at: '2026-04-15T09:00:00+02:00' };
const termijnShape = {
  refers_to: { voorschot: { stage: 'VOORSCHOT', required: true } },
  executed_on: { parameter: 'maand', once_per: 'month' },
  until: { stage: 'TOEKENNING' },
};

describe('wanneer de cel een uitvoering uitvoert', () => {
  it('vindt de gram waarnaar de wet verwijst op fase of op artikel', () => {
    expect(referredGram([voorschot], { stage: 'VOORSCHOT' })).toBe(voorschot);
    expect(referredGram([voorschot], { to: 'wet#2' })).toBe(voorschot);
    expect(referredGram([voorschot], { stage: 'TOEKENNING' })).toBeNull();
  });

  it('begint op de dag van het voorschot als dat vandaag is, en anders de dag erna', () => {
    expect(executionStart(termijnShape, [voorschot], '2024-11-20')).toBe('2024-11-20');
    expect(executionStart(termijnShape, [voorschot], '2025-01-01')).toBe('2024-11-21');
  });

  it('begint niet zonder voorschot, en niet meer na de toekenning', () => {
    expect(executionStart(termijnShape, [], '2025-01-01')).toBeNull();
    expect(executionStart(termijnShape, [voorschot, toekenning], '2026-05-01')).toBeNull();
  });

  it('vraagt elke gepasseerde maand één keer, en een maand met een termijn niet opnieuw', () => {
    const paid = { name: 'termijn', effective_at: '2024-12-01T00:00:00+01:00' };
    const days = pendingExecutionDays({
      shape: termijnShape,
      event: 'termijn',
      caseGrams: [voorschot, paid],
      start: '2024-11-21',
      checkedThrough: '2024-11-20',
      today: '2025-02-01',
    });
    expect(days).toEqual(['2025-01-01', '2025-02-01']);
    expect(
      pendingExecutionDays({ shape: termijnShape, event: 'termijn', caseGrams: [], start: '2025-01-01', checkedThrough: '2025-01-01', today: '2025-01-20' }),
    ).toEqual([]);
  });

  it('vraagt niets voor een ritme dat de demo niet kent', () => {
    const shape = { ...termijnShape, executed_on: { parameter: 'dag' } };
    expect(pendingExecutionDays({ shape, event: 'x', caseGrams: [], start: '2025-01-01', today: '2025-03-01' })).toEqual([]);
  });
});

describe('de volgende uitvoering', () => {
  it('is de eerste maand waarin de wet een gram geeft', () => {
    const asked = [];
    const preview = (day) => {
      asked.push(day);
      return day === '2025-01-01' ? { fields: { termijnbedrag: 100 } } : null;
    };
    expect(nextExecution(preview, '2024-11-20')).toEqual({ date: '2025-01-01', gram: { fields: { termijnbedrag: 100 } } });
    expect(asked).toEqual(['2024-12-01', '2025-01-01']);
  });

  it('is er niet als de wet er geen meer geeft of de cel weigert', () => {
    expect(nextExecution(() => null, '2025-11-20', 3)).toBeNull();
    expect(nextExecution(() => { throw new Error('beëindigd'); }, '2025-11-20')).toBeNull();
  });
});

describe('de datums die de wet een besluit geeft', () => {
  const fields = { uiterste: { type: 'date' }, uitbetalen: { type: 'date' }, bedrag: { type: 'amount' } };

  it('houdt alleen de datums die niet met de besluitdag meeschuiven', () => {
    const vandaag = { fields: { uiterste: '2026-10-15', uitbetalen: '2026-05-13', bedrag: 1 } };
    const morgen = { fields: { uiterste: '2026-10-15', uitbetalen: '2026-05-14', bedrag: 1 } };
    expect(fixedDates(vandaag, morgen, fields, '2026-04-15')).toEqual([{ name: 'uiterste', date: '2026-10-15' }]);
    expect(fixedDates(vandaag, morgen, fields, '2026-10-15')).toEqual([]);
    expect(fixedDates(vandaag, morgen, fields)).toEqual([{ name: 'uiterste', date: '2026-10-15' }]);
  });

  it('geeft van een gram de datums die nog komen', () => {
    const gram = { fields: { uiterste: '2026-10-15', uitbetalen: '2026-05-13' } };
    expect(comingDates(gram, fields, '2026-05-01')).toEqual([
      { name: 'uiterste', date: '2026-10-15' },
      { name: 'uitbetalen', date: '2026-05-13' },
    ]);
  });
});

describe('of een besluit zijn moment heeft', () => {
  it('heeft het als elke datum uit het dossier voorbij is', () => {
    expect(decisionDue({ aanslag: '2026-04-15' }, '2026-04-15')).toBe(true);
    expect(decisionDue({ aanslag: '2026-04-15' }, '2026-04-14')).toBe(false);
  });

  it('heeft het niet als het besluit geen datum uit het dossier vraagt', () => {
    expect(decisionDue({}, '2030-01-01', ['2026-12-31'])).toBe(false);
  });

  it('heeft het zonder datum uit het dossier op de vaste datum die de wet geeft', () => {
    expect(decisionDue({ aanslag: null }, '2026-12-30', ['2026-12-31'])).toBe(false);
    expect(decisionDue({ aanslag: null }, '2026-12-31', ['2026-12-31'])).toBe(true);
    expect(decisionDue({ aanslag: null }, '2030-01-01')).toBe(false);
  });
});

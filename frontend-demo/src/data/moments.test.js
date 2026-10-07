import { describe, expect, it } from 'vitest';
import { addDays, addMonths, comingDates, decisionDue, fixedDates, isEnded, nextExecution, nextMoment, periodEnd } from './moments.js';

describe('datums', () => {
  it('telt dagen en maanden over de grens van een jaar', () => {
    expect(addDays('2024-12-31', 1)).toBe('2025-01-01');
    expect(addDays('2024-02-28', 1)).toBe('2024-02-29');
    expect(addMonths('2024-11-20', 2)).toBe('2025-01-20');
    expect(addMonths('2025-01-31', 1)).toBe('2025-02-28');
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

describe('de volgende uitvoering', () => {
  const ended = () => Object.assign(new Error('beëindigd'), { name: 'ended' });

  it('is de eerste dag die de cel geeft waarop de wet een gram geeft', () => {
    const asked = [];
    const preview = (day) => {
      asked.push(day);
      return day === '2025-01-01' ? { fields: { termijnbedrag: 100 } } : null;
    };
    const days = ['2024-12-01', '2025-01-01', '2025-02-01'];
    expect(nextExecution(preview, days)).toEqual({ date: '2025-01-01', gram: { fields: { termijnbedrag: 100 } } });
    expect(asked).toEqual(['2024-12-01', '2025-01-01']);
  });

  it('is er niet als de wet er geen meer geeft of de uitvoering is beëindigd', () => {
    expect(nextExecution(() => null, ['2025-12-01'])).toBeNull();
    expect(nextExecution(() => null, [])).toBeNull();
    expect(nextExecution(() => { throw ended(); }, ['2025-12-01'])).toBeNull();
    expect(isEnded(ended())).toBe(true);
  });

  it('laat elke andere fout van de cel door', () => {
    expect(() => nextExecution(() => { throw new Error('kapot'); }, ['2025-12-01'])).toThrow('kapot');
    expect(isEnded(new Error('x'))).toBe(false);
  });
});

describe('de datums die de wet een besluit geeft', () => {
  const fields = { uiterste: { type: 'date' }, uitbetalen: { type: 'date' }, bedrag: { type: 'amount' } };

  it('houdt alleen de datums die niet met de besluitdag meeschuiven', () => {
    // Twee voorbeelden een maand uit elkaar: wat vier weken na de dagtekening
    // valt, schuift mee; het eind van het jaar erna niet.
    const nu = { fields: { uiterste: '2026-12-31', uitbetalen: '2026-05-13', bedrag: 1 } };
    const maandLater = { fields: { uiterste: '2026-12-31', uitbetalen: '2026-06-12', bedrag: 1 } };
    expect(fixedDates(nu, maandLater, fields, '2026-04-15')).toEqual([{ name: 'uiterste', date: '2026-12-31' }]);
    expect(fixedDates(nu, maandLater, fields, '2026-12-31')).toEqual([]);
    expect(fixedDates(nu, maandLater, fields)).toEqual([{ name: 'uiterste', date: '2026-12-31' }]);
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

/**
 * De klok van de store, met een nagebootste cel: één zaak van het voorschot
 * tot na de toekenning. De echte cel draait als WASM en is er in een
 * unittest niet; wat hier bewezen wordt is de volgorde waarin de store haar
 * aanspreekt, niet wat de wet zegt.
 */
import { describe, expect, it } from 'vitest';
import { advanceTo, executeDue } from './clock.js';

/** Een cel die per maand op de eerste een termijn geeft, tot de toekenning. */
function fakeCel() {
  const grams = [{ id: 'v', name: 'voorschot', effective_at: '2025-03-10T09:00:00', recorded_at: '2025-03-10T09:00:00' }];
  const calls = [];
  const toekenning = () => grams.find((g) => g.name === 'toekenning');
  const firstOfMonths = (from, through) => {
    const out = [];
    let [y, m] = from.split('-').map(Number);
    for (;;) {
      const day = `${y}-${String(m).padStart(2, '0')}-01`;
      if (day > through) return out;
      if (day > from) out.push(day);
      [y, m] = m === 12 ? [y + 1, 1] : [y, m + 1];
    }
  };
  return {
    grams,
    calls,
    dueExecutions(event, root, after, through, now) {
      calls.push(['due', after, through, now]);
      if (toekenning()) return [];
      return firstOfMonths(after ?? '2025-03-10', through).map((day) => ({ day }));
    },
    execute(event, root, day, now) {
      calls.push(['execute', day, now]);
      if (toekenning()) throw Object.assign(new Error('beëindigd'), { name: 'ended' });
      if (day > now.slice(0, 10)) throw new Error(`${day} ligt na ${now}`);
      const gram = { id: `t${day}`, name: 'termijn', effective_at: `${day}T00:00:00`, recorded_at: now };
      grams.push(gram);
      return gram;
    },
    decide(now) {
      calls.push(['decide', now]);
      grams.push({ id: 'k', name: 'toekenning', effective_at: now, recorded_at: now });
    },
  };
}

describe('de klok', () => {
  it('loopt een zaak van het voorschot tot na de toekenning, langs elk moment en nooit terug', () => {
    const cel = fakeCel();
    const c = { id: 'z', applicationGramId: 'a' };
    let today = '2025-03-10';
    const days = [];
    const now = () => `${today}T10:00:00`;
    const aanslag = '2025-06-15';
    advanceTo('2025-09-20', {
      today: () => today,
      setToday: (day) => {
        expect(day > today).toBe(true);
        today = day;
        days.push(day);
      },
      cases: () => [c],
      momentsOf: () => {
        const next = cel.dueExecutions('termijn', 'a', today, '2026-12-31', now())[0]?.day;
        return [{ date: next }, { date: aanslag }].filter((m) => m.date);
      },
      step: (x) => {
        executeDue(x, cel, ['termijn'], { today, now: now() });
        if (today >= aanslag && !cel.grams.some((g) => g.name === 'toekenning')) cel.decide(now());
      },
    });

    // Elke eerste van de maand, en de dag van de aanslag, dan het doel.
    expect(days).toEqual(['2025-04-01', '2025-05-01', '2025-06-01', '2025-06-15', '2025-09-20']);
    // Een termijn per verschuldigde dag, één keer gevraagd, vóór de toekenning.
    const executed = cel.calls.filter(([k]) => k === 'execute').map(([, day]) => day);
    expect(executed).toEqual(['2025-04-01', '2025-05-01', '2025-06-01']);
    // Het besluit op zijn moment, en daarna geen termijn meer.
    const order = cel.calls.filter(([k]) => k !== 'due').map(([k, d]) => `${k} ${d.slice(0, 10)}`);
    expect(order.indexOf('decide 2025-06-15')).toBe(order.length - 1);
    expect(c.chronicleError).toBeUndefined();
    // De kroniek loopt alleen vooruit, en geen gram geldt na zijn vastlegging.
    const recorded = cel.grams.map((g) => g.recorded_at);
    expect([...recorded].sort()).toEqual(recorded);
    for (const g of cel.grams) expect(g.effective_at <= g.recorded_at).toBe(true);
    expect(c.executedThrough).toEqual({ termijn: { '-': '2025-06-01' } });
  });

  it('gaat niet terug en blijft staan op een dag die al is', () => {
    let today = '2025-05-01';
    const ops = { today: () => today, setToday: (d) => (today = d), cases: () => [], momentsOf: () => [], step: () => {} };
    advanceTo('2025-04-01', ops);
    advanceTo('2025-05-01', ops);
    expect(today).toBe('2025-05-01');
    advanceTo('2025-05-03', ops);
    expect(today).toBe('2025-05-03');
  });

  it('voert elke dag uit voor de periode die de cel noemt, en houdt per periode bij wat gevraagd is', () => {
    // Het voorschot over 2025 wordt in december in één bedrag betaald; dat
    // over 2026, dezelfde dag verleend, begint ook in december. Wat al voor
    // 2025 is gevraagd, houdt 2026 niet tegen.
    const y = (value) => ({ unit: 'year', value });
    const c = { applicationGramId: 'a' };
    const executed = [];
    let years = [2025];
    let fail = true;
    const cel = {
      dueExecutions: () => years.map((v) => ({ day: '2025-12-10', period: y(v) })),
      execute: (event, root, day, now, period) => {
        if (period === 2026 && fail) throw new Error('kapot');
        executed.push(`${day} ${period}`);
        return { id: `${day}-${period}` };
      },
    };
    const run = () => executeDue(c, cel, ['termijn'], { today: '2025-12-10', now: '2025-12-10T11:00:00' });
    expect(run()).toBe(true);
    expect(c.executedThrough).toEqual({ termijn: { 'year:2025': '2025-12-10' } });
    // Later die dag het voorschot over 2026: zijn eerste termijn valt dezelfde dag.
    years = [2025, 2026];
    expect(run()).toBe(false);
    expect(c.chronicleError).toBe('kapot');
    fail = false;
    expect(run()).toBe(true);
    expect(executed).toEqual(['2025-12-10 2025', '2025-12-10 2026']);
    expect(c.executedThrough).toEqual({ termijn: { 'year:2025': '2025-12-10', 'year:2026': '2025-12-10' } });
  });

  it('zet een fout van de cel op de zaak en vraagt daarna niets meer', () => {
    const c = { applicationGramId: 'a' };
    const cel = {
      dueExecutions: () => [{ day: '2025-04-01' }, { day: '2025-05-01' }],
      execute: () => {
        throw new Error('kapot');
      },
    };
    expect(executeDue(c, cel, ['termijn'], { today: '2025-05-01', now: '2025-05-01T10:00:00' })).toBe(false);
    expect(c.chronicleError).toBe('kapot');
    expect(c.executedThrough).toBeUndefined();
  });
});

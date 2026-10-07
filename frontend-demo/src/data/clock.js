/**
 * De klok van de demo: hoe de tijd vooruit loopt en wat de cel dan vastlegt.
 *
 * Dit is orkestratie, geen wet. Welke dagen een uitvoering (een betaalde
 * termijn) valt, zegt de cel uit de wet (`dueExecutions`); of er op zo'n dag
 * iets ontstaat ook (`execute`). Wanneer een besluit zijn moment heeft, zegt
 * de store. Hier staat alleen de volgorde: langs elk moment van elke zaak,
 * nooit terug, en per dag eerst de uitvoeringen en dan het besluit.
 *
 * Los van de store, zodat de volgorde te testen is met een nagebootste cel.
 */
import { nextMoment } from './moments.js';

/**
 * Leg vast wat er in zaak `c` tot en met `today` uit de wet voortkomt: per
 * uitvoering in `events` de dagen die de cel geeft na wat al gevraagd is
 * (`c.executedThrough`), elk op `now`. Stopt bij de eerste fout van de cel,
 * die op de zaak komt te staan (`c.chronicleError`).
 *
 * `cel` is `{ dueExecutions(event, root, after, through, now), execute(event, root, day, now) }`.
 * Geeft terug of er een gram is vastgelegd.
 */
export function executeDue(c, cel, events, { today, now }) {
  if (!c?.applicationGramId) return false;
  let recorded = false;
  for (const event of events) {
    let days;
    try {
      days = cel.dueExecutions(event, c.applicationGramId, c.executedThrough?.[event] ?? null, today, now);
    } catch (e) {
      c.chronicleError = String(e?.message ?? e);
      return recorded;
    }
    for (const day of days) {
      try {
        if (cel.execute(event, c.applicationGramId, day, now)) recorded = true;
      } catch (e) {
        c.chronicleError = String(e?.message ?? e);
        return recorded;
      }
      c.executedThrough = { ...(c.executedThrough ?? {}), [event]: day };
    }
  }
  return recorded;
}

/**
 * Zet de klok vooruit naar `date`, langs elk moment van elke zaak en niet in
 * één sprong: een zaak waarvan het besluit eerder valt, krijgt het op die
 * dag, en daarna wat erop volgt. Terug kan niet: een gram ligt nooit in de
 * toekomst (RFC-044).
 *
 * `ops`: `today()` en `setToday(day)` (de peildatum), `cases()` (de open
 * zaken), `momentsOf(c)` (wat de wet als volgende momenten van een zaak
 * geeft, `{date}`), en `step(c)`: wat er op de nieuwe dag in een zaak
 * gebeurt (uitvoeren, dan een besluit waarvan het moment er is).
 */
export function advanceTo(date, ops) {
  if (!date || date <= ops.today()) return;
  while (ops.today() < date) {
    const next = nextMoment(
      ops.cases().flatMap((c) => ops.momentsOf(c)),
      ops.today(),
    )?.date;
    ops.setToday(next && next < date ? next : date);
    for (const c of ops.cases()) ops.step(c);
  }
}

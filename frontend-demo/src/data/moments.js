/**
 * De tijd in de demo: welk moment de wet als volgende geeft.
 *
 * De demo heeft één klok, de peildatum (`state.referenceDate`). Die loopt
 * alleen vooruit: een gram ligt nooit in de toekomst (RFC-044), dus terug kan
 * alleen door opnieuw te beginnen. Wat het volgende moment is, staat niet in
 * een kalender hier, maar komt uit de wet en de kroniek: de cel voert de wet
 * uit zonder vast te leggen (een voorbeeld), en wat daar als datum uitkomt is
 * een moment. Dit bestand rekent alleen met datums en met wat de cel
 * teruggeeft; welke gebeurtenis, fase of welk veld het is, weet het niet.
 *
 * Datums zijn `JJJJ-MM-DD`, zonder tijdzone: de wet rekent in dagen.
 */

/** `JJJJ-MM-DD` als middernacht in UTC, om mee te rekenen. */
function utc(date) {
  const [y, m, d] = String(date).slice(0, 10).split('-').map(Number);
  return new Date(Date.UTC(y, m - 1, d));
}

function iso(d) {
  return d.toISOString().slice(0, 10);
}

/**
 * Dezelfde dag `months` kalendermaanden verder, of de laatste dag van die
 * maand als hij korter is. Rekenen met een datum, geen regel van de wet.
 */
export function addMonths(date, months) {
  const d = utc(date);
  const day = d.getUTCDate();
  const last = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + months + 1, 0)).getUTCDate();
  return iso(new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + months, Math.min(day, last))));
}

/** De dag van een moment (RFC 3339) of een datum. */
export function dayOf(moment) {
  return String(moment ?? '').slice(0, 10);
}

/**
 * De laatste dag van een periode zoals een gram haar draagt (`{unit, value}`,
 * RFC-045): het eind van een kalenderjaar. `null` voor een eenheid die de
 * demo niet kent.
 */
export function periodEnd(period) {
  if (period?.unit === 'year' && Number.isInteger(period.value)) return `${period.value}-12-31`;
  return null;
}

/** Het vroegste moment na `date`, of null. */
export function nextMoment(moments, date) {
  return (
    [...(moments ?? [])]
      .filter((m) => m?.date && m.date > date)
      .sort((a, b) => a.date.localeCompare(b.date))[0] ?? null
  );
}

/**
 * Of een fout van de cel zegt dat een uitvoering in deze zaak is beëindigd
 * (de zaak heeft een gram van de fase die haar beëindigt, `until`): dan komt
 * er niets meer. De cel geeft zo'n fout de naam `ended`.
 */
export function isEnded(error) {
  return error?.name === 'ended';
}

/**
 * De eerste van `days` (de dagen die de cel als uitvoeringsdagen geeft,
 * `dueExecutions`) waarop de wet zegt dat er iets ontstaat, gevraagd met
 * `preview(day)` (de cel, zonder vast te leggen): de dag en wat ze zou
 * vastleggen. Welke dagen het zijn, zegt de cel uit de wet; dit bestand kent
 * geen kalender. Alleen de weigering "beëindigd" betekent dat er geen
 * volgende is; elke andere fout gaat door naar de aanroeper.
 */
export function nextExecution(preview, days) {
  for (const day of days ?? []) {
    let gram;
    try {
      gram = preview(day);
    } catch (e) {
      if (isEnded(e)) return null;
      throw e;
    }
    if (gram) return { date: day, gram };
  }
  return null;
}

/**
 * De datumvelden die de wet een besluit geeft, los van de dag waarop het
 * wordt genomen: hetzelfde in twee voorbeelden van het besluit. Een datum die
 * met de besluitdag meeschuift (vier weken na de dagtekening) is nog geen
 * moment; een datum die vaststaat (zes maanden na de aanslag, het eind van
 * het jaar erna) wel. Met `today` alleen wat daarna ligt.
 *
 * Een heuristiek van de demo, geen regel van de wet: de aanroeper vraagt de
 * twee voorbeelden op dagen in verschillende kalendermaanden (de eerste dag
 * en dezelfde dag een maand later, `addMonths`), zodat een datum die per
 * maand meeschuift (de eerste van de volgende maand) niet als vast telt. Dat
 * de wet zelf zegt welke datum vaststaat, is een latere stap.
 */
export function fixedDates(first, second, fields, today = null) {
  const out = [];
  for (const [name, field] of Object.entries(fields ?? {})) {
    if (field?.type !== 'date') continue;
    const value = first?.fields?.[name];
    if (typeof value !== 'string' || value !== second?.fields?.[name]) continue;
    if (!today || value > today) out.push({ name, date: value });
  }
  return out;
}

/** De datumvelden van een gram die na `today` liggen: wat de wet nog geeft. */
export function comingDates(gram, fields, today) {
  return Object.entries(fields ?? {})
    .filter(([name, field]) => field?.type === 'date' && typeof gram?.fields?.[name] === 'string')
    .map(([name]) => ({ name, date: gram.fields[name] }))
    .filter((m) => m.date > today);
}

/**
 * Of het besluit van een fase genomen kan worden.
 *
 * - Geeft het dossier er datums voor (de aanslag van Awir 19), dan als elk
 *   daarvan op of vóór `today` ligt: eerder kan het besluit er niet zijn.
 * - Vraagt het besluit zo'n datum maar heeft het dossier er geen (geen
 *   aanslag), dan op de vaste datum die de wet het besluit geeft (`lawDates`,
 *   uit `fixedDates`; Awir 19 lid 2: uiterlijk 31 december van het jaar erna).
 * - Een besluit dat geen datum uit het dossier vraagt, heeft geen moment dat
 *   de demo kent en komt niet vanzelf.
 */
export function decisionDue(dossierDates, today, lawDates = []) {
  const asked = Object.values(dossierDates ?? {});
  if (asked.length === 0) return false;
  const known = asked.filter((d) => typeof d === 'string');
  if (known.length) return known.every((d) => d <= today);
  return lawDates.some((d) => d <= today);
}

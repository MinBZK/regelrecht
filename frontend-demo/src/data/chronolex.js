import { formatValue } from './format.js';

/**
 * De aanvraag als feit in een kroniek (chronolex, RFC-022), zoals de demo die
 * toont.
 *
 * Wat een aanvraag bevat, zegt de wet: de cel (WasmCell) voert het artikel uit
 * dat de aanvraag vestigt en geeft de velden terug, elk met het artikel dat
 * erom vraagt. Dit bestand is de kant van de demo: welke cel bij een wet hoort,
 * wat de persona invult, en welke grammen bij een zaak horen.
 */

/**
 * De gebeurtenissen van een wet in de cellen van het corpus: het besluit (een
 * gebeurtenis die een artikel van deze wet vestigt) en de aanvraag waarop dat
 * besluit wordt genomen (`produces.decides_on` van dat artikel). `null` als de
 * wet geen besluit in een kroniek legt.
 */
export function eventsForLaw(cells, lawDoc) {
  if (!lawDoc) return null;
  for (const cell of cells ?? []) {
    for (const decision of cell.events) {
      const [lawId, number] = decision.establishes.split('#');
      if (lawId !== lawDoc.$id) continue;
      const article = (lawDoc.articles ?? []).find((a) => String(a.number) === number);
      const on = article?.machine_readable?.execution?.produces?.decides_on ?? [];
      const application = cell.events.find((e) => on.includes(e.establishes));
      if (application) return { cell, decision, application };
    }
  }
  return null;
}

/**
 * Wat de persona op de aanvraag invult, per veld dat de wet vraagt. Alleen
 * wat de demo weet: een veld zonder waarde blijft weg (een onvolledige
 * aanvraag is nog steeds een aanvraag, Awb 4:5). Wat de cel zelf invult (de
 * gevraagde beschikking) vult de persona niet in.
 */
export function applicationValues(shape, { bsn, name, address, date }) {
  const known = {
    bsn,
    aangevraagd_berekeningsjaar: date ? Number(date.slice(0, 4)) : undefined,
    naam_aanvrager: name,
    adres_aanvrager: address,
    dagtekening: date,
    ondertekening: name,
  };
  const values = {};
  for (const field of shape.fields) {
    if (field.fixed != null) continue;
    const value = known[field.name];
    if (value !== undefined && value !== null && value !== '') values[field.name] = value;
  }
  return values;
}

/** Het woonadres van een persona uit de BRP-gegevens, als één regel. */
export function addressOf(persona) {
  const rows = persona?.sources?.RvIG?.verblijfplaats ?? [];
  const home = rows.find((r) => r.type === 'WOONADRES') ?? rows[0];
  if (!home) return undefined;
  return `${home.straat} ${home.huisnummer}, ${home.postcode} ${home.woonplaats}`;
}

/**
 * Het moment van nu op de datum `date` (JJJJ-MM-DD), in de tijdzone van de
 * browser, als RFC 3339. De cel leest daar de dag van ontvangst uit; een
 * moment in UTC zou een aanvraag kort na middernacht een dag eerder leggen.
 */
export function momentOn(date, now = new Date()) {
  const pad = (n) => String(Math.abs(n)).padStart(2, '0');
  const offset = -now.getTimezoneOffset();
  const sign = offset >= 0 ? '+' : '-';
  const time = `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;
  return `${date}T${time}${sign}${pad(Math.trunc(offset / 60))}:${pad(offset % 60)}`;
}

/** De grammen van één zaak: de aanvraag en alles wat ernaar verwijst. */
export function gramsOfCase(grams, applicationId) {
  if (!applicationId) return [];
  return (grams ?? []).filter(
    (g) => g.id === applicationId || Object.values(g.refers_to ?? {}).includes(applicationId),
  );
}

/**
 * Een bepaling (`<regelwerk>#<artikel>[ lid n]`) zoals een mens haar leest:
 * de naam van de wet uit het corpus en het artikel. Een wet die het corpus
 * niet kent, houdt haar id.
 */
export function provisionLabel(corpus, reference) {
  const [lawId, rest = ''] = String(reference).split('#');
  const name = corpus?.lawById?.(lawId)?.name ?? lawId;
  return rest ? `${name}, art. ${rest}` : name;
}

/**
 * De waarde van een veld van een gram zoals een mens haar leest. Een jaartal
 * (een veld dat op `jaar` eindigt) blijft een jaartal: "2026", niet "2.026".
 */
export function fieldText(name, value, spec = null, corpus = null) {
  if (Number.isInteger(value) && /jaar$/.test(name)) return String(value);
  // Een bepaling (de gevraagde beschikking) bij naam: "Zorgtoeslag, art. 2".
  if (typeof value === 'string' && /^[a-z][a-z0-9_]*#\S/.test(value)) return provisionLabel(corpus, value);
  return formatValue(value, spec);
}

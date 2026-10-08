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
 * De gebeurtenissen van een wet in de cellen van het corpus: de besluiten (de
 * gebeurtenissen die een artikel van deze wet vestigt; een besluitartikel in
 * een procedure met meer besluiten vestigt er meer, zoals het voorschot en de
 * toekenning) en de aanvraag waarop die besluiten worden genomen
 * (`produces.decides_on` van dat artikel). `null` als de wet geen besluit in
 * een kroniek legt.
 */
export function eventsForLaw(cells, lawDoc) {
  if (!lawDoc) return null;
  for (const cell of cells ?? []) {
    let application = null;
    const decisions = cell.events.filter((decision) => {
      const [lawId, number] = decision.establishes.split('#');
      if (lawId !== lawDoc.$id) return false;
      const article = (lawDoc.articles ?? []).find((a) => String(a.number) === number);
      const on = article?.machine_readable?.execution?.produces?.decides_on ?? [];
      const found = cell.events.find((e) => on.includes(e.establishes));
      application ??= found ?? null;
      return !!found;
    });
    if (application && decisions.length) return { cell, decisions, application };
  }
  return null;
}

/**
 * De cellen die een wet van organisatie `service` uitvoeren, elk één keer, in
 * de volgorde van de wetten in het corpus.
 */
export function cellsOfService(corpus, service) {
  if (!corpus || !service) return [];
  const found = new Map();
  for (const law of corpus.latestById.values()) {
    if (law.service !== service) continue;
    const cell = eventsForLaw(corpus.cells, law.doc)?.cell;
    if (cell) found.set(cell.id, cell);
  }
  return [...found.values()];
}

/**
 * Wat de persona op de aanvraag invult, per veld dat de wet vraagt. Welke
 * waarde bij welk veld hoort, staat in de configuratie van het profiel
 * (`application` in demo-config.yaml); een waarde `$<naam>` vult de demo in
 * uit `tokens` (`$bsn`, `$reference_date`, `$reference_year`). Een veld zonder
 * waarde blijft weg (een onvolledige aanvraag is nog steeds een aanvraag, Awb
 * 4:5). Wat de cel zelf invult (een veld met `fixed`) vult de persona niet in.
 */
export function applicationValues(shape, configured, tokens = {}) {
  const resolve = (v) => (typeof v === 'string' && v.startsWith('$') ? tokens[v.slice(1)] : v);
  const values = {};
  for (const field of shape?.fields ?? []) {
    if (field.fixed != null) continue;
    const value = resolve(configured?.[field.name]);
    if (value !== undefined && value !== null && value !== '') values[field.name] = value;
  }
  return values;
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

/**
 * De grammen van één zaak: de aanvraag en alles wat er, direct of via een
 * andere gram van de zaak, naar verwijst. Een betaalde termijn verwijst naar
 * het voorschot, en het voorschot naar de aanvraag.
 */
export function gramsOfCase(grams, applicationId) {
  if (!applicationId) return [];
  const ids = new Set([applicationId]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const g of grams ?? []) {
      if (!ids.has(g.id) && Object.values(g.refers_to ?? {}).some((id) => ids.has(id))) {
        ids.add(g.id);
        grew = true;
      }
    }
  }
  return (grams ?? []).filter((g) => ids.has(g.id));
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
 * Waar een bepaling (`<regelwerk>#<artikel>[ lid n]`) in de demo staat: de
 * wet en het artikel, om naar te linken. Een lid wijst naar zijn artikel.
 * `null` als het corpus de wet niet kent: dan valt er niets te openen.
 */
export function provisionTarget(corpus, reference) {
  const [lawId, rest = ''] = String(reference ?? '').split('#');
  if (!lawId || !corpus?.lawById?.(lawId)) return null;
  const article = rest.trim().split(/\s+/)[0] || null;
  return { lawId, article };
}

/**
 * De waarde van een veld van een gram zoals een mens haar leest, naar wat de
 * wet over het veld zegt. `field` is het veld uit de vorm van de gebeurtenis
 * (WasmCell.shape: `type`, `fixed`), `spec` de declaratie in de wet die het
 * besluit neemt. Een veld dat de cel vastzet (de gevraagde beschikking) is
 * een bepaling: bij naam. Een geheel getal zonder eenheid blijft een getal
 * zonder groepering ("2026", niet "2.026"). Zonder declaratie in het
 * besluitartikel (een veld dat een andere wet vraagt of geeft, zoals het
 * geschatte inkomen van Awir 16) leest de waarde naar het type van het veld:
 * een bedrag is een bedrag.
 */
export function fieldText(value, field = null, spec = null, corpus = null) {
  if (field?.fixed != null) return provisionLabel(corpus, value);
  if (field?.type === 'number' && !spec?.type_spec?.unit && Number.isInteger(value)) return String(value);
  return formatValue(value, spec ?? field);
}

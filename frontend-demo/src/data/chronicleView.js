/**
 * Een kroniek zoals een mens haar leest: de velden van een gram, wat er
 * betaald is, en de momenten die de wet nog geeft. Generiek: wat een veld is
 * en hoe het te lezen, zegt de wet (de vorm van de gebeurtenis, `type`, en de
 * declaratie in het artikel dat de gram vestigt). Welke gebeurtenis het is,
 * weet dit bestand niet.
 *
 * `fieldsOf(gram)` geeft de velden van de gebeurtenis van een gram per naam,
 * zoals de cel ze afleidt (de store: `gramFields`).
 */
import { fieldSpec, humanize } from './format.js';
import { fieldText, provisionLabel } from './chronolex.js';
import { t } from '../i18n/index.js';

function specOf(gram, name, corpus) {
  const doc = gram?.regulation ? corpus?.lawById?.(gram.regulation)?.doc : null;
  return doc ? fieldSpec(doc, name) : null;
}

/** Elk veld van een gram, met de waarde zoals een mens haar leest. */
export function gramRows(gram, fields, corpus) {
  return Object.entries(gram?.fields ?? {}).map(([name, value]) => ({
    name,
    text: fieldText(value, fields?.[name], specOf(gram, name, corpus), corpus),
  }));
}

/**
 * De velden van een gram die een burger wil zien: de bedragen en de datums.
 * Ook een bedrag van nul: dat er niets wordt teruggevorderd, is ook een
 * uitkomst. Een datum zonder waarde niet.
 */
export function citizenRows(gram, fields, corpus) {
  return gramRows(gram, fields, corpus).filter(({ name }) => {
    const value = gram.fields[name];
    const type = fields?.[name]?.type;
    if (type === 'amount') return typeof value === 'number';
    if (type === 'date') return typeof value === 'string';
    return false;
  });
}

/**
 * Per bedragveld de som over `grams` (de betaalde termijnen): wat er tot nu
 * toe is ontvangen. Leest als een bedrag zoals het veld dat zegt.
 */
export function amountTotals(grams, fieldsOf, corpus) {
  const totals = new Map();
  for (const gram of grams ?? []) {
    const fields = fieldsOf(gram);
    for (const [name, value] of Object.entries(gram.fields ?? {})) {
      if (fields?.[name]?.type !== 'amount' || typeof value !== 'number') continue;
      const total = totals.get(name) ?? { name, value: 0, count: 0, gram, field: fields[name] };
      total.value += value;
      total.count += 1;
      totals.set(name, total);
    }
  }
  return [...totals.values()].map((x) => ({
    name: x.name,
    value: x.value,
    count: x.count,
    text: fieldText(x.value, x.field, specOf(x.gram, x.name, corpus), corpus),
  }));
}

/**
 * Eén moment uit `nextMoments` als regel: wat het is (`text`), waar het
 * vandaan komt (`supporting`) en, bij een uitvoering, wat de cel dan zou
 * vastleggen (`value`). `decided` zegt of het besluit waar een datum bij
 * hoort al genomen is.
 */
export function momentView(m, { corpus, fieldsOf, decided = () => false }) {
  const provision = m.provision ? provisionLabel(corpus, m.provision) : '';
  if (m.kind === 'execution') {
    const value = gramRows(m.gram, fieldsOf(m.gram), corpus)
      .map((r) => r.text)
      .join(' · ');
    return { ...m, text: humanize(m.name), supporting: t('zaak.moments.execution'), value };
  }
  if (m.kind === 'period_end') {
    return { ...m, text: t('zaak.moments.period_end', { period: m.period.value }), supporting: t('zaak.moments.period_end.supporting') };
  }
  if (m.kind === 'dossier') return { ...m, text: humanize(m.name), supporting: t('zaak.moments.dossier') };
  const key = decided(m.event) ? 'zaak.moments.given' : 'zaak.moments.expected';
  return { ...m, text: humanize(m.name), supporting: t(key, { provision, event: humanize(m.event) }) };
}

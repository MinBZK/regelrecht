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
 * Wat een lexostatus van de cel geeft (`values`, uit `read`), als regels
 * zoals een mens ze leest: per parameter de waarde, gelezen als het veld van
 * de gram waaruit de cel haar afleidt (`fields`, uit `lexostatusFields`: een
 * som van termijnbedragen is een bedrag). Wat de cel optelt, telt dit bestand
 * niet na.
 */
export function lexostatusRows(values, fields, corpus) {
  return Object.entries(values ?? {}).map(([name, value]) => {
    const field = fields?.[name] ?? null;
    const lawId = field?.declared_by?.split('#')[0];
    const doc = lawId ? corpus?.lawById?.(lawId)?.doc : null;
    return { name, value, text: fieldText(value, field, doc ? fieldSpec(doc, field.name) : null, corpus) };
  });
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

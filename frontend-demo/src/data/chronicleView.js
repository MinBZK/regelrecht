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

/**
 * De declaratie van parameter `name` in een van de wetten waarop de gram of
 * een van haar velden rust (`legal_basis`, `field_basis`): een invoer die een
 * andere wet vraagt of geeft, zoals het geschatte inkomen van Awir 16, staat
 * niet in de wet die besluit.
 */
function basisSpecOf(gram, name, corpus) {
  const references = [...(gram?.legal_basis ?? []), ...Object.values(gram?.field_basis ?? {}).flat()];
  for (const reference of new Set(references)) {
    const doc = corpus?.lawById?.(String(reference).split('#')[0])?.doc;
    const spec = doc ? fieldSpec(doc, name) : null;
    if (spec) return spec;
  }
  return null;
}

/** Elk veld van een gram, met de waarde zoals een mens haar leest. */
export function gramRows(gram, fields, corpus) {
  return Object.entries(gram?.fields ?? {}).map(([name, value]) => ({
    name,
    text: fieldText(value, fields?.[name], specOf(gram, name, corpus), corpus),
  }));
}

/**
 * De invoer van een besluit (`inputs`: per parameter `{value, provenance}`)
 * met de waarde zoals een mens haar leest: naar de declaratie in de wet die
 * het besluit neemt, en zonder declaratie een geheel getal als getal ("2025",
 * niet "2.025"). De herkomst blijft zoals de cel haar vastlegt.
 */
export function inputRows(gram, corpus) {
  return Object.entries(gram?.inputs ?? {}).map(([name, input]) => {
    const value = input && typeof input === 'object' && 'value' in input ? input.value : input;
    const spec = specOf(gram, name, corpus) ?? basisSpecOf(gram, name, corpus);
    const field = !spec && Number.isInteger(value) ? { type: 'number' } : null;
    return { name, value, provenance: input?.provenance ?? null, text: fieldText(value, field, spec, corpus) };
  });
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
    const supporting = m.period ? t('zaak.moments.execution.period', { period: m.period.value }) : t('zaak.moments.execution');
    return { ...m, text: humanize(m.name), supporting, value };
  }
  if (m.kind === 'period_end') {
    return { ...m, text: t('zaak.moments.period_end', { period: m.period.value }), supporting: t('zaak.moments.period_end.supporting') };
  }
  if (m.kind === 'dossier') {
    const supporting = m.period ? t('zaak.moments.dossier.period', { period: m.period.value }) : t('zaak.moments.dossier');
    return { ...m, text: humanize(m.name), supporting };
  }
  // Een volgend besluit op dezelfde aanvraag, op de dag die het beleid van
  // de houder geeft (een voorschot voor het jaar erna).
  if (m.kind === 'decision') {
    const text = `${humanize(m.event)} · ${t('zaak.moments.decision', { period: m.period?.value ?? '' })}`;
    return { ...m, text, supporting: t('zaak.moments.decision.supporting', { provision }) };
  }
  // Een besluit dat een cel ambtshalve over de persoon van de zaak neemt (de
  // aanslag van de inspecteur), op de dag die het beleid van de houder geeft.
  if (m.kind === 'ex_officio') {
    const text = `${humanize(m.event)} · ${t('zaak.moments.ex_officio', { period: m.period?.value ?? '' })}`;
    return { ...m, text, supporting: t('zaak.moments.decision.supporting', { provision }) };
  }
  const key = decided(m.event) ? 'zaak.moments.given' : 'zaak.moments.expected';
  return { ...m, text: humanize(m.name), supporting: t(key, { provision, event: humanize(m.event) }) };
}

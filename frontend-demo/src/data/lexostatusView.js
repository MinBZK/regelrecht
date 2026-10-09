/**
 * De lexostatussen van een cel zoals een mens ze leest: hoe de cel haar eigen
 * kroniek terugleest, en wat dat nu oplevert. Generiek: welke lexostatus,
 * welk veld of welk beleid het is, weet dit bestand niet. Dat geeft de cel
 * (`WasmCell.lexostatuses`, `WasmCell.readLexostatus`), en wat een uitvoer
 * betekent staat in het regelwerk zelf.
 *
 * Een lexostatus reduceert op een van twee manieren: in de configuratie van
 * de cel (`kind: configuration`: filter, keuze, afleidingen), of met een
 * artikel in het beleid van de houder dat een kroniek van de cel als register
 * leest (`kind: policy`).
 */
import { lexostatusRows } from './chronicleView.js';
import { fieldSpec, humanize } from './format.js';
import { t } from '../i18n/index.js';

/**
 * Of de demo een lexostatus per zaak kan lezen: haar invoer is `root`, en
 * hooguit de periode waarvoor zij leest (`period`, zoals de cel die noemt:
 * het berekeningsjaar van een aanvraag die ook voor de jaren erna geldt).
 */
export function readsPerCase(description) {
  const inputs = description?.inputs ?? [];
  return inputs.includes('root') && inputs.every((i) => i === 'root' || i === description.period);
}

/**
 * De perioden waarover de grammen van een zaak gaan (`period.value`),
 * oplopend en elk één keer: een lexostatus die per periode leest, leest de
 * zaak per periode.
 */
export function periodsOf(grams) {
  const values = (grams ?? []).map((g) => g?.period?.value).filter((v) => Number.isInteger(v));
  return [...new Set(values)].sort((a, b) => a - b);
}

/**
 * De lezingen van `description` voor een zaak met grammen `grams`: één
 * (`null`), of één per periode van de zaak als zij per periode leest.
 */
export function readingPeriods(description, grams) {
  return description?.period ? periodsOf(grams) : [null];
}

/**
 * Wat een lezing van `description` voor de zaak `root` als invoer krijgt:
 * `root`, en de periode als zij per periode leest.
 */
export function readingInputs(description, root, period = null) {
  const inputs = { root };
  if (description?.period && period != null) inputs[description.period] = period;
  return inputs;
}

/**
 * Het filter van een reductie als regels: per kenmerk van de gram de waarde
 * waarop gefilterd wordt. Een waarde `$<naam>` is de invoer `<naam>` van de
 * lexostatus (`input`).
 */
export function filterRows(filter) {
  return Object.entries(filter ?? {})
    .filter(([, value]) => value != null)
    .map(([key, value]) => ({
      key,
      value,
      input: typeof value === 'string' && value.startsWith('$') ? value.slice(1) : null,
    }));
}

/** De regels waarmee een afleiding een parameter uit de gram haalt. */
const RULES = ['field', 'moment', 'filled', 'sum', 'period'];

/**
 * De afleidingen van een reductie: per parameter de regel (`field`,
 * `moment`, `filled`, `sum`, `period`), wat die regel leest (`of`: een veld,
 * een moment of de eenheid van de periode) en de rechtsgrond.
 */
export function derivationRows(derivations) {
  return Object.entries(derivations ?? {}).map(([name, derivation]) => {
    const rule = RULES.find((r) => derivation?.[r] !== undefined) ?? null;
    return { name, rule, of: rule ? derivation[rule] : null, legalBasis: derivation?.legal_basis ?? [] };
  });
}

/**
 * De artikelen van een beleid dat een register leest, elk als bepaling
 * (`<beleid>#<artikel>`) met zijn uitvoer en wat het regelwerk over die
 * uitvoer zegt (`description`).
 */
export function policyArticles(description, lawDoc) {
  return (description?.articles ?? []).map((a) => {
    const article = (lawDoc?.articles ?? []).find((x) => String(x.number) === a.number);
    const declared = article?.machine_readable?.execution?.output ?? [];
    return {
      number: a.number,
      provision: `${description.name}#${a.number}`,
      outputs: a.outputs.map((name) => ({ name, description: declared.find((o) => o.name === name)?.description ?? '' })),
    };
  });
}

/**
 * Wat een lezing gaf (`readLexostatus`: per parameter `{value, provenance}`)
 * als regels zoals een mens ze leest, elk met het artikel waar ze vandaan komt
 * als de cel dat noemt (`provenance.article`, bij een beleid). `fields` zijn
 * de velden van de grammen waaruit de lexostatus leest (`lexostatusFields`);
 * een parameter die een artikel geeft, leest als de uitvoer van dat artikel.
 */
export function readingRows(reading, fields, corpus) {
  const values = {};
  const byName = { ...(fields ?? {}) };
  for (const [name, input] of Object.entries(reading?.values ?? {})) {
    values[name] = input?.value;
    const article = input?.provenance?.article;
    if (article && !byName[name]) {
      // Het type van de uitvoer, zoals het artikel het declareert: een jaar
      // zonder eenheid leest als getal ("2025"), niet als "2.025".
      const type = fieldSpec(corpus?.lawById?.(article.split('#')[0])?.doc, name)?.type;
      byName[name] = { name, declared_by: article, ...(type ? { type } : {}) };
    }
  }
  return lexostatusRows(values, byName, corpus).map((row) => ({
    ...row,
    article: reading.values[row.name]?.provenance?.article ?? null,
  }));
}

/**
 * De grammen waaruit een lezing kwam, met hun plaats in de kroniek zoals de
 * achterkant haar toont (`entries`, uit `storedChronicle`). Een gram die daar
 * niet staat, houdt alleen zijn id (`position: null`).
 */
export function readingGrams(ids, entries) {
  const byId = new Map((entries ?? []).map((e) => [e.gram.id, e]));
  return (ids ?? []).map((id) => {
    const entry = byId.get(id);
    return { id, position: entry?.position ?? null, name: entry?.gram.name ?? null, date: entry?.gram.effective_at?.slice(0, 10) ?? null };
  });
}

// ---- in gewone woorden ------------------------------------------------------
//
// Wat hieronder staat, maakt van de beschrijving van een lexostatus zinnen die
// iemand zonder de configuratie kan lezen: wat zij geeft, waar de cel het
// haalt, wie het gebruikt en waarvoor. Alles volgt uit de beschrijving die de
// cel geeft, de vorm van de gebeurtenissen die haar lezen en de regelwerken;
// geen naam van een lexostatus, veld of wet staat hier.

/** De soort van een gram (`type`): een aanvraag, een besluit of een uitvoering. */
const KINDS = { submission: 'submission', decretogram: 'decision', executogram: 'execution' };
export function gramKind(type) {
  return KINDS[type] ?? null;
}

/** Een naam zoals in een lopende zin: zonder hoofdletter vooraan. */
const inSentence = (name) => {
  const text = humanize(name);
  return text && text !== text.toUpperCase() ? text.charAt(0).toLowerCase() + text.slice(1) : text;
};

/**
 * Waar een reductie uit leest, als zelfstandig naamwoord: de gebeurtenis bij
 * naam als het filter haar noemt ("“Voorschottermijn betaald”"), anders de
 * soort gram ("de aanvraag", "het besluit"). `each` geeft de vorm voor elk van
 * meer grammen ("elke “Voorschottermijn betaald”").
 */
export function sourceNoun(filter, { each = false } = {}) {
  const suffix = each ? '.each' : '';
  if (filter?.event) return t(`lexo.source.event${suffix}`, { event: humanize(filter.event) });
  const kind = gramKind(filter?.type);
  if (kind === 'submission' && filter?.subtype) return t(`lexo.source.subtype${suffix}`, { kind: inSentence(filter.subtype) });
  return t(`lexo.source.${kind ?? 'any'}${suffix}`);
}

/**
 * Een gebeurtenis die een lexostatus leest (`read_by`), met wat de vorm die de
 * cel eruit afleidt (`shape`, WasmCell.shape) erover zegt: haar naam zoals een
 * mens hem leest, of zij een besluit of een uitvoering vastlegt, en de
 * artikelen die eraan meedoen: het artikel dat haar vestigt en elk artikel dat
 * een veld toevoegt (een haak in de fase van het besluit).
 */
export function readerOf(readBy, shape) {
  const articles = [shape?.establishes, ...(shape?.fields ?? []).map((f) => f.declared_by)].filter(Boolean);
  return {
    event: readBy.event,
    label: humanize(readBy.event),
    kind: gramKind(shape?.type),
    articles: [...new Set(articles)],
  };
}

/** Een opsomming in een zin: "a", "a en b", "a, b en c". */
export function listText(items) {
  const list = (items ?? []).filter(Boolean);
  if (list.length < 2) return list[0] ?? '';
  return `${list.slice(0, -1).join(', ')} ${t('lexo.and')} ${list.at(-1)}`;
}

/**
 * Voor wie een lexostatus is, naar de soort van wie haar leest: "het
 * besluit", "de besluiten", "het besluit en de uitvoering". Leeg als niemand
 * haar leest.
 */
export function readersNoun(readers) {
  const count = (kind) => (readers ?? []).filter((r) => r.kind === kind).length;
  const parts = ['decision', 'execution']
    .map((kind) => [kind, count(kind)])
    .filter(([, n]) => n > 0)
    .map(([kind, n]) => t(`lexo.readers.${kind}.${n === 1 ? 'one' : 'other'}`));
  return listText(parts);
}

/** De namen van de gegevens die een lexostatus geeft. */
export function valueNames(description) {
  if (description?.kind === 'policy') return (description.articles ?? []).flatMap((a) => a.outputs);
  return Object.keys(description?.reduction?.derivations ?? {});
}

/**
 * De zin onder de naam van een lexostatus: wat zij geeft en aan wie ("Geeft
 * het besluit 4 gegevens uit de aanvraag").
 */
export function givesText(description, readers) {
  const n = valueNames(description).length;
  const source = description?.kind === 'policy' ? t('lexo.source.register') : sourceNoun(description?.reduction?.filter);
  const who = readersNoun(readers);
  return who ? t.plural(n, 'lexo.gives', { readers: who, source }) : t.plural(n, 'lexo.gives.unread', { source });
}

/**
 * Waar de cel een lexostatus vandaan haalt, in woorden. In de configuratie:
 * welke grammen van haar eigen kroniek, van deze zaak, per periode, en welke
 * ervan ("de laatste versie die geldt"). Bij een beleid: dat zij het leest
 * volgens dat beleid; welk beleid het is, noemt de link ernaast.
 */
export function sourceText(description, actor) {
  if (description?.kind === 'policy') return t('lexo.source.policy', { actor });
  const filter = description?.reduction?.filter ?? {};
  const source = sourceNoun(filter, { each: description?.reduction?.pick === 'all' });
  const parts = [filter.root ? t('lexo.source.of_case', { source }) : source];
  if (description?.period) parts.push(t('lexo.source.per_period', { period: inSentence(description.period) }));
  parts.push(t(`lexo.source.pick.${description?.reduction?.pick ?? 'latest'}`));
  return t('lexo.source.configuration', { actor, what: parts.join(', ') });
}

/**
 * Waar één gegeven vandaan komt, in woorden. In de configuratie volgt dat uit
 * de regel van zijn afleiding en de soort gram ("ingevuld in de aanvraag",
 * "de dag waarop de aanvraag binnenkwam"); bij een beleid uit wat het
 * regelwerk over de uitvoer zegt (`description`).
 */
export function originText(derivation, filter, description = '') {
  if (!derivation) return description || '';
  const submission = gramKind(filter?.type) === 'submission';
  const source = sourceNoun(filter);
  switch (derivation.rule) {
    case 'field':
      return t(submission ? 'lexo.origin.filled_in' : 'lexo.origin.recorded', { source });
    case 'moment':
      return t(submission ? 'lexo.origin.received' : 'lexo.origin.counts', { source });
    case 'filled':
      return t('lexo.origin.filled', { field: inSentence(derivation.of), source });
    case 'sum':
      return t('lexo.origin.sum', { field: humanize(derivation.of), each: sourceNoun(filter, { each: true }) });
    case 'period':
      return t('lexo.origin.period', { source });
    default:
      return t('lexo.rule.unknown');
  }
}

/**
 * Waar een gegeven `name` als invoer dient: elk artikel dat meedoet aan een
 * gebeurtenis die de lexostatus leest (`readers`, uit `readerOf`) en een
 * parameter met die naam heeft, met de gebeurtenissen waarbij. `lawDoc(id)`
 * geeft het regelwerk. Leeg als geen artikel zo'n parameter heeft.
 */
export function usesOf(name, readers, lawDoc) {
  const uses = new Map();
  for (const reader of readers ?? []) {
    for (const provision of reader.articles) {
      const [lawId, number] = provision.split('#');
      const article = (lawDoc(lawId)?.articles ?? []).find((a) => String(a.number) === number);
      const parameters = article?.machine_readable?.execution?.parameters ?? [];
      if (!parameters.some((p) => p.name === name)) continue;
      if (!uses.has(provision)) uses.set(provision, []);
      uses.get(provision).push(reader.label);
    }
  }
  return [...uses].map(([provision, labels]) => ({ provision, readers: labels }));
}

/**
 * Per gegeven van een lexostatus een regel voor de tabel: de naam zoals een
 * mens hem leest, de technische naam, waar het vandaan komt, waar het als
 * invoer dient en de grondslag. `lawDoc(id)` geeft het regelwerk.
 */
export function valueRows(description, readers, lawDoc) {
  if (description?.kind === 'policy') {
    const articles = policyArticles(description, lawDoc(description.name));
    return articles.flatMap((a) =>
      a.outputs.map((o) => ({
        name: o.name,
        label: humanize(o.name),
        origin: originText(null, null, o.description),
        uses: usesOf(o.name, readers, lawDoc),
        basis: [a.provision],
      })),
    );
  }
  const filter = description?.reduction?.filter ?? {};
  return derivationRows(description?.reduction?.derivations).map((d) => ({
    name: d.name,
    label: humanize(d.name),
    origin: originText(d, filter),
    uses: usesOf(d.name, readers, lawDoc),
    basis: d.legalBasis,
  }));
}

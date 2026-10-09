/**
 * De lexostatussen van een cel zoals een mens ze leest: wat de cel op de
 * peildatum uit haar eigen kroniek afleidt, en wat dat nu oplevert.
 * Generiek: welke lexostatus, welk veld of welk beleid het is, weet dit
 * bestand niet. Dat geeft de cel (`WasmCell.lexostatuses`,
 * `WasmCell.readLexostatus`), en wat een gegeven betekent staat in het
 * regelwerk zelf.
 *
 * De wet bepaalt de interface: welke gegevens een besluit vraagt (de
 * parameters van zijn artikel en de haken erop). Hoe de cel ze uit haar
 * kroniek haalt, is de implementatie, en die komt hier op twee manieren
 * voor: afgeleid uit de wet (`kind: submission`: wat de aanvraag zegt, zoals
 * de wet de aanvraag beschrijft), of een artikel in het beleid van de houder
 * dat de kroniek als register leest (`kind: policy`). Elke lexostatus draagt
 * haar vorm (`fields`: naam, type, eenheid) uit dat artikel.
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
 * Wat een lezing gaf (`readLexostatus`: per parameter `{value, provenance}`)
 * als regels zoals een mens ze leest, elk met het artikel waar ze vandaan komt
 * als de cel dat noemt (`provenance.article`). `fields` zijn de gegevens die
 * de lexostatus geeft, per naam, zoals de cel ze beschrijft (`fields` van
 * haar beschrijving); een gegeven dat er niet in staat en dat een artikel
 * geeft, leest als de uitvoer van dat artikel.
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
// iemand zonder de configuratie kan lezen: wat zij is, wat zij geeft en aan
// wie, waar dat is vastgelegd, en per gegeven welke wet erom vraagt en hoe de
// cel het afleidt. Alles volgt uit de beschrijving die de cel geeft, de vorm
// van de gebeurtenissen die haar lezen en de regelwerken; geen naam van een
// lexostatus, veld of wet staat hier.

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

/** De namen van de gegevens die een lexostatus geeft. */
export function valueNames(description) {
  return (description?.fields ?? []).map((f) => f.name);
}

/**
 * Wat een lexostatus is, als kop: bij een aanvraag "Wat er in de aanvraag
 * staat"; bij een artikel van het beleid de gegevens die het geeft
 * ("Uitbetaalde voorschotten"), want een artikel heeft geen opschrift.
 */
export function titleText(description) {
  if (description?.kind === 'submission') return t('lexo.title.submission', { kind: inSentence(description.name) });
  const names = valueNames(description);
  if (!names.length) return humanize(description?.name);
  return listText([humanize(names[0]), ...names.slice(1).map(inSentence)]);
}

/**
 * De zin onder de kop: aan wie zij wat geeft ("Geeft “Voorschot verleend” en
 * “Zorgtoeslag toegekend” 3 gegevens: BSN, aangevraagd berekeningsjaar en
 * datum ontvangst.").
 */
export function givesText(description, readers) {
  const names = valueNames(description);
  const values = listText(names.map(inSentence));
  const who = listText((readers ?? []).map((r) => t('lexo.reader', { event: r.label })));
  return who ? t.plural(names.length, 'lexo.gives', { readers: who, values }) : t.plural(names.length, 'lexo.gives.unread', { values });
}

/**
 * Het type van een gegeven in woorden ("datum", "bedrag in eurocent"), zoals
 * het artikel het declareert.
 */
export function typeText(field) {
  const type = field?.type;
  if (!type) return t('lexo.type.unknown');
  const word = t(`lexo.type.${type}`);
  return field.unit ? t('lexo.type.with_unit', { type: word, unit: field.unit }) : word;
}

/**
 * De vorm van wat een lexostatus geeft, als klein schema:
 * `{ bsn: tekst, datum_ontvangst: datum }`.
 */
export function shapeText(description) {
  const parts = (description?.fields ?? []).map((f) => `${f.name}: ${typeText(f)}`);
  return parts.length ? `{ ${parts.join(', ')} }` : '{ }';
}

/**
 * Hoe de cel een lexostatus afleidt, in woorden: uit de wet (de aanvraag,
 * zoals het artikel dat haar vestigt en de haken erop haar beschrijven), of
 * met een artikel in het beleid van de houder.
 */
export function sourceText(description, actor) {
  if (description?.kind === 'policy') return t('lexo.source.policy', { actor });
  return t('lexo.source.law', { actor, kind: inSentence(description?.name) });
}

/**
 * Hoe de cel één gegeven afleidt, in woorden: bij de aanvraag ingevuld door
 * de aanvrager, of de dag waarop zij binnenkwam; bij een beleid wat het
 * regelwerk over de uitvoer zegt (`description`).
 */
export function originText(description, field) {
  if (description?.kind === 'submission') {
    const kind = inSentence(description.name);
    return t(field?.moment ? 'lexo.origin.received' : 'lexo.origin.filled_in', { kind });
  }
  return field?.description ?? '';
}

/**
 * Welk artikel om een gegeven `name` vraagt: elk artikel dat meedoet aan een
 * gebeurtenis die de lexostatus leest (`readers`, uit `readerOf`) en een
 * parameter met die naam heeft, met de gebeurtenissen waarbij. Dat is de
 * interface die de wet de cel oplegt. `lawDoc(id)` geeft het regelwerk. Leeg
 * als geen artikel zo'n parameter heeft.
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
 * mens hem leest, de technische naam en het type, welke wet erom vraagt
 * (interface), hoe de cel het afleidt (implementatie) en de grondslag.
 * `lawDoc(id)` geeft het regelwerk.
 */
export function valueRows(description, readers, lawDoc) {
  return (description?.fields ?? []).map((f) => ({
    name: f.name,
    label: humanize(f.name),
    type: typeText(f),
    origin: originText(description, f),
    uses: usesOf(f.name, readers, lawDoc),
    basis: f.legal_basis ?? [],
  }));
}

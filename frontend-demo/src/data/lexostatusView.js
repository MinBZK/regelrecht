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
import { fieldSpec } from './format.js';

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
    return { id, position: entry?.position ?? null, name: entry?.gram.name ?? null };
  });
}

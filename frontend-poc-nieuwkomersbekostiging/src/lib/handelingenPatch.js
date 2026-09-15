/**
 * Pure helpers om het uitvoeringslastmodel (data/handelingen.yaml) te lezen en
 * te bewerken: een handeling toevoegen, verwijderen of een veld aanpassen.
 *
 * Waarom hier en niet als tekstpatch zoals yamlPatch.js: een handeling is een
 * lijstitem met een block scalar (`omschrijving: >-`) en een geneste
 * `grondslag`-map. Een regel vervangen gaat daar goed zolang het veld op één
 * regel staat, maar een item toevoegen of verwijderen niet. Deze module leest
 * en herserialiseert daarom het hele document; het commentaar bovenin
 * data/handelingen.yaml gaat daarbij verloren. Dat is voor de werkversie van
 * de assistent acceptabel -- het bronbestand in corpus-poc/ blijft ongemoeid,
 * en de gebruiker ziet de wijziging als diff.
 *
 * Alle functies geven een NIEUWE yaml-tekst terug en muteren hun invoer niet.
 */
// Named imports, geen default: js-yaml 5 heeft geen default-export meer, en dit
// bestand wordt ook buiten de app geladen (de assistent draait het uit
// /app/assistent/app/<casus>/src, tegen zijn eigen node_modules).
import { load, dump } from 'js-yaml';

/** Velden die een handeling mag dragen, met hun verwachte vorm. */
const VELDEN = {
  omschrijving: 'string',
  partij: 'string',
  sector: 'string',
  aanleiding: 'string',
  minuten: 'number',
  tarief: 'string',
  vanaf_jaar: 'number',
};

/** Aanleidingen die de simulatie kan tellen (metrics.js AANLEIDINGEN). */
export const AANLEIDINGEN = [
  'per_school_peildatum',
  'per_leerling_peildatum',
  'per_ambigu_leerling',
  'per_aanvraag',
  'per_aanvraag_1_januari',
  'per_bezwaar',
  'per_school_jaar',
  'per_leerling_zonder_bsn',
  'per_aanvraag_voorbereidingskosten',
];

const PARTIJEN = ['school', 'duo', 'ocw', 'overig'];
const SECTOREN = ['po', 'vo'];

function parse(yamlText) {
  const doc = load(yamlText);
  if (!doc || typeof doc !== 'object') throw new Error('handelingen.yaml is leeg of geen object');
  if (!Array.isArray(doc.handelingen)) throw new Error('handelingen.yaml heeft geen lijst `handelingen`');
  return doc;
}

function serialiseer(doc) {
  return dump(doc, { lineWidth: 100, noRefs: true });
}

/**
 * De handelingen als platte lijst [{id, omschrijving, partij, sector,
 * aanleiding, minuten, tarief, vanaf_jaar}], voor een keuzelijst of om de
 * assistent te laten zien wat er is.
 */
export function listHandelingen(yamlText) {
  const doc = parse(yamlText);
  return doc.handelingen.map((h) => ({
    id: h.id,
    omschrijving: typeof h.omschrijving === 'string' ? h.omschrijving.trim().replace(/\s+/g, ' ') : null,
    partij: h.partij ?? null,
    sector: h.sector ?? null,
    aanleiding: h.aanleiding ?? null,
    minuten: h.minuten ?? null,
    tarief: h.tarief ?? null,
    vanaf_jaar: h.vanaf_jaar ?? null,
  }));
}

/** De tariefnamen die in dit document bestaan (eurocent per uur). */
export function listTarieven(yamlText) {
  const doc = parse(yamlText);
  return { ...(doc.tarieven ?? {}) };
}

/**
 * Controleer één veldwaarde tegen de vorm die de simulatie verwacht.
 * Geeft een foutmelding terug, of null als het veld in orde is.
 */
function keurVeld(veld, waarde, doc) {
  const verwacht = VELDEN[veld];
  if (!verwacht) {
    return `Onbekend veld "${veld}". Toegestaan: ${Object.keys(VELDEN).join(', ')}.`;
  }
  if (verwacht === 'number') {
    if (!Number.isFinite(Number(waarde))) return `Veld "${veld}" moet een getal zijn.`;
    if (veld === 'minuten' && Number(waarde) < 0) return 'Veld "minuten" kan niet negatief zijn.';
    return null;
  }
  if (typeof waarde !== 'string' || !waarde.trim()) return `Veld "${veld}" moet een niet-lege tekst zijn.`;
  if (veld === 'aanleiding' && !AANLEIDINGEN.includes(waarde)) {
    return `Onbekende aanleiding "${waarde}". De simulatie telt alleen: ${AANLEIDINGEN.join(', ')}.`;
  }
  if (veld === 'partij' && !PARTIJEN.includes(waarde)) {
    return `Onbekende partij "${waarde}". Toegestaan: ${PARTIJEN.join(', ')}.`;
  }
  if (veld === 'sector' && !SECTOREN.includes(waarde)) {
    return `Onbekende sector "${waarde}". Toegestaan: ${SECTOREN.join(', ')} (laat weg voor allebei).`;
  }
  if (veld === 'tarief' && !(waarde in (doc.tarieven ?? {}))) {
    const namen = Object.keys(doc.tarieven ?? {}).join(', ');
    return `Onbekend tarief "${waarde}". Bestaande tarieven: ${namen}.`;
  }
  return null;
}

function normaliseer(veld, waarde) {
  return VELDEN[veld] === 'number' ? Number(waarde) : waarde;
}

/**
 * Nieuwe yaml-tekst zonder de handeling met dit id.
 * Gooit als het id niet bestaat -- stil niets doen is precies de "de assistent
 * heeft niets gewijzigd"-ervaring die we willen vermijden.
 */
export function removeHandeling(yamlText, id) {
  const doc = parse(yamlText);
  const index = doc.handelingen.findIndex((h) => h.id === id);
  if (index < 0) {
    throw new Error(`Handeling "${id}" bestaat niet. Bestaande id's: ${doc.handelingen.map((h) => h.id).join(', ')}`);
  }
  const verwijderd = doc.handelingen[index];
  doc.handelingen = doc.handelingen.filter((_, i) => i !== index);
  return { yaml: serialiseer(doc), verwijderd };
}

/**
 * Nieuwe yaml-tekst met een handeling erbij. `handeling` moet een id, een
 * partij, een aanleiding, minuten en een tarief dragen; sector en vanaf_jaar
 * zijn optioneel (geen sector = telt in po en vo).
 */
export function addHandeling(yamlText, handeling) {
  const doc = parse(yamlText);
  const id = handeling?.id;
  if (typeof id !== 'string' || !id.trim()) throw new Error('Een nieuwe handeling heeft een id nodig.');
  if (doc.handelingen.some((h) => h.id === id)) {
    throw new Error(`Handeling "${id}" bestaat al; pas hem aan met wijzig_handeling in plaats van hem toe te voegen.`);
  }
  for (const verplicht of ['partij', 'aanleiding', 'minuten', 'tarief']) {
    if (handeling[verplicht] === undefined || handeling[verplicht] === null) {
      throw new Error(`Een nieuwe handeling heeft "${verplicht}" nodig.`);
    }
  }
  const nieuw = { id };
  for (const veld of Object.keys(VELDEN)) {
    const waarde = handeling[veld];
    if (waarde === undefined || waarde === null) continue;
    const fout = keurVeld(veld, waarde, doc);
    if (fout) throw new Error(fout);
    nieuw[veld] = normaliseer(veld, waarde);
  }
  if (handeling.grondslag && typeof handeling.grondslag === 'object') nieuw.grondslag = handeling.grondslag;
  doc.handelingen = [...doc.handelingen, nieuw];
  return { yaml: serialiseer(doc), toegevoegd: nieuw };
}

/**
 * Nieuwe yaml-tekst met gewijzigde velden op één handeling. `velden` is een
 * map veld -> waarde; een waarde van null verwijdert het veld (zo laat je een
 * handeling voor beide sectoren tellen door `sector` weg te halen).
 */
export function patchHandeling(yamlText, id, velden) {
  const doc = parse(yamlText);
  const handeling = doc.handelingen.find((h) => h.id === id);
  if (!handeling) {
    throw new Error(`Handeling "${id}" bestaat niet. Bestaande id's: ${doc.handelingen.map((h) => h.id).join(', ')}`);
  }
  const entries = Object.entries(velden ?? {});
  if (!entries.length) throw new Error('Geef minstens één veld om te wijzigen.');
  const veranderd = [];
  for (const [veld, waarde] of entries) {
    if (waarde === null) {
      if (!(veld in VELDEN)) throw new Error(`Onbekend veld "${veld}".`);
      if (veld in handeling) {
        veranderd.push(`${veld}: ${handeling[veld]} -> (weg)`);
        delete handeling[veld];
      }
      continue;
    }
    const fout = keurVeld(veld, waarde, doc);
    if (fout) throw new Error(fout);
    const oud = handeling[veld];
    const nieuw = normaliseer(veld, waarde);
    if (oud !== nieuw) veranderd.push(`${veld}: ${oud ?? '(leeg)'} -> ${nieuw}`);
    handeling[veld] = nieuw;
  }
  if (!veranderd.length) throw new Error(`Handeling "${id}" had die waarden al; niets gewijzigd.`);
  return { yaml: serialiseer(doc), veranderd };
}

/** Nieuwe yaml-tekst met een gewijzigd tarief (eurocent per uur). */
export function patchTarief(yamlText, naam, eurocentPerUur) {
  const doc = parse(yamlText);
  if (!doc.tarieven || !(naam in doc.tarieven)) {
    const namen = Object.keys(doc.tarieven ?? {}).join(', ');
    throw new Error(`Onbekend tarief "${naam}". Bestaande tarieven: ${namen}.`);
  }
  const waarde = Number(eurocentPerUur);
  if (!Number.isFinite(waarde) || waarde < 0) throw new Error('Een tarief is een niet-negatief getal in eurocent per uur.');
  const oud = doc.tarieven[naam];
  doc.tarieven = { ...doc.tarieven, [naam]: waarde };
  return { yaml: serialiseer(doc), oud, nieuw: waarde };
}

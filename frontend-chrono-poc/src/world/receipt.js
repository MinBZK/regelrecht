/**
 * Het uitvoeringsreceipt van een decretogram lezen. Niets casus-eigens.
 *
 * Het beeld van de wereld draagt dit receipt niet — het bevat wandkloktijd, en
 * een contract dat per run verschilt is geen contract — maar het gram draagt het
 * wél: een decretogram *is* het RFC-013 Execution Receipt van het besluit. Deze
 * module zet het antwoord van `fetchGramReceipt` om in wat een weergave nodig
 * heeft: secties met regels, de geladen regelingen als tabel, en de waarden die
 * van een ander geaccepteerd zijn met hun bron en hun bevoegd gezag.
 *
 * **Niets weggelaten.** Elke sectie die het receipt draagt komt in beeld, ook een
 * die deze module niet bij naam kent: RFC-013 mag morgen iets toevoegen, en dan
 * hoort dat hier vanzelf te verschijnen in plaats van stil te ontbreken. Wat deze
 * module doet is ordenen en benoemen, niet selecteren.
 */
import { humanize } from './format.js';
import { regulationOf } from './snapshot.js';

/**
 * De volgorde waarin de secties gelezen horen te worden.
 *
 * Niet de alfabetische volgorde waarin ze op de draad staan: een lezer begint bij
 * de vraag "welke wet, welke versie" en eindigt bij de instelling van de engine.
 * Wat hier niet in staat, komt er in de volgorde van het antwoord achteraan.
 */
const SECTION_ORDER = ['provenance', 'scope', 'execution', 'results', 'engine_config'];

/** Hoe een sectie heet in gewone woorden. */
const SECTION_LABELS = {
  provenance: 'Herkomst van de uitvoering',
  scope: 'Bereik van de uitvoering',
  execution: 'Uitvoering',
  results: 'Resultaten',
  engine_config: 'Instelling van de engine',
};

/** Wat een sectie in één regel betekent. */
const SECTION_NOTES = {
  provenance: 'welke regeling in welke versie, door welke engine',
  scope: 'wat er tijdens de uitvoering geladen was',
  execution: 'op welke datum en met welke waarden er gerekend is',
  results: 'wat de regeling vaststelde, en waar elke uitkomst vandaan komt',
  engine_config: 'de keuzes waaronder de engine draaide',
};

/**
 * De velden van het receipt die deze module apart toont en dus niet nog eens in
 * een sectie moeten opduiken.
 */
const OWN_VIEW = ['gram', 'accepted_values', 'timestamp'];

/** De lijst binnen `scope` die als tabel getoond wordt in plaats van als regels. */
const LOADED_REGULATIONS = 'loaded_regulations';

/**
 * Draagt dit gram een uitvoeringsreceipt om op te vragen?
 *
 * Twee eisen, en de tweede is de echte. Een decretogram is een besluit dat de cel
 * zelf nam, maar een **bron-cel zonder engine** legt haar eigen vaststelling ook
 * zo vast: even goed een decretogram, alleen heeft er nooit een uitvoering
 * gedraaid, en dan is er geen receipt. Het beeld laat dat verschil aan één veld
 * zien — de **regeling** — want die schrijft `Decretogram::event` in dezelfde
 * vastlegging als het receipt zelf
 * (`packages/simulator/src/cell/besluit.rs`); een gram dat er geen draagt, heeft
 * ook geen uitvoering achter zich.
 *
 * Zonder die tweede toets zou een uitklap "Receipt" beloven wat de server met een
 * 404 moet weigeren, en een aanbod dat alleen een foutmelding oplevert is erger
 * dan geen aanbod.
 */
export function carriesReceipt(gram) {
  return gram?.kind === 'decretogram' && regulationOf(gram) !== null;
}

/** Het gram waar dit receipt bij hoort; leeg als het antwoord het niet draagt. */
export function receiptGram(receipt) {
  const gram = receipt?.gram;
  return gram && typeof gram === 'object' ? gram : {};
}

/**
 * De secties van het receipt, in leesvolgorde, elk met zijn regels.
 *
 * Een regel is een naam en een waarde; geneste objecten worden uitgevouwen tot
 * regels met een samengestelde naam (`outputs · hoogte_zorgtoeslag`), want dat is
 * wat een lezer wil zien in plaats van JSON in een cel.
 */
export function receiptSections(receipt) {
  const known = SECTION_ORDER.filter((key) => isSection(receipt?.[key]));
  const rest = Object.keys(receipt ?? {})
    .filter((key) => !OWN_VIEW.includes(key) && !known.includes(key))
    .filter((key) => isSection(receipt[key]));
  return [...known, ...rest]
    .map((key) => ({
      key,
      label: SECTION_LABELS[key] ?? humanize(key),
      note: SECTION_NOTES[key] ?? '',
      rows: rows(omit(receipt[key], key === 'scope' ? [LOADED_REGULATIONS] : [])),
    }))
    .filter((section) => section.rows.length > 0);
}

/**
 * De regelingen die tijdens de uitvoering geladen waren, met hun hash.
 *
 * Eigen tabel en geen regels: zonder deze lijst — id, versie, hash — is de
 * uitvoering niet te reproduceren, en dat is te belangrijk om als
 * `scope.loaded_regulations.0.hash` in een opsomming te verdwijnen.
 */
export function loadedRegulations(receipt) {
  const loaded = receipt?.scope?.[LOADED_REGULATIONS];
  if (!Array.isArray(loaded)) return [];
  return loaded.map((regulation) => ({
    id: regulation?.id ?? '',
    validFrom: regulation?.valid_from ?? null,
    validTo: regulation?.valid_to ?? null,
    hash: regulation?.hash ?? null,
  }));
}

/**
 * De waarden die dit besluit van een andere cel accepteerde.
 *
 * Met de bron-cel én het bevoegd gezag dat die cel erbij noemde. Dat tweede is
 * het punt: een cel-id is een adres, en wie het receipt terugleest hoort te zien
 * wiens vaststelling geaccepteerd is (invariant I5).
 */
export function acceptedValues(receipt) {
  const accepted = receipt?.accepted_values;
  if (!Array.isArray(accepted)) return [];
  return accepted.map((value) => ({
    output: value?.output ?? '',
    value: value?.value,
    cell: value?.cell ?? '',
    authority: value?.authority ?? null,
    lexostatus: value?.lexostatus ?? null,
    field: value?.field ?? null,
    opMoment: value?.op_moment ?? null,
    zaakkenmerk: value?.zaakkenmerk ?? null,
    askedBy: value?.asked_by ?? null,
    signature: value?.signature ?? null,
  }));
}

/**
 * De tijdstempel van de uitvoering, met erbij wat voor tijd het is.
 *
 * De toelichting komt van de server en wordt hier niet overgeschreven: dat deze
 * tijd een wandkloktijd is en geen moment in de logische tijd van de wereld, is
 * precies de reden dat het beeld dit receipt niet draagt, en die uitleg hoort
 * uit één mond te komen.
 */
export function receiptTimestamp(receipt) {
  const timestamp = receipt?.timestamp;
  if (!timestamp || typeof timestamp !== 'object') return null;
  return { wallClock: timestamp.wall_clock ?? null, note: timestamp.note ?? '' };
}

/** Is dit iets om als sectie met regels te tonen? */
function isSection(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/** Hetzelfde object zonder de genoemde sleutels. */
function omit(value, keys) {
  return Object.fromEntries(Object.entries(value ?? {}).filter(([key]) => !keys.includes(key)));
}

/** Het scheidingsteken tussen de lagen van een samengestelde naam. */
const PATH_SEPARATOR = ' · ';

/**
 * Eén object als regels: naam, waarde.
 *
 * Geneste objecten en lijsten van objecten worden uitgevouwen; een lijst van
 * losse waarden blijft één regel, want die leest als één opsomming. Een lege
 * lijst of een leeg object komt er als regel in met wat er is — "geen" — in
 * plaats van te verdwijnen: dat een sectie niets te melden had, is zelf iets om
 * te zien.
 */
function rows(value, prefix = '') {
  if (!isSection(value)) return [];
  return Object.entries(value).flatMap(([key, entry]) => {
    const name = prefix ? `${prefix}${PATH_SEPARATOR}${key}` : key;
    if (isSection(entry)) {
      const nested = rows(entry, name);
      return nested.length > 0 ? nested : [{ name, value: null }];
    }
    if (Array.isArray(entry) && entry.some(isSection)) {
      return entry.flatMap((item, index) => rows(item, `${name}${PATH_SEPARATOR}${index + 1}`));
    }
    return [{ name, value: entry }];
  });
}

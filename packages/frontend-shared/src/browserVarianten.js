/**
 * Varianten die de gebruiker zelf bewaart, in de browser.
 *
 * Waarom dit bestaat: "bewaar als variant" maakte een git-branch in de
 * casus-checkout. Hosted staat die er niet, en sinds de verhuizing naar de
 * monorepo klopt het pad ook lokaal niet meer. Er was daarmee geen enkele
 * manier om een wijziging vast te houden: een ververste pagina gooide alles
 * weg. Dit is die manier.
 *
 * Wat een browser-variant is: dezelfde vorm als een ingecheckte variant uit
 * `variants.json` (`{ id, title, files }`), zodat de rest van de app er niets
 * van hoeft te weten. Het enige verschil is waar de tekst vandaan komt, en dat
 * `herkomst: 'browser'` erbij staat zodat de UI hem kan merken.
 *
 * Wat dit bewust niet is: een deelbare variant. Hij leeft in deze browser, op
 * dit apparaat, en is weg als iemand zijn browsergegevens wist. Dat staat ook
 * zo in de tekst bij de knop; een demo-variant die stilletjes verdwijnt is
 * erger dan een die vooraf zegt dat hij dat kan.
 *
 * Over drift: elke variant bewaart per bestand een vingerafdruk van de tekst
 * waarop hij gemaakt is. Wijkt het corpus later af, dan is de bewerking
 * gemaakt op een wet die er niet meer zo staat. `controleerDrift` meldt dat; de
 * app laat de variant dan wel activeren maar zegt erbij dat hij bij een oudere
 * wetsversie hoort. Dit is precies de reden dat bewerkte YAML niet zomaar in de
 * opslag hoorde (zie de kop van useBewaardeStand), dus zonder deze controle
 * zouden we die fout herhalen.
 *
 * Over ruimte: localStorage gaat rond de 5 MB dicht en een wet is hier ~50 KB,
 * dus een variant of veertig past erin. Daarom de vingerafdruk in plaats van de
 * hele basistekst, en daarom meldt `bewaarVariant` het hard als de opslag vol
 * zit: een "bewaard" dat na een ververs weg blijkt, is erger dan een weigering.
 */
import { ref } from 'vue';
import { bewaardeRef, beschermSleutel, bewaarStand } from './useBewaardeStand.js';

/** De sleutel in de bewaarde stand. */
const SLEUTEL = 'varianten';

/**
 * Het voorvoegsel van een browser-variant-id. Houdt ze uit elkaar met de
 * ingecheckte varianten (a1, nk-2, …), die dit voorvoegsel nooit dragen.
 */
export const BROWSER_PREFIX = 'eigen-';

/** Is dit het id van een variant uit de browser? */
export function isBrowserVariant(id) {
  return typeof id === 'string' && id.startsWith(BROWSER_PREFIX);
}

/**
 * Een korte vingerafdruk van een wetstekst, om te zien of hij later gewijzigd
 * is. Een hash en niet de tekst zelf: een wet is hier ~50 KB, en met de tekst
 * erbij kostte elke variant het dubbele van een opslag die op zo'n 5 MB
 * dichtgaat. Dat scheelt de helft, en meer dan "is dit nog dezelfde tekst"
 * hoeven we niet te weten.
 *
 * Geen cryptografie: dit weert geen aanvaller af, het merkt een wijziging op.
 * FNV-1a is daar genoeg voor en heeft geen async nodig, anders dan SubtleCrypto.
 */
export function vingerafdruk(tekst) {
  if (typeof tekst !== 'string') return null;
  let h = 0x811c9dc5;
  for (let i = 0; i < tekst.length; i++) {
    h ^= tekst.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  // De lengte erbij: twee teksten met dezelfde hash maar een andere lengte
  // zijn dan alsnog te onderscheiden, en dat kost vier tekens.
  return `${h.toString(36)}-${tekst.length.toString(36)}`;
}

/**
 * "Begin opnieuw" wist de keuzes van de gebruiker, niet zijn werk. De banner
 * bij die knop belooft met zoveel woorden dat opgeslagen varianten blijven
 * staan, en een variant is het enige hier dat je niet zo weer terughebt.
 */
beschermSleutel(SLEUTEL);

/**
 * De bewaarde varianten. Vorm per stuk:
 *   { id, titel, gemaakt, basis, bestanden: [{ pad, yaml, origineelHash }] }
 * waarin `pad` het indexpad van het basisdocument is (hetzelfde als `base` in
 * variants.json) en `origineelHash` de vingerafdruk van de tekst waarop
 * bewerkt is.
 */
const opgeslagen = bewaardeRef(SLEUTEL, []);

/** Een oplopend nummer dat bumpt bij elke wijziging, om op te watchen. */
const versie = ref(0);

/** Maak van een bewaarde variant de vorm die de lawStore van variants.json kent. */
function alsVariant(v) {
  return {
    id: v.id,
    title: v.titel,
    files: v.bestanden.map((b) => ({ path: b.pad, base: b.pad })),
    herkomst: 'browser',
    gemaakt: v.gemaakt,
    basis: v.basis ?? null,
  };
}

/** Alle browser-varianten, in de vorm van variants.json, nieuwste eerst. */
export function browserVarianten() {
  versie.value;
  return [...opgeslagen.value]
    .sort((a, b) => String(b.gemaakt ?? '').localeCompare(String(a.gemaakt ?? '')))
    .map(alsVariant);
}

/** De rauwe bewaarde vorm van één variant, of null. */
export function browserVariant(id) {
  versie.value;
  return opgeslagen.value.find((v) => v.id === id) ?? null;
}

/**
 * De YAML van één bestand van een browser-variant, of null als deze variant
 * dat bestand niet bewerkt. Dit is wat de lawStore inleest in plaats van een
 * fetch.
 */
export function browserVariantYaml(id, pad) {
  return browserVariant(id)?.bestanden.find((b) => b.pad === pad)?.yaml ?? null;
}

/**
 * Een leesbaar id uit een titel: kleine letters, streepjes, met het
 * browser-voorvoegsel ervoor. Botst hij met een bestaande, dan komt er een
 * nummer achter.
 */
export function maakId(titel, bestaandeIds = []) {
  const kern = String(titel ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40) || 'variant';
  const bezet = new Set(bestaandeIds);
  let id = `${BROWSER_PREFIX}${kern}`;
  for (let n = 2; bezet.has(id); n++) id = `${BROWSER_PREFIX}${kern}-${n}`;
  return id;
}

/** Een titel die overblijft na trimmen en het samentrekken van spaties. */
function keurTitel(titel) {
  const schoon = String(titel ?? '').trim().replace(/\s+/g, ' ');
  if (!schoon) throw new Error('Een variant heeft een titel nodig.');
  return schoon;
}

/** Bestanden moeten een pad en een tekst dragen; anders is de variant stuk. */
function keurBestanden(bestanden) {
  if (!Array.isArray(bestanden) || !bestanden.length) {
    throw new Error('Er is niets bewerkt om te bewaren.');
  }
  for (const b of bestanden) {
    if (!b?.pad || typeof b.yaml !== 'string') {
      throw new Error(`Onbruikbaar bestand in de variant: ${JSON.stringify(b?.pad)}`);
    }
  }
}

/** De bestanden in de bewaarde vorm, met een vingerafdruk van de basistekst. */
function alsBestanden(bestanden) {
  return bestanden.map((b) => ({
    pad: b.pad,
    yaml: b.yaml,
    // De vingerafdruk van de tekst waarop bewerkt is. Hiermee is later te
    // zien of het corpus onder de variant vandaan is gewijzigd, zonder die
    // hele tekst een tweede keer te bewaren.
    origineelHash: vingerafdruk(b.origineel),
  }));
}

/** Schrijf de lijst weg; gooit als de opslag vol zit of uit staat. */
function schrijf(nieuw) {
  // Meteen schrijven en kijken of het lukte, in plaats van het aan de watcher
  // van de ref over te laten. Die slikt een volle opslag stil in, en dan meldt
  // de app "bewaard" terwijl het werk na een ververs weg is.
  if (!bewaarStand(SLEUTEL, nieuw)) {
    throw new Error(
      'De browseropslag zit vol of staat uit, dus deze variant is niet bewaard. '
      + 'Verwijder een eerdere eigen variant en probeer het opnieuw.',
    );
  }
  opgeslagen.value = nieuw;
  versie.value++;
}

/**
 * Bewaar een variant.
 *
 * @param {object} opties
 * @param {string} opties.titel - wat de gebruiker intypte
 * @param {Array<{pad: string, yaml: string, origineel: string}>} opties.bestanden
 * @param {string|null} [opties.basis] - het variant-id waarop voortgeborduurd is
 * @param {string[]} [opties.bestaandeIds] - id's die al bezet zijn (ook de
 *   ingecheckte varianten, zodat een eigen variant er nooit een overschaduwt)
 * @returns {object} de bewaarde variant in variants.json-vorm
 */
export function bewaarVariant({ titel, bestanden, basis = null, bestaandeIds = [] }) {
  const schoon = keurTitel(titel);
  keurBestanden(bestanden);
  const id = maakId(schoon, [...bestaandeIds, ...opgeslagen.value.map((v) => v.id)]);
  const variant = {
    id,
    titel: schoon,
    gemaakt: new Date().toISOString(),
    basis,
    bestanden: alsBestanden(bestanden),
  };
  schrijf([...opgeslagen.value, variant]);
  return alsVariant(variant);
}

/**
 * Werk een bestaande eigen variant bij: dezelfde variant, nieuwe inhoud.
 *
 * Zonder dit kon een variant alleen groeien. Wie in zijn eigen variant
 * doorwerkte en wilde bewaren, kreeg er een tweede naast, met dezelfde naam
 * plus een nummer erachter. Bij de gebruikelijke gang (variant maken,
 * doorrekenen, parameter bijstellen, opnieuw) levert dat een rij bijna-gelijke
 * kolommen op, en de browseropslag heeft een grens.
 *
 * `gemaakt` blijft staan: het is dezelfde variant, en die datum bepaalt de
 * volgorde in de keuzelijst. `bijgewerkt` komt erbij, zodat zichtbaar is dat
 * er sinds het maken aan gewerkt is.
 *
 * @param {string} id - de variant die wordt overschreven
 * @param {object} opties
 * @param {Array<{pad: string, yaml: string, origineel: string}>} opties.bestanden
 * @param {string} [opties.titel] - een nieuwe naam; weggelaten blijft de oude
 * @returns {object} de bijgewerkte variant in variants.json-vorm
 */
export function werkVariantBij(id, { bestanden, titel } = {}) {
  const bestaand = opgeslagen.value.find((v) => v.id === id);
  if (!bestaand) throw new Error(`Deze variant staat niet in deze browser: ${id}`);
  keurBestanden(bestanden);
  const variant = {
    ...bestaand,
    titel: titel === undefined ? bestaand.titel : keurTitel(titel),
    bijgewerkt: new Date().toISOString(),
    bestanden: alsBestanden(bestanden),
  };
  schrijf(opgeslagen.value.map((v) => (v.id === id ? variant : v)));
  return alsVariant(variant);
}

/** Verwijder een bewaarde variant. Geeft terug of er iets weg was. */
export function verwijderVariant(id) {
  const voor = opgeslagen.value.length;
  opgeslagen.value = opgeslagen.value.filter((v) => v.id !== id);
  const weg = opgeslagen.value.length !== voor;
  if (weg) versie.value++;
  return weg;
}

/**
 * Is het corpus gewijzigd sinds deze variant is gemaakt?
 *
 * @param {string} id
 * @param {(pad: string) => string|undefined} huidigeBasis - de basistekst van
 *   een document zoals die nu in de app staat
 * @returns {{afgedreven: boolean, paden: string[], onbekend: boolean}}
 *   `onbekend` als de variant van voor deze controle is en geen vingerafdruk
 *   meedraagt; dan valt er niets te vergelijken en zwijgt de app liever dan
 *   iets te beweren.
 */
export function controleerDrift(id, huidigeBasis) {
  const v = browserVariant(id);
  if (!v) return { afgedreven: false, paden: [], onbekend: false };
  const paden = [];
  let onbekend = false;
  for (const b of v.bestanden) {
    if (typeof b.origineelHash !== 'string') {
      onbekend = true;
      continue;
    }
    const nu = huidigeBasis(b.pad);
    if (typeof nu !== 'string') continue;
    if (vingerafdruk(nu) !== b.origineelHash) paden.push(b.pad);
  }
  return { afgedreven: paden.length > 0, paden, onbekend };
}

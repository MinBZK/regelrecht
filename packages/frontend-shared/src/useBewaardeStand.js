/**
 * useBewaardeStand - keuzes en instellingen overleven een pagina-ververs.
 *
 * Waarom dit één plek is en geen losse localStorage-aanroepen: een casus heeft
 * op drie schermen bij elkaar een stuk of tien instellingen, en die moeten
 * allemaal op dezelfde manier terugkomen, versiebestendig zijn en met één
 * knop te wissen. Dat is precies wat hier gebeurt.
 *
 * Wat wél wordt bewaard: wat de gebruiker zélf koos (welke varianten als
 * kolom, de werkversie, de persona, de keuzes van die persona,
 * populatie-instellingen, sessie-aannames). Wat niet: uitkomsten van
 * berekeningen. Die zijn afgeleid, kosten niets om opnieuw te maken en zouden
 * bij een gewijzigd corpus stilletjes verouderd terugkomen.
 *
 * Bewerkte YAML gaat niet door `bewaardeRef`. Een wetstekst moet tegen het
 * corpus in de repo passen, en die na een ververs blind terugzetten kan
 * betekenen dat je een wijziging bewerkt die niet meer bestaat. Zulke
 * bewerkingen horen in een variant, die zijn uitgangspunt meebewaart en bij
 * drift meldt dat hij bij een oudere wetsversie hoort.
 */
import { ref, watch } from 'vue';

/**
 * Welke casus deze opslag bedient. Bepaalt de sleutelprefix, zodat twee
 * casussen elkaars keuzes niet lezen. Dat is geen theoretisch geval: ze
 * draaien hosted achter hetzelfde portaal en dus op dezelfde origin, waar
 * localStorage gedeeld is. Lokaal scheiden de poorten ze nog wel.
 */
let casus = null;

/**
 * Versie van het opslagformaat. Verhogen zodra bewaarde waarden niet meer
 * kloppen met de code; oude sleutels worden dan genegeerd in plaats van
 * verkeerd ingelezen.
 */
const VERSIE = 1;

/** Alles wat deze casus bewaart, zodat "begin opnieuw" niets mist. */
const sleutels = new Set();

/**
 * Zet de casus. Eén keer aanroepen bij het opstarten van de app, vóór de
 * eerste `bewaardeRef`. Een tweede naam is een fout: er hangen dan al refs aan
 * de oude prefix die stil hun waarde zouden verliezen.
 */
export function stelCasusIn(naam) {
  if (!/^[a-z0-9-]+$/.test(String(naam ?? ''))) {
    throw new Error(`stelCasusIn: ongeldige casusnaam ${JSON.stringify(naam)}`);
  }
  if (casus !== null && casus !== naam) {
    throw new Error(`stelCasusIn: casus staat al op '${casus}' en kan niet naar '${naam}'`);
  }
  casus = naam;
}

function prefix() {
  if (casus === null) {
    // Hard, want het alternatief is stil in de verkeerde of in een naamloze
    // ruimte schrijven, en dat merk je pas als iemands keuzes weg zijn.
    throw new Error('useBewaardeStand: stelCasusIn() is nog niet aangeroepen');
  }
  return `${casus}:v${VERSIE}:`;
}

function lees(naam, standaard) {
  try {
    const rauw = localStorage.getItem(prefix() + naam);
    if (rauw === null) return standaard;
    return JSON.parse(rauw);
  } catch {
    // Kapotte of onleesbare waarde: begin met de standaard in plaats van de
    // pagina te laten struikelen.
    return standaard;
  }
}

/**
 * Schrijf een waarde weg. Geeft terug of dat lukte.
 *
 * Voor de meeste dingen hier (welke kolom, welke persona, de stand van een
 * schuifje) is een mislukte schrijfactie geen ramp: in privémodus werkt de
 * demo dan gewoon zonder onthouden, en de aanroeper negeert de uitkomst.
 * Voor iets dat de gebruiker zelf gemaakt heeft, is het dat wél, en die
 * aanroeper kijkt dus naar wat hier uitkomt.
 */
function schrijf(naam, waarde) {
  try {
    localStorage.setItem(prefix() + naam, JSON.stringify(waarde));
    return true;
  } catch {
    return false;
  }
}

/**
 * Een ref die zichzelf bewaart. Gebruik dit in plaats van ref() voor alles
 * wat de gebruiker instelt.
 *
 * @param {string} naam - unieke sleutel binnen deze casus
 * @param {*} standaard - waarde als er niets bewaard is
 * @param {object} opties
 * @param {(waarde: any) => boolean} [opties.geldig] - controle op de gelezen
 *   waarde; bij false wordt de standaard gebruikt. Handig voor id's die naar
 *   een variant verwijzen die niet meer bestaat.
 * @returns {import('vue').Ref}
 */
export function bewaardeRef(naam, standaard, { geldig } = {}) {
  sleutels.add(naam);
  const gelezen = lees(naam, standaard);
  const start = geldig && gelezen !== standaard && !geldig(gelezen) ? standaard : gelezen;
  const r = ref(start);
  watch(r, (nieuw) => schrijf(naam, nieuw), { deep: true });
  return r;
}

/** Losse waarde lezen zonder ref, voor code die zelf al reactief is. */
export function leesStand(naam, standaard) {
  sleutels.add(naam);
  return lees(naam, standaard);
}

/** Losse waarde bewaren zonder ref. Geeft terug of het lukte. */
export function bewaarStand(naam, waarde) {
  sleutels.add(naam);
  return schrijf(naam, waarde);
}

/**
 * Sleutels die `wisStand` laat staan. "Begin opnieuw" wist de keuzes van de
 * gebruiker, niet zijn werk: de banner bij die knop belooft met zoveel woorden
 * dat opgeslagen varianten blijven staan, en een variant is het enige dat hier
 * niet opnieuw te maken is.
 */
const beschermd = new Set();

/** Meld een sleutel die "begin opnieuw" niet mag wissen. */
export function beschermSleutel(naam) {
  beschermd.add(naam);
}

/**
 * Wis alles wat deze casus onthoudt en herlaad, zodat elke composable met
 * zijn eigen standaardwaarde begint. Herladen is hier eerlijker dan de refs
 * terugzetten: er hangen simulaties, engines en workers aan die stand, en die
 * allemaal met de hand terugdraaien is precies waar bugs in kruipen.
 */
export function wisStand() {
  try {
    const p = prefix();
    const houden = new Set([...beschermd].map((naam) => p + naam));
    for (const naam of [...sleutels]) {
      if (!houden.has(p + naam)) localStorage.removeItem(p + naam);
    }
    // Ook sleutels van een vorige sessie die deze keer nog niet zijn
    // geregistreerd, bijvoorbeeld van een scherm dat je niet hebt geopend.
    for (const key of Object.keys(localStorage)) {
      if (key.startsWith(p) && !houden.has(key)) localStorage.removeItem(key);
    }
  } catch {
    // Niets te wissen als opslag niet beschikbaar is.
  }
  window.location.reload();
}

/** Is er iets bewaard? Bepaalt of de "begin opnieuw"-knop zin heeft. */
export function heeftBewaardeStand() {
  try {
    const p = prefix();
    const houden = new Set([...beschermd].map((naam) => p + naam));
    return Object.keys(localStorage).some((k) => k.startsWith(p) && !houden.has(k));
  } catch {
    return false;
  }
}

/** Alleen voor tests: vergeet de casus en de geregistreerde sleutels. */
export function _resetVoorTest() {
  casus = null;
  sleutels.clear();
  beschermd.clear();
}

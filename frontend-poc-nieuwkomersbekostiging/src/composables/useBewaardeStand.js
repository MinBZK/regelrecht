/**
 * useBewaardeStand - keuzes en instellingen overleven een pagina-ververs.
 *
 * Waarom dit één plek is en geen losse localStorage-aanroepen: de demo heeft
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
 * Bewerkte YAML blijft ook buiten de opslag. Dat is een wetstekst die tegen
 * een corpus in de repo aan moet passen; die na een ververs terugzetten kan
 * betekenen dat je een wijziging bewerkt die niet meer bestaat. Zulke
 * bewerkingen horen in een variant-branch ("Bewaar als variant"), niet in de
 * browser.
 */
import { ref, watch } from 'vue';

/**
 * De twee casussen draaien op verschillende poorten en dus op verschillende
 * origins; localStorage is daarmee al gescheiden. De prefix is er voor de
 * leesbaarheid in de devtools en om onze sleutels te kunnen wissen zonder
 * aan andere te komen.
 */
const CASUS = 'ocw';

/**
 * Versie van het opslagformaat. Verhogen zodra bewaarde waarden niet meer
 * kloppen met de code; oude sleutels worden dan genegeerd in plaats van
 * verkeerd ingelezen.
 */
const VERSIE = 1;

const PREFIX = `${CASUS}:v${VERSIE}:`;

/** Alles wat deze casus bewaart, zodat "begin opnieuw" niets mist. */
const sleutels = new Set();

function lees(naam, standaard) {
  try {
    const rauw = localStorage.getItem(PREFIX + naam);
    if (rauw === null) return standaard;
    return JSON.parse(rauw);
  } catch {
    // Kapotte of onleesbare waarde: begin met de standaard in plaats van de
    // pagina te laten struikelen.
    return standaard;
  }
}

function schrijf(naam, waarde) {
  try {
    localStorage.setItem(PREFIX + naam, JSON.stringify(waarde));
  } catch {
    // Privémodus of volle opslag: de demo werkt dan gewoon zonder onthouden.
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

/** Losse waarde bewaren zonder ref. */
export function bewaarStand(naam, waarde) {
  sleutels.add(naam);
  schrijf(naam, waarde);
}

/**
 * Wis alles wat deze casus onthoudt en herlaad, zodat elke composable met
 * zijn eigen standaardwaarde begint. Herladen is hier eerlijker dan de refs
 * terugzetten: er hangen simulaties, engines en workers aan die stand, en die
 * allemaal met de hand terugdraaien is precies waar bugs in kruipen.
 */
export function wisStand() {
  try {
    for (const naam of [...sleutels]) localStorage.removeItem(PREFIX + naam);
    // Ook sleutels van een vorige sessie die deze keer nog niet zijn
    // geregistreerd, bijvoorbeeld van een scherm dat je niet hebt geopend.
    for (const key of Object.keys(localStorage)) {
      if (key.startsWith(PREFIX)) localStorage.removeItem(key);
    }
  } catch {
    // Niets te wissen als opslag niet beschikbaar is.
  }
  window.location.reload();
}

/** Is er iets bewaard? Bepaalt of de "begin opnieuw"-knop zin heeft. */
export function heeftBewaardeStand() {
  try {
    return Object.keys(localStorage).some((k) => k.startsWith(PREFIX));
  } catch {
    return false;
  }
}

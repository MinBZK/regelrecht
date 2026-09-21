/**
 * De pagina's van de app, elk op een eigen hash-adres.
 *
 * Met een portaal: het aanvraagportaal en inzicht in je aanvraag. Met acties in
 * het wereldbestand: een pagina **per actor** (`#/actor/<cel>`), met wat die
 * actor kan doen en wat de wet en haar eigen stand daarover zeggen. En altijd de
 * wereld zelf ("Achter de schermen"). Is de wereld de enige pagina, dan is er
 * geen navigatie.
 *
 * Een hash en geen router: de server serveert één `index.html`, een link als
 * `#/inzicht` werkt zonder dat de server hem kent, en de navigatie van het
 * ontwerpsysteem (`nldd-tab-bar navigation` met `href` per item) levert gewone
 * links op die de browser zelf volgt. Wat hier overblijft is lezen waar de hash
 * staat.
 */
import { onBeforeUnmount, onMounted, ref } from 'vue';

/** De pagina met de wereld: cellen, journaal, observatielog. Er is er altijd één. */
export const WORLD_PAGE = 'wereld';

/** De pagina van één actor; welke, staat achter de schuine streep. */
export const ACTOR_PAGE = 'actor';

/** Het adres van de pagina van één actor. */
export function actorHref(actor) {
  return `#/${ACTOR_PAGE}/${encodeURIComponent(actor)}`;
}

/** De actor uit een hash als `#/actor/burger`; leeg als er geen staat. */
export function actorFromHash(hash) {
  const match = /^#\/actor\/([^/?#]+)/.exec(String(hash ?? ''));
  if (!match) return '';
  try {
    return decodeURIComponent(match[1]);
  } catch {
    return '';
  }
}

/**
 * De pagina's, in de volgorde van de navigatie.
 *
 * Het label van het portaal komt uit het wereldbestand, de actoren uit het beeld;
 * de andere namen zijn van deze app en noemen geen casus. Eén ingang voor alle
 * actoren, en niet een tab per actor: de pagina zelf kiest tussen hen, en de
 * link opent de eerste.
 */
export function pages(portaal, actors = []) {
  const list = [];
  if (portaal) {
    list.push(
      { key: 'portaal', href: '#/portaal', text: portaal.label, icon: 'send' },
      { key: 'inzicht', href: '#/inzicht', text: 'Inzicht in je aanvraag', icon: 'search' },
    );
  }
  if (actors.length > 0) {
    list.push({ key: ACTOR_PAGE, href: actorHref(actors[0]), text: 'Per actor', icon: 'group' });
  }
  if (list.length === 0) return [];
  list.push({ key: WORLD_PAGE, href: `#/${WORLD_PAGE}`, text: 'Achter de schermen', icon: 'binoculars' });
  return list;
}

/** Het eerste stuk van een hash als `#/inzicht`; leeg als er geen staat. */
export function pageFromHash(hash) {
  const match = /^#\/([\w-]+)/.exec(String(hash ?? ''));
  return match ? match[1] : '';
}

/**
 * Welke pagina er nu open hoort te staan.
 *
 * Een adres dat bij deze wereld niet bestaat (geen portaal, geen acties, of een
 * tikfout) valt terug: met een portaal op het portaal, want dat is de ingang
 * voor wie de wereld als aanvrager opent; zonder op de wereld.
 */
export function currentPage(hash, portaal, actors = []) {
  const list = pages(portaal, actors);
  const wanted = pageFromHash(hash);
  if (list.some((page) => page.key === wanted)) return wanted;
  return portaal ? 'portaal' : WORLD_PAGE;
}

/**
 * Welke actor er op de actorpagina open hoort te staan: die uit het adres, of
 * de eerste als het adres er geen of een onbekende noemt.
 */
export function currentActor(hash, actors) {
  const wanted = actorFromHash(hash);
  return actors.includes(wanted) ? wanted : (actors[0] ?? '');
}

/** De hash van het adres, bijgewerkt zodra de browser een andere volgt. */
export function useHash() {
  const hash = ref(typeof window === 'undefined' ? '' : window.location.hash);
  const update = () => {
    hash.value = window.location.hash;
  };
  onMounted(() => window.addEventListener('hashchange', update));
  onBeforeUnmount(() => window.removeEventListener('hashchange', update));
  return hash;
}

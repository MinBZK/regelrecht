/**
 * De pagina's van de app, elk op een eigen hash-adres.
 *
 * Drie pagina's als er een portaal is: het aanvraagportaal, inzicht in je
 * aanvraag, en de wereld zelf ("Achter de schermen"). Zonder portaal is er één
 * pagina en geen navigatie: een wereldbestand zonder `portaal` ziet er precies
 * zo uit als voordat er portalen waren.
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

/**
 * De pagina's, in de volgorde van de navigatie.
 *
 * Het label van het portaal komt uit het wereldbestand; de andere twee zijn
 * namen van deze app en noemen geen casus.
 */
export function pages(portaal) {
  if (!portaal) return [];
  return [
    { key: 'portaal', href: '#/portaal', text: portaal.label, icon: 'send' },
    { key: 'inzicht', href: '#/inzicht', text: 'Inzicht in je aanvraag', icon: 'search' },
    { key: WORLD_PAGE, href: `#/${WORLD_PAGE}`, text: 'Achter de schermen', icon: 'binoculars' },
  ];
}

/** Het eerste stuk van een hash als `#/inzicht`; leeg als er geen staat. */
export function pageFromHash(hash) {
  const match = /^#\/([\w-]+)/.exec(String(hash ?? ''));
  return match ? match[1] : '';
}

/**
 * Welke pagina er nu open hoort te staan.
 *
 * Een adres dat bij deze wereld niet bestaat (geen portaal, of een tikfout)
 * valt terug op de eerste pagina: met een portaal is dat het portaal, want dat
 * is de ingang voor wie de wereld als aanvrager opent; zonder is het de wereld.
 */
export function currentPage(hash, portaal) {
  const list = pages(portaal);
  const wanted = pageFromHash(hash);
  if (list.some((page) => page.key === wanted)) return wanted;
  return list[0]?.key ?? WORLD_PAGE;
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

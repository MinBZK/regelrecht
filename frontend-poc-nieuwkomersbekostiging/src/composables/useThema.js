/**
 * Licht/donker-thema. Het nldd design system kiest zijn palet op
 * `:root[data-scheme]`: de `light-dark()`-tokens hangen aan de `color-scheme`
 * die dat attribuut zet, en de componenten lezen het attribuut bovendien zelf
 * (`_resolveActiveScheme`, plus een MutationObserver erop). 'systeem' is de
 * *afwezigheid* van het attribuut, zodat `prefers-color-scheme` het overneemt.
 *
 * Zet het niet als inline `style.colorScheme` op <html>: dat stuurt de
 * light-dark()-tokens wel aan, maar `_resolveActiveScheme` vindt geen
 * data-scheme, loopt vanaf zijn eigen parent omhoog en komt nooit bij het
 * root-element uit — de componenten blijven dan in hun eigen stand hangen.
 *
 * Dit is dezelfde afspraak als in de editor en lawmaking, die hem via
 * `@regelrecht/frontend-shared` (`applyColorScheme`) toepassen.
 */
import { ref, watchEffect } from 'vue';

const OPSLAGSLEUTEL = 'thema';
const STANDEN = ['systeem', 'licht', 'donker'];

const thema = ref(
  STANDEN.includes(localStorage.getItem(OPSLAGSLEUTEL))
    ? localStorage.getItem(OPSLAGSLEUTEL)
    : 'systeem',
);

watchEffect(() => {
  const schemes = { licht: 'light', donker: 'dark' };
  const scheme = schemes[thema.value];
  if (scheme) {
    document.documentElement.setAttribute('data-scheme', scheme);
  } else {
    document.documentElement.removeAttribute('data-scheme');
  }
  localStorage.setItem(OPSLAGSLEUTEL, thema.value);
});

export function useThema() {
  const volgende = () => {
    const i = STANDEN.indexOf(thema.value);
    thema.value = STANDEN[(i + 1) % STANDEN.length];
  };
  return { thema, volgende };
}

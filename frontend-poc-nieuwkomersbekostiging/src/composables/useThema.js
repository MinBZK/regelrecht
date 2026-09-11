/**
 * Licht/donker-thema. Het nldd design system volgt de CSS `color-scheme`
 * (er is geen aparte thema-API): systeemvoorkeur tenzij expliciet licht of
 * donker geforceerd op het root-element. Keuze wordt onthouden.
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
  const schemes = { systeem: 'light dark', licht: 'light', donker: 'dark' };
  document.documentElement.style.colorScheme = schemes[thema.value];
  localStorage.setItem(OPSLAGSLEUTEL, thema.value);
});

export function useThema() {
  const volgende = () => {
    const i = STANDEN.indexOf(thema.value);
    thema.value = STANDEN[(i + 1) % STANDEN.length];
  };
  return { thema, volgende };
}

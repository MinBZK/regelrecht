/**
 * Licht/donker-thema. Het nldd design system kiest zijn palet op
 * `:root[data-scheme]`: de `light-dark()`-tokens hangen aan de `color-scheme`
 * die dat attribuut zet, en de componenten lezen het attribuut bovendien zelf
 * (`_resolveActiveScheme`, plus een MutationObserver erop).
 *
 * Twee standen en geen drie. "Systeem" was als knop een derde stand die je
 * moest doorlopen om terug te komen waar je was, terwijl het geen uiterlijk is
 * maar een herkomst: het zegt alleen waar de stand vandaan kwam. De
 * systeemvoorkeur bepaalt nu de startstand, en de knop zet daarna licht of
 * donker. Wie nooit klikt, volgt zijn systeem; wie klikt, heeft gekozen.
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
const STANDEN = ['licht', 'donker'];

/** Wat het systeem van de bezoeker wil, als er nog niets gekozen is. */
function systeemStand() {
  try {
    return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'donker' : 'licht';
  } catch {
    return 'licht';
  }
}

// Een eerder bewaarde 'systeem' (van voor deze twee standen) telt als "nog
// niets gekozen", dus die valt terug op wat het systeem nu wil.
const bewaard = localStorage.getItem(OPSLAGSLEUTEL);
const thema = ref(STANDEN.includes(bewaard) ? bewaard : systeemStand());

watchEffect(() => {
  document.documentElement.setAttribute('data-scheme', thema.value === 'donker' ? 'dark' : 'light');
  localStorage.setItem(OPSLAGSLEUTEL, thema.value);
});

export function useThema() {
  const volgende = () => {
    thema.value = thema.value === 'donker' ? 'licht' : 'donker';
  };
  return { thema, volgende };
}

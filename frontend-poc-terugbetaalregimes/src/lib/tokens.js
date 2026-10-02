/**
 * Kleuren uit de design-system-tokens voor grafieken (echarts kent geen CSS-
 * variabelen). De tokens zijn light-dark(); een proefelement lost ze op in de
 * kleur die op dit moment geldt. De hexwaarden zijn alleen terugval.
 */
import { computed } from 'vue';
import { useThema } from '../composables/useThema.js';

export function resolveToken(token, fallback) {
  if (typeof document === 'undefined') return fallback;
  const el = document.createElement('span');
  el.style.cssText = `position:absolute;visibility:hidden;color:var(${token})`;
  document.body.appendChild(el);
  const kleur = getComputedStyle(el).color;
  el.remove();
  return kleur && kleur !== 'rgba(0, 0, 0, 0)' ? kleur : fallback;
}

/** Vulkleur van een Rijkskleur-categorie (oranje, lintblauw, paars, groen, …). */
export function categorieKleur(naam, fallback, variant = 'filled') {
  return resolveToken(`--semantics-categories-${naam}-${variant}-background-color`, fallback);
}

/** Volgt de themakeuze van de app (systeem, licht, donker). */
export function useDonker() {
  const { thema } = useThema();
  return computed(() => {
    if (thema.value === 'donker') return true;
    if (thema.value === 'licht') return false;
    return typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches;
  });
}

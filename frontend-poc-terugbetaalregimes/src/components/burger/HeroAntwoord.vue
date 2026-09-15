<template>
  <nldd-card class="hero">
    <nldd-container padding="32" gap="8">
      <span class="hero-label">Je betaalt nu</span>
      <span class="hero-bedrag">{{ euro(maandbedrag) }}</span>
      <span class="hero-per">per maand</span>
      <p class="hero-onder">{{ onderregel }}</p>
      <p class="hero-regime">{{ regimeZin(regime) }}</p>
      <!-- Ruimte voor de onderbouwing en een eventueel signaal: die horen bij
           dit bedrag, dus staan ze in dezelfde kaart en niet als losse blokken
           eronder. -->
      <div class="hero-extra"><slot></slot></div>
    </nldd-container>
  </nldd-card>
</template>

<script setup>
import { euro } from '../../lib/format.js';
import { regimeZin } from '../../lib/burgerTekst.js';

defineProps({
  maandbedrag: { type: Number, default: null },
  onderregel: { type: String, default: '' },
  regime: { type: String, default: null },
});
</script>

<style scoped>
.hero { width: 100%; }
.hero-label { font-size: 1.05em; color: var(--semantics-content-secondary-color); }
.hero-bedrag {
  display: block;
  font-size: 3.2em;
  font-weight: 700;
  line-height: 1.05;
  font-variant-numeric: tabular-nums;
  color: var(--semantics-content-color);
}
.hero-per { font-size: 1.05em; color: var(--semantics-content-secondary-color); }
.hero-onder {
  margin: var(--primitives-space-8) 0 0;
  font-size: 1.05em;
  line-height: 1.5;
  max-width: 60ch;
  color: var(--semantics-content-color);
}
/* Wat in de slot komt (signaal, onderbouwing) hoort bij het bedrag, maar
   krijgt lucht zodat het niet aan de regimeregel plakt. Een wrapper, want
   ::slotted werkt alleen in shadow DOM en dit is een gewone Vue-slot. */
.hero-extra { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.hero-extra:empty { display: none; }
.hero-extra:not(:empty) { margin-top: var(--primitives-space-16); }
.hero-regime {
  margin: 0;
  font-size: 0.95em;
  color: var(--semantics-content-secondary-color);
}
</style>

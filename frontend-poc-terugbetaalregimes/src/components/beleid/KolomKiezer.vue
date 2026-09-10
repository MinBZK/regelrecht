<template>
  <div class="kk">
    <nldd-toggle-button-group
      v-if="variants.length"
      type="checkbox"
      size="sm"
      accessible-label="Beleidsvarianten naast huidig recht"
    >
      <nldd-toggle-button
        v-for="v in variants"
        :key="v.id"
        :text="chipTitel(v)"
        :accessible-label="kortTitel(v)"
        :title="kortTitel(v)"
        :value="v.id"
        :selected="selectedVariants.includes(v.id) ? true : undefined"
        :disabled="!selectedVariants.includes(v.id) && selectedVariants.length >= MAX_VARIANTEN ? true : undefined"
        @change="onToggle(v.id, $event)"
      ></nldd-toggle-button>
    </nldd-toggle-button-group>

    <p v-else class="kk-leeg">
      Geen varianten gevonden: maak een <code>variant/*</code>-branch die het corpus wijzigt en draai
      <code>just dev</code> opnieuw.
    </p>

    <p class="kk-hint">
      Een kolom tonen is iets anders dan bewerken. Bewerken doe je op de werkversie, en die kies je in de balk
      bovenaan; hier bepaal je alleen wat er naast huidig recht wordt doorgerekend. Deze keuze geldt voor de
      beleidspagina en de persona's samen.
    </p>
  </div>
</template>

<!--
  Kolomkeuze voor de beleidsview: huidig recht staat altijd links, hiernaast
  komen de aangevinkte beleidsvarianten. Dezelfde chips als in de
  nieuwkomerscasus, zodat de twee demo's er hetzelfde uitzien.
-->

<script setup>
import { useLawStore, kortTitel, chipTitel } from '../../engine/lawStore.js';
import { usePopulation, MAX_VARIANTEN } from '../../composables/usePopulation.js';

const { variants } = useLawStore();
const { selectedVariants, toggleVariant } = usePopulation();

/**
 * De component vertelt zelf of hij aan of uit gaat; blind togglen op elk
 * change-event laat de knop en de state uit de pas lopen.
 */
function onToggle(id, event) {
  const aan = event.detail?.selected ?? !selectedVariants.value.includes(id);
  const staatAan = selectedVariants.value.includes(id);
  if (aan === staatAan) return;
  if (aan && selectedVariants.value.length >= MAX_VARIANTEN) return;
  toggleVariant(id);
}
</script>

<style scoped>
.kk { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
/* De knoppen breken hun tekst niet af; op een smal scherm zouden lange
   labels buiten het paneel steken. De groep mag wel afbreken. */
.kk nldd-toggle-button-group { max-width: 100%; }
.kk-leeg, .kk-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>

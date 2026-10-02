<template>
  <div v-if="zichtbaar.length" class="am">
    <nldd-banner
      v-for="m in zichtbaar"
      :key="m.id"
      :variant="m.soort === 'vraag' ? 'warning' : 'accent'"
      dismissible
      @dismiss="$emit('wis', m.id)"
    >
      {{ m.tekst }}
      <nldd-button
        v-if="naarLabel"
        slot="actions"
        size="sm"
        variant="secondary"
        :text="naarLabel"
        @click="$emit('ga')"
      ></nldd-button>
    </nldd-banner>
  </div>
</template>

<!--
  Meldingen van de beleidsassistent, op elke pagina.

  Waarom in de app en niet alleen als browsernotificatie: een notificatie
  verschijnt pas zinvol als de pagina verborgen is. Wie binnen de app naar
  Scenario's is gelopen heeft de pagina gewoon open, en ziet dus niets. Juist
  dat is het geval waar het om begon: doorwerken op een ander tabblad terwijl
  de assistent rekent.

  De melding staat hier los van het assistentpaneel, want dat paneel is op die
  andere pagina's niet gemonteerd.
-->

<script setup>
import { computed } from 'vue';

const props = defineProps({
  /** [{ id, soort, tekst }] */
  meldingen: { type: Array, default: () => [] },
  /** Staat de gebruiker al op de pagina van de assistent? Dan hoeft dit niet. */
  opAssistentPagina: { type: Boolean, default: false },
  /** Tekst van de knop die naar de assistent gaat; leeg laat de knop weg. */
  naarLabel: { type: String, default: '' },
});
defineEmits(['wis', 'ga']);

/**
 * Op de pagina van de assistent zelf zegt een banner niets: daar staat het
 * gesprek al in beeld. Hooguit de laatste twee, want een rij banners duwt de
 * inhoud weg.
 */
const zichtbaar = computed(() => (props.opAssistentPagina ? [] : props.meldingen.slice(-2)));
</script>

<style scoped>
.am {
  display: flex; flex-direction: column; gap: var(--primitives-space-8);
  padding: var(--primitives-space-8) var(--primitives-space-24);
}
</style>

<script setup>
// De handelingen in een deel van de zaak (bij een besluit, of bij de zaak
// zelf), met hun soort en of ze nu kunnen. Openen kiest er een.
import { soortTekst, statusTekst } from '../zaak.js';

defineProps({
  handelingen: { type: Array, required: true },
  label: { type: String, required: true },
});
const emit = defineEmits(['open']);
</script>

<template>
  <nldd-table columns="minmax(240px,2fr) minmax(160px,1fr) minmax(160px,1fr) 110px" :accessible-label="label">
    <nldd-table-row slot="header">
      <nldd-text-cell text="Handeling"></nldd-text-cell>
      <nldd-text-cell text="Soort"></nldd-text-cell>
      <nldd-text-cell text="Stand"></nldd-text-cell>
      <nldd-text-cell text=""></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="h in handelingen" :key="h.naam">
      <nldd-text-cell :text="h.label" :supporting-text="h.artikel"></nldd-text-cell>
      <nldd-text-cell :text="soortTekst(h)"></nldd-text-cell>
      <nldd-text-cell :text="statusTekst(h)" :supporting-text="h.reden ?? ''"></nldd-text-cell>
      <nldd-cell>
        <nldd-button variant="secondary" text="Open" @click="emit('open', h.naam)"></nldd-button>
      </nldd-cell>
    </nldd-table-row>
  </nldd-table>
</template>

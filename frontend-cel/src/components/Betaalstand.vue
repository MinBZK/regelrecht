<script setup>
// De betaalstand: per handeling met een bedrag de uitkomsten van haar proef
// zonder formulier (zoals wat er nog te betalen is), zoals de wet het nu
// uitrekent over de zaak.
defineProps({ stand: { type: Array, required: true } });
</script>

<template>
  <nldd-table columns="minmax(200px,1fr) minmax(240px,1fr) minmax(160px,1fr)" accessible-label="Betaalstand">
    <nldd-table-row slot="header">
      <nldd-text-cell text="Handeling"></nldd-text-cell>
      <nldd-text-cell text="Uitkomst"></nldd-text-cell>
      <nldd-text-cell text="Waarde"></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="s in stand" :key="s.sleutel">
      <nldd-text-cell :text="s.handeling"></nldd-text-cell>
      <nldd-text-cell :text="s.naam"></nldd-text-cell>
      <nldd-text-cell :text="s.waarde"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
</template>

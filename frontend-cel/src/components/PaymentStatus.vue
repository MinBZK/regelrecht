<script setup>
// The payment status: per action with an amount the outputs of its trial
// without a form (such as what is still to be paid), as the law computes it
// over the case now.
defineProps({ entries: { type: Array, required: true } });
</script>

<template>
  <nldd-table columns="minmax(200px,1fr) minmax(240px,1fr) minmax(160px,1fr)" accessible-label="Betaalstand">
    <nldd-table-row slot="header">
      <nldd-text-cell text="Handeling"></nldd-text-cell>
      <nldd-text-cell text="Uitkomst"></nldd-text-cell>
      <nldd-text-cell text="Waarde"></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="e in entries" :key="e.key">
      <nldd-text-cell :text="e.action"></nldd-text-cell>
      <nldd-text-cell :text="e.name"></nldd-text-cell>
      <nldd-text-cell :text="e.value"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
</template>

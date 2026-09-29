<script setup>
// The actions in a part of the case (with a decision, or with the case
// itself), with their kind and whether they can be taken now. Open picks one.
import { kindText, statusText } from '../case.js';

defineProps({
  actions: { type: Array, required: true },
  label: { type: String, required: true },
});
const emit = defineEmits(['open']);
</script>

<template>
  <nldd-table columns="90px minmax(240px,2fr) minmax(160px,1fr) minmax(160px,1fr)" :accessible-label="label">
    <nldd-table-row slot="header">
      <nldd-text-cell text=""></nldd-text-cell>
      <nldd-text-cell text="Handeling"></nldd-text-cell>
      <nldd-text-cell text="Soort"></nldd-text-cell>
      <nldd-text-cell text="Stand"></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="a in actions" :key="a.name">
      <nldd-cell>
        <nldd-button variant="secondary" text="Open" :accessible-label="`Open: ${a.label}`" @click="emit('open', a.name)"></nldd-button>
      </nldd-cell>
      <nldd-text-cell :text="a.label" :supporting-text="a.article"></nldd-text-cell>
      <nldd-text-cell :text="kindText(a)"></nldd-text-cell>
      <nldd-text-cell :text="statusText(a)" :supporting-text="a.reason ?? ''"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
</template>

<template>
  <details class="paneel" :open="open ? true : undefined" @toggle="isOpen = $event.target.open">
    <summary class="paneel-kop">
      <span class="paneel-titel">{{ titel }}</span>
      <nldd-badge v-if="badge" size="sm" color="warning" :text="badge"></nldd-badge>
      <span v-if="!isOpen && samenvatting" class="paneel-samenvatting">{{ samenvatting }}</span>
    </summary>
    <p v-if="subtitel" class="paneel-sub">{{ subtitel }}</p>
    <div class="paneel-inhoud"><slot /></div>
  </details>
</template>

<!--
  Inklapbare sectie (progressive disclosure): dicht toont hij één regel
  samenvatting, open de inhoud. Eén titelstijl voor alle secties in de
  beleidsview, zodat de linkerkolom als een lijst van taken leest.
-->

<script setup>
import { ref } from 'vue';

const props = defineProps({
  titel: { type: String, required: true },
  subtitel: { type: String, default: '' },
  /** Eén regel die staat als de sectie dicht is (huidige stand van de knoppen). */
  samenvatting: { type: String, default: '' },
  badge: { type: String, default: '' },
  open: { type: Boolean, default: false },
});
const isOpen = ref(props.open);
</script>

<style scoped>
.paneel {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-base-background-color);
}
.paneel-kop {
  display: flex; align-items: baseline; gap: var(--primitives-space-8); flex-wrap: wrap;
  padding: var(--primitives-space-12) var(--primitives-space-16);
  cursor: pointer; list-style: none;
}
.paneel-kop::-webkit-details-marker { display: none; }
.paneel-kop::before { content: '▸'; color: var(--semantics-content-secondary-color); width: 1em; }
.paneel[open] > .paneel-kop::before { content: '▾'; }
.paneel-titel { font-weight: 600; font-size: 1.05em; }
.paneel-samenvatting { font-size: 0.9em; color: var(--semantics-content-secondary-color); margin-left: auto; text-align: right; }
.paneel-sub { margin: 0; padding: 0 var(--primitives-space-16); font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.paneel-inhoud { padding: var(--primitives-space-12) var(--primitives-space-16) var(--primitives-space-16); display: flex; flex-direction: column; gap: var(--primitives-space-12); }
</style>

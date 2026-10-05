<script setup>
// A node of the map as a native button, so the graph can be reached by
// keyboard (vue-flow's own node only selects on Enter). A folded law opens
// to its articles; any other node shows its fragment.
import { Handle, Position } from '@vue-flow/core';
import { decidedByCode, nodeText } from '../map.js';

const props = defineProps({ node: { type: Object, required: true } });
defineEmits(['activate']);

const label = () => {
  if (props.node.kind === 'law') return `Open ${nodeText(props.node)}: toon de artikelen`;
  if (decidedByCode(props.node)) return `Toon waar de code ${nodeText(props.node)} bepaalt`;
  return `Toon het fragment van ${nodeText(props.node)}`;
};
</script>

<template>
  <Handle type="target" :position="Position.Left" />
  <button
    type="button"
    class="map-node"
    :class="{ 'map-node--code': decidedByCode(node) }"
    :aria-label="label()"
    @click="$emit('activate', node)"
  >
    {{ nodeText(node) }}
  </button>
  <Handle type="source" :position="Position.Right" />
</template>

<style scoped>
/* A node is a plain button in the canvas: NLDD tokens, the page's font, and
   long names wrap instead of running out of the box. */
.map-node {
  display: block;
  width: 220px;
  padding: var(--primitives-space-6) var(--primitives-space-8);
  font: inherit;
  font-size: var(--primitives-font-size-80);
  line-height: 1.3;
  text-align: center;
  overflow-wrap: anywhere;
  color: var(--semantics-content-color);
  background: var(--semantics-surfaces-base-background-color);
  border: var(--semantics-dividers-thickness) solid var(--semantics-content-secondary-color);
  border-radius: var(--semantics-controls-sm-corner-radius);
  cursor: pointer;
  /* vue-flow turns pointer events off on a node it neither selects nor drags. */
  pointer-events: auto;
}
/* Decided by the code with knowledge of the law or the case: the critical
   colours of the design system, so it stands out on the map. */
.map-node--code {
  color: var(--semantics-buttons-critical-tinted-content-color);
  background: var(--semantics-buttons-critical-tinted-background-color);
  border-color: var(--semantics-buttons-critical-tinted-highlight-border-color);
  border-width: 2px;
}
.map-node:focus-visible {
  outline: var(--semantics-focus-ring-outline);
  outline-offset: var(--semantics-focus-ring-outline-offset);
  box-shadow: var(--semantics-focus-ring-box-shadow);
}
</style>

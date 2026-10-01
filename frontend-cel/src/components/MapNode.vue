<script setup>
// A node of the map as a native button, so the graph can be reached by
// keyboard (vue-flow's own node only selects on Enter). A folded law opens
// to its articles; any other node shows its fragment.
import { Handle, Position } from '@vue-flow/core';
import { nodeText } from '../map.js';

const props = defineProps({ node: { type: Object, required: true } });
defineEmits(['activate']);

const label = () =>
  props.node.kind === 'law'
    ? `Open ${nodeText(props.node)}: toon de artikelen`
    : `Toon het fragment van ${nodeText(props.node)}`;
</script>

<template>
  <Handle type="target" :position="Position.Left" />
  <button type="button" class="map-node" :aria-label="label()" @click="$emit('activate', node)">
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
.map-node:focus-visible {
  outline: var(--semantics-focus-ring-outline);
  outline-offset: var(--semantics-focus-ring-outline-offset);
  box-shadow: var(--semantics-focus-ring-box-shadow);
}
</style>

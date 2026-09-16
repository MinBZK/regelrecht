<script setup>
import { Handle, Position } from '@vue-flow/core';
import { humanize } from '../../data/format.js';

// One source, input or output of a law, with the persona's value when the
// engine produced one. Inputs carry the handle an edge leaves from (left),
// outputs the handle an edge arrives at (right).
const props = defineProps({ data: { type: Object, required: true } });
</script>

<template>
  <div :class="['graph-item', `graph-item--${props.data.kind}`]" :title="props.data.ref ? `${props.data.name} ← ${props.data.ref.regulation}.${props.data.ref.output}` : props.data.name">
    <Handle v-if="props.data.kind === 'inputs'" type="source" :position="Position.Left" />
    <span class="graph-item__name">{{ humanize(props.data.name) }}</span>
    <span v-if="props.data.value !== undefined" class="graph-item__value">{{ props.data.value }}</span>
    <Handle v-if="props.data.kind === 'outputs'" type="target" :position="Position.Right" />
  </div>
</template>

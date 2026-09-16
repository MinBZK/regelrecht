<script setup>
import OrgLogo from '../OrgLogo.vue';
import { useDemo } from '../../store/demoStore.js';

// The frame of one law in the graph: organisation mark, name and executing
// organisation. The boxes with sources, inputs and outputs are child nodes.
const props = defineProps({ data: { type: Object, required: true } });
const { corpus } = useDemo();
</script>

<template>
  <div :class="['graph-law', props.data.selected ? 'graph-law--selected' : '']" :title="props.data.law.id">
    <div class="graph-law__header">
      <OrgLogo :service="props.data.law.service" size="sm" />
      <div class="graph-law__title">
        <span class="graph-law__name">{{ props.data.law.name }}</span>
        <span class="graph-law__service">{{ corpus?.services?.[props.data.law.service]?.name ?? props.data.law.service }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { moduleLabel, moduleLeaf } from '../lib/guide.js';

// One module of the open crate as a row of a list. A nested module is indented
// one step per level with a spacer cell, so the list stays flat and every row is
// a plain link.
const props = defineProps({
  g: { type: Object, required: true },
  module: { type: Object, required: true },
  depth: { type: Number, default: 0 },
});

const label = computed(() => {
  const p = props.module.path;
  if (p === '' || p.startsWith('bin:')) return moduleLabel(p, props.g.route.crate);
  return moduleLeaf(p);
});
const selected = computed(() => props.g.route.module === props.module.path);
const indent = computed(() => String(props.depth * 16));
const visibility = computed(() => (props.module.vis === 'pub' ? null : props.module.vis));
</script>

<template>
  <nldd-list-item :href="g.hrefFor({ module: module.path })" :selected="selected">
    <nldd-spacer-cell v-if="depth > 0" :size="indent" />
    <nldd-text-cell :text="label" :supporting-text="module.doc || 'No module documentation'" />
    <nldd-cell v-if="module.cycleWith.length">
      <nldd-tag size="sm" color="warning" :text="`calls in a cycle with ${module.cycleWith.join(', ')}`" />
    </nldd-cell>
    <nldd-cell v-if="visibility">
      <nldd-tag size="sm" :text="visibility" />
    </nldd-cell>
  </nldd-list-item>
</template>

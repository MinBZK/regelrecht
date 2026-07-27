<script setup>
import { computed, inject } from 'vue';
import { Handle, Position } from '@vue-flow/core';

const props = defineProps({
  id: { type: String, required: true },
  data: { type: Object, required: true },
});

const toggleExpand = inject('toggleExpand');
const expandSubtree = inject('expandSubtree');
const selectNode = inject('selectNode');

const node = computed(() => props.data.node);
const kindLabel = computed(() => node.value.kind);

function onToggle(event) {
  event.stopPropagation();
  toggleExpand?.(props.id);
}

function onSelect() {
  selectNode?.(props.id);
}

function onExpandAll(event) {
  event.stopPropagation();
  expandSubtree?.(props.id);
}
</script>

<template>
  <!-- Handles are hidden via CSS (edges are not user-connectable) but must
       exist for relationship edges to attach to. -->
  <Handle type="target" :position="Position.Left" />
  <Handle type="source" :position="Position.Right" />

  <div class="arch-node__header" @click="onSelect">
    <button
      v-if="data.expandable"
      type="button"
      class="arch-node__toggle"
      :aria-label="data.expanded ? 'Inklappen' : 'Uitklappen'"
      @click="onToggle"
      @dblclick.stop="onExpandAll"
      :title="data.expanded ? 'Inklappen (dubbelklik: alles)' : 'Uitklappen (dubbelklik: hele subtree)'"
    >{{ data.expanded ? '▾' : '▸' }}</button>
    <span v-else class="arch-node__leaf-dot" aria-hidden="true"></span>

    <span class="arch-node__kind">{{ kindLabel }}</span>
    <span class="arch-node__name" :title="node.name">{{ node.name }}</span>
    <span v-if="data.expandable" class="arch-node__count">{{ data.childCount }}</span>
  </div>
</template>

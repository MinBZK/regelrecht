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
  <!-- Eight handles (source + target on each side), hidden via CSS. Edges are
       not user-connectable; the handles exist so buildFlow can attach a
       relationship line to whichever side faces the other endpoint, keeping
       lines from always running left-to-right. -->
  <Handle id="source-top" type="source" :position="Position.Top" />
  <Handle id="target-top" type="target" :position="Position.Top" />
  <Handle id="source-right" type="source" :position="Position.Right" />
  <Handle id="target-right" type="target" :position="Position.Right" />
  <Handle id="source-bottom" type="source" :position="Position.Bottom" />
  <Handle id="target-bottom" type="target" :position="Position.Bottom" />
  <Handle id="source-left" type="source" :position="Position.Left" />
  <Handle id="target-left" type="target" :position="Position.Left" />

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
    <span
      v-if="data.internalCount > 0"
      class="arch-node__internal"
      :title="`${data.internalCount} interne relatie(s) tussen onderliggende onderdelen`"
    >↺ {{ data.internalCount }}</span>
    <span v-if="data.expandable" class="arch-node__count">{{ data.childCount }}</span>
  </div>
</template>

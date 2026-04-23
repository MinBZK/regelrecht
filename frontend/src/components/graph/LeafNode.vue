<script setup>
import { Handle } from '@vue-flow/core';

defineProps({
  data: { type: Object, required: true },
  sourcePosition: { type: String, default: null },
  targetPosition: { type: String, default: null },
});
</script>

<template>
  <div class="leaf-node" :title="data.tooltip || ''">
    {{ data.label }}
  </div>
  <Handle v-if="targetPosition" type="target" :position="targetPosition" />
  <Handle v-if="sourcePosition" type="source" :position="sourcePosition" />
</template>

<style scoped>
.leaf-node {
  padding: 4px 8px;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: default;
}
</style>

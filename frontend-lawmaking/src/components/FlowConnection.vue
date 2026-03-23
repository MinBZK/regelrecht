<template>
  <path
    class="flow-connection"
    :class="{ 'flow-connection--active': isActive }"
    :d="pathData"
    fill="none"
    stroke="var(--color-connection)"
    stroke-width="3"
    :stroke-dasharray="isActive ? 'none' : dashArray"
  />
</template>

<script setup>
import { computed } from 'vue';

const props = defineProps({
  connection: { type: Object, required: true },
  stages: { type: Array, required: true },
  isActive: { type: Boolean, default: false },
  columnWidth: { type: Number, default: 220 },
  rowHeight: { type: Number, default: 80 },
});

const offsetX = 80;
const offsetY = 50;

function getPos(stageId) {
  const stage = props.stages.find((s) => s.id === stageId);
  if (!stage) return { x: 0, y: 0 };
  return {
    x: offsetX + stage.col * props.columnWidth,
    y: offsetY + stage.row * props.rowHeight,
  };
}

const dashArray = computed(() => {
  if (props.connection.type === 'main-continues') return '6 4';
  return 'none';
});

const pathData = computed(() => {
  const from = getPos(props.connection.from);
  const to = getPos(props.connection.to);
  const type = props.connection.type;

  // Straight line for same-column connections
  if (from.x === to.x) {
    return `M ${from.x} ${from.y} L ${to.x} ${to.y}`;
  }

  // S-curve for cross-column connections
  const topY = Math.min(from.y, to.y);
  const bottomY = Math.max(from.y, to.y);
  const midY = topY + (bottomY - topY) * 0.5;
  return `M ${from.x} ${from.y} C ${from.x} ${midY}, ${to.x} ${midY}, ${to.x} ${to.y}`;
});
</script>

<style scoped>
.flow-connection {
  transition: opacity 0.5s ease;
}

.flow-connection:not(.flow-connection--active) {
  opacity: 0;
}

.flow-connection--active {
  opacity: 1;
}
</style>

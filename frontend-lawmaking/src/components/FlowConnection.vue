<template>
  <path
    class="flow-connection"
    :class="{ 'flow-connection--active': isActive }"
    :d="pathData"
    fill="none"
    :stroke="strokeColor"
    :stroke-width="strokeWidth"
    :stroke-dasharray="isActive ? 'none' : dashArray"
    :stroke-dashoffset="isActive ? 0 : dashOffset"
  />
</template>

<script setup>
import { computed } from 'vue';
import { stages } from '../data/flowData.js';

const props = defineProps({
  connection: { type: Object, required: true },
  isActive: { type: Boolean, default: false },
  columnWidth: { type: Number, default: 220 },
  rowHeight: { type: Number, default: 80 },
});

const offsetX = 80;
const offsetY = 50;

function getPos(stageId) {
  const stage = stages.find((s) => s.id === stageId);
  if (!stage) return { x: 0, y: 0 };
  return {
    x: offsetX + stage.col * props.columnWidth,
    y: offsetY + stage.row * props.rowHeight,
  };
}

const strokeColor = computed(() => {
  const type = props.connection.type;
  if (type === 'ci-fork' || type === 'ci-chain' || type === 'ci-return') {
    return 'var(--color-ci)';
  }
  if (type === 'branch-off') return 'var(--color-branch-feature)';
  if (type === 'merge-in') return 'var(--color-branch-feature)';
  if (type === 'main-continues') return 'var(--color-branch-main)';

  // Determine from the "from" node
  const from = stages.find((s) => s.id === props.connection.from);
  if (from?.branch === 'feature') return 'var(--color-branch-feature)';
  return 'var(--color-branch-main)';
});

const strokeWidth = computed(() => {
  if (props.connection.type === 'main-continues') return 3;
  return 2.5;
});

const dashArray = computed(() => {
  if (props.connection.type === 'main-continues') return '6 4';
  return 'none';
});

const dashOffset = '0';

const pathData = computed(() => {
  const from = getPos(props.connection.from);
  const to = getPos(props.connection.to);
  const type = props.connection.type;

  if (type === 'straight' || type === 'ci-chain') {
    return `M ${from.x} ${from.y} L ${to.x} ${to.y}`;
  }

  if (type === 'main-continues') {
    // Dashed line along main branch skipping the feature section
    return `M ${from.x} ${from.y} L ${to.x} ${to.y}`;
  }

  if (type === 'branch-off') {
    // Curve from main to feature branch
    const midY = from.y + (to.y - from.y) * 0.5;
    return `M ${from.x} ${from.y} C ${from.x} ${midY}, ${to.x} ${midY}, ${to.x} ${to.y}`;
  }

  if (type === 'merge-in') {
    // Curve from feature back to main
    const midY = from.y + (to.y - from.y) * 0.5;
    return `M ${from.x} ${from.y} C ${from.x} ${midY}, ${to.x} ${midY}, ${to.x} ${to.y}`;
  }

  if (type === 'ci-fork') {
    // Curve from feature branch to CI column
    const midX = from.x + (to.x - from.x) * 0.5;
    return `M ${from.x} ${from.y} C ${midX} ${from.y}, ${midX} ${to.y}, ${to.x} ${to.y}`;
  }

  if (type === 'ci-return') {
    // Curve from CI column back to feature branch
    const midX = from.x + (to.x - from.x) * 0.5;
    return `M ${from.x} ${from.y} C ${midX} ${from.y}, ${midX} ${to.y}, ${to.x} ${to.y}`;
  }

  return `M ${from.x} ${from.y} L ${to.x} ${to.y}`;
});
</script>

<style>
.flow-connection {
  transition: opacity 0.5s ease, stroke-dashoffset 0.8s ease;
}

.flow-connection:not(.flow-connection--active) {
  opacity: 0;
}

.flow-connection--active {
  opacity: 1;
}
</style>

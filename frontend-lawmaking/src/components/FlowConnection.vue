<template>
  <path
    class="flow-connection"
    :class="{ 'flow-connection--active': isActive }"
    :d="pathData"
    fill="none"
    :stroke="strokeColor"
    :stroke-width="strokeWidth"
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

const strokeColor = computed(() => {
  const type = props.connection.type;
  if (type === 'ci-fork' || type === 'ci-chain' || type === 'ci-return') {
    return 'var(--color-ci)';
  }
  if (type === 'branch-off') return 'var(--color-branch-feature)';
  if (type === 'merge-in') return 'var(--color-branch-feature)';
  if (type === 'main-continues') return 'var(--color-branch-main)';

  // Determine from the "from" node
  const from = props.stages.find((s) => s.id === props.connection.from);
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

  if (type === 'branch-off' || type === 'merge-in' || type === 'ci-fork' || type === 'ci-return') {
    const dy = to.y - from.y;
    let midY;
    if (dy > 20) {
      // Normal downward: S-curve with control point halfway between
      midY = from.y + dy * 0.5;
    } else {
      // Same row or upward: arc below both points
      const bottomY = Math.max(from.y, to.y);
      midY = bottomY + 50;
    }
    return `M ${from.x} ${from.y} C ${from.x} ${midY}, ${to.x} ${midY}, ${to.x} ${to.y}`;
  }

  return `M ${from.x} ${from.y} L ${to.x} ${to.y}`;
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

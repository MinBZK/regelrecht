<template>
  <g
    class="flow-node"
    :class="{ 'flow-node--active': isActive, 'flow-node--selected': isSelected }"
    :transform="`translate(${x}, ${y})`"
    @click.stop="$emit('select', stage.id)"
    role="button"
    :tabindex="isActive ? 0 : -1"
    @keydown.enter.stop="$emit('select', stage.id)"
  >
    <!-- Node shape -->
    <circle
      v-if="shape === 'circle'"
      :r="radius"
      :fill="color"
      :stroke="color"
      stroke-width="2"
      class="flow-node__shape"
    />
    <circle
      v-else-if="shape === 'circle-double'"
      :r="radius + 3"
      fill="none"
      :stroke="color"
      stroke-width="2"
      class="flow-node__shape"
    />
    <circle
      v-if="shape === 'circle-double'"
      :r="radius - 1"
      :fill="color"
      :stroke="color"
      stroke-width="2"
      class="flow-node__shape"
    />
    <rect
      v-else-if="shape === 'diamond'"
      :x="-radius"
      :y="-radius"
      :width="radius * 2"
      :height="radius * 2"
      :fill="color"
      :stroke="color"
      stroke-width="2"
      :transform="`rotate(45)`"
      class="flow-node__shape"
    />
    <rect
      v-else-if="shape === 'square'"
      :x="-radius"
      :y="-radius"
      :width="radius * 2"
      :height="radius * 2"
      rx="3"
      :fill="color"
      :stroke="color"
      stroke-width="2"
      class="flow-node__shape"
    />
    <polygon
      v-else-if="shape === 'triangle'"
      :points="trianglePoints"
      :fill="color"
      :stroke="color"
      stroke-width="2"
      class="flow-node__shape"
    />
    <polygon
      v-else-if="shape === 'star'"
      :points="starPoints"
      :fill="color"
      :stroke="color"
      stroke-width="2"
      class="flow-node__shape"
    />

    <!-- Selection ring -->
    <circle
      v-if="isSelected"
      :r="radius + 6"
      fill="none"
      :stroke="color"
      stroke-width="2"
      opacity="0.4"
    />

    <!-- Labels to the right of the node -->
    <g :transform="labelTransform">
      <text
        class="flow-node__git-label"
        :y="-6"
        :fill="'var(--color-text-muted)'"
        font-size="11"
      >{{ stage.gitLabel }}</text>
      <text
        class="flow-node__law-label"
        :y="10"
        :fill="'var(--color-text-primary)'"
        font-size="14"
        font-weight="700"
      >{{ stage.lawLabel }}</text>
      <text
        v-if="stage.subtitle"
        class="flow-node__subtitle"
        :y="24"
        :fill="'var(--color-text-secondary)'"
        font-size="11"
      >{{ stage.subtitle }}</text>
    </g>
  </g>
</template>

<script setup>
import { computed } from 'vue';
import { typeColors, typeShapes } from '../data/flowData.js';

const props = defineProps({
  stage: { type: Object, required: true },
  isActive: { type: Boolean, default: false },
  isSelected: { type: Boolean, default: false },
  columnWidth: { type: Number, default: 220 },
  rowHeight: { type: Number, default: 80 },
});

defineEmits(['select']);

const radius = 10;

const x = computed(() => 80 + props.stage.col * props.columnWidth);
const y = computed(() => 50 + props.stage.row * props.rowHeight);

const color = computed(() => typeColors[props.stage.type] || 'var(--color-branch-main)');
const shape = computed(() => typeShapes[props.stage.type] || 'circle');

const labelTransform = computed(() => {
  // CI check labels go further right to avoid overlap with connection lines
  if (props.stage.col === 2) {
    return `translate(22, 0)`;
  }
  return `translate(20, 0)`;
});

const trianglePoints = computed(() => {
  const r = radius + 2;
  return `0,${-r} ${r},${r} ${-r},${r}`;
});

const starPoints = computed(() => {
  const outer = radius + 2;
  const inner = radius / 2;
  const points = [];
  for (let i = 0; i < 5; i++) {
    const outerAngle = (Math.PI / 2) + (i * 2 * Math.PI / 5);
    const innerAngle = outerAngle + Math.PI / 5;
    points.push(`${Math.cos(outerAngle) * outer},${-Math.sin(outerAngle) * outer}`);
    points.push(`${Math.cos(innerAngle) * inner},${-Math.sin(innerAngle) * inner}`);
  }
  return points.join(' ');
});
</script>

<style>
.flow-node {
  cursor: pointer;
  transition: opacity var(--transition-normal);
}

.flow-node:not(.flow-node--active) {
  opacity: 0;
  pointer-events: none;
}

.flow-node--active {
  opacity: 1;
  animation: nodeAppear 0.4s ease-out;
}

.flow-node__shape {
  transition: transform var(--transition-fast);
}

.flow-node:hover .flow-node__shape {
  transform: scale(1.2);
}

.flow-node__git-label {
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  letter-spacing: 0.02em;
}

.flow-node__law-label {
  font-family: var(--font-family);
}

.flow-node__subtitle {
  font-family: var(--font-family);
}

@keyframes nodeAppear {
  from {
    opacity: 0;
    transform: translate(var(--x), var(--y)) scale(0.5);
  }
  to {
    opacity: 1;
    transform: translate(var(--x), var(--y)) scale(1);
  }
}
</style>

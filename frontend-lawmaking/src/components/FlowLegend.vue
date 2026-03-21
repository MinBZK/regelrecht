<template>
  <div class="flow-legend">
    <div
      v-for="item in legend"
      :key="item.label"
      class="flow-legend__item"
    >
      <svg width="20" height="20" viewBox="0 0 20 20">
        <circle
          v-if="item.shape === 'circle'"
          cx="10" cy="10" r="6"
          :fill="item.color"
        />
        <rect
          v-else-if="item.shape === 'diamond'"
          x="4" y="4" width="12" height="12"
          :fill="item.color"
          transform="rotate(45 10 10)"
        />
        <rect
          v-else-if="item.shape === 'square'"
          x="4" y="4" width="12" height="12"
          rx="2"
          :fill="item.color"
        />
        <polygon
          v-else-if="item.shape === 'triangle'"
          points="10,3 17,17 3,17"
          :fill="item.color"
        />
        <polygon
          v-else-if="item.shape === 'star'"
          points="10,2 12.4,7.2 18,8 13.8,12 15,17.6 10,14.8 5,17.6 6.2,12 2,8 7.6,7.2"
          :fill="item.color"
        />
      </svg>
      <span class="flow-legend__label">{{ item.label }}</span>
    </div>
  </div>
</template>

<script setup>
import { legend } from '../data/flowConstants.js';
</script>

<style>
.flow-legend {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-4);
  padding: var(--spacing-2) var(--spacing-4);
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
}

.flow-legend__item {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
}

.flow-legend__label {
  white-space: nowrap;
}
</style>

<template>
  <transition name="slide">
    <div v-if="stage" class="stage-detail">
      <div class="stage-detail__header">
        <h2 class="stage-detail__title">{{ stage.lawLabel }}</h2>
        <button
          class="stage-detail__close"
          @click="$emit('close')"
          aria-label="Sluiten"
        >&#x2715;</button>
      </div>

      <div class="stage-detail__mapping">
        <div class="stage-detail__mapping-row">
          <span class="stage-detail__mapping-icon">&#x1F4BB;</span>
          <div>
            <div class="stage-detail__mapping-label">Git / CI/CD</div>
            <div class="stage-detail__mapping-value stage-detail__mapping-value--git">{{ stage.gitLabel }}</div>
          </div>
        </div>
        <div class="stage-detail__mapping-arrow">&#x21C6;</div>
        <div class="stage-detail__mapping-row">
          <span class="stage-detail__mapping-icon">&#x2696;</span>
          <div>
            <div class="stage-detail__mapping-label">Wetgeving</div>
            <div class="stage-detail__mapping-value">{{ stage.lawLabel }}</div>
          </div>
        </div>
      </div>

      <p class="stage-detail__description">{{ stage.description }}</p>

      <div class="stage-detail__meta">
        <span class="stage-detail__badge" :style="{ background: badgeColor }">
          {{ stage.type }}
        </span>
        <span v-if="stage.subtitle" class="stage-detail__subtitle">
          {{ stage.subtitle }}
        </span>
      </div>
    </div>
  </transition>
</template>

<script setup>
import { computed } from 'vue';
import { typeColors } from '../data/flowConstants.js';

const props = defineProps({
  stage: { type: Object, default: null },
});

defineEmits(['close']);

const badgeColor = computed(() => {
  if (!props.stage) return '';
  return typeColors[props.stage.type] || 'var(--color-branch-main)';
});
</script>

<style>
.stage-detail {
  position: fixed;
  top: var(--nav-height);
  right: 0;
  bottom: 0;
  width: 380px;
  max-width: 100vw;
  background: var(--color-surface);
  border-left: 1px solid var(--color-border);
  box-shadow: var(--shadow-md);
  padding: var(--spacing-6);
  overflow-y: auto;
  z-index: 10;
}

.stage-detail__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: var(--spacing-6);
}

.stage-detail__close {
  background: none;
  border: 1px solid var(--color-border);
  border-radius: var(--border-radius-sm);
  cursor: pointer;
  font-size: 16px;
  padding: 4px 8px;
  color: var(--color-text-secondary);
  line-height: 1;
  flex-shrink: 0;
}

.stage-detail__close:hover {
  background: var(--color-slate-100);
  color: var(--color-text-primary);
}

.stage-detail__title {
  font-size: var(--font-size-2xl);
  font-weight: 700;
  margin: 0;
  line-height: var(--line-height-snug);
}

.stage-detail__mapping {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-4);
  background: var(--color-slate-50);
  border-radius: var(--border-radius-lg);
  margin-bottom: var(--spacing-6);
}

.stage-detail__mapping-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  flex: 1;
}

.stage-detail__mapping-icon {
  font-size: 24px;
  line-height: 1;
}

.stage-detail__mapping-label {
  font-size: var(--font-size-xs);
  color: var(--color-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.stage-detail__mapping-value {
  font-size: var(--font-size-sm);
  font-weight: 700;
}

.stage-detail__mapping-value--git {
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-weight: 400;
}

.stage-detail__mapping-arrow {
  font-size: 20px;
  color: var(--color-text-muted);
  flex-shrink: 0;
}

.stage-detail__description {
  font-size: var(--font-size-base);
  line-height: var(--line-height-relaxed);
  color: var(--color-text-primary);
  margin: 0 0 var(--spacing-6);
}

.stage-detail__meta {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.stage-detail__badge {
  display: inline-block;
  padding: 2px 10px;
  border-radius: 12px;
  font-size: var(--font-size-xs);
  color: white;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.stage-detail__subtitle {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

/* Slide transition */
.slide-enter-active,
.slide-leave-active {
  transition: transform 0.3s ease, opacity 0.3s ease;
}

.slide-enter-from,
.slide-leave-to {
  transform: translateX(100%);
  opacity: 0;
}
</style>

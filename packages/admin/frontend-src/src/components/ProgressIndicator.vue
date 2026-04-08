<script setup>
import { computed } from 'vue';
import { PHASE_LABELS } from '../constants.js';

const props = defineProps({
  progress: { type: Object, default: null },
});

const totalSteps = computed(() => props.progress?.total_steps || 3);
const currentStep = computed(() => props.progress?.step || 1);
const phaseLabel = computed(() => {
  if (!props.progress?.phase) return 'Processing\u2026';
  return PHASE_LABELS[props.progress.phase] || props.progress.phase;
});

const details = computed(() => {
  if (!props.progress) return [];
  const items = [];
  if (props.progress.article_count) items.push(`${props.progress.article_count} articles`);
  if (props.progress.iteration) items.push(`iteration ${props.progress.iteration}`);
  return items;
});

const hasPhase = computed(() => props.progress && props.progress.phase);
</script>

<template>
  <div class="detail-section">
    <h3 class="detail-section__title">Progress</h3>
    <div class="detail-phase">
      <template v-if="hasPhase">
        <span class="detail-phase__steps">
          <span
            v-for="i in totalSteps"
            :key="i"
            class="detail-phase__dot"
            :class="{ 'detail-phase__dot--active': i <= currentStep }"
          />
        </span>
        <span class="detail-phase__label">
          Step {{ currentStep }} / {{ totalSteps }}: {{ phaseLabel }}
        </span>
      </template>
      <span v-else class="detail-phase__label">Processing&hellip;</span>
    </div>
    <div v-if="details.length > 0" class="detail-phase__meta">
      {{ details.join(' \u00B7 ') }}
    </div>
  </div>
</template>

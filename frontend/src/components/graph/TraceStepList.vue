<script setup>
import { computed, watch, nextTick } from 'vue';
import { formatValue } from '../../utils/outputFormat.js';

const props = defineProps({
  steps: { type: Array, required: true },
  currentStepIdx: { type: Number, required: true },
  filter: { type: String, default: 'highlights' }, // 'highlights' | 'all'
});

const emit = defineEmits(['select-step']);

const visibleSteps = computed(() => {
  if (props.filter === 'all') return props.steps;
  return props.steps.filter((s) => s.edgeIds.length > 0 || s.nodeIds.length > 0);
});

function truncate(v) {
  const s = formatValue(v);
  return s.length > 60 ? `${s.substring(0, 57)}…` : s;
}

// Keep the active step in view when navigation moves it out of the
// visible area. queueMicrotask in demo; Vue's nextTick is the idiomatic
// equivalent.
watch(
  () => props.currentStepIdx,
  (idx) => {
    if (idx < 0) return;
    nextTick(() => {
      const el = document.querySelector(`[data-step-idx="${idx}"]`);
      if (el) el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    });
  },
);
</script>

<template>
  <div class="step-list">
    <button
      v-for="step in visibleSteps"
      :key="steps.indexOf(step)"
      type="button"
      class="step-row"
      :class="{ 'step-row--active': steps.indexOf(step) === currentStepIdx }"
      :data-step-idx="steps.indexOf(step)"
      :style="{ paddingLeft: `${step.depth * 10 + 12}px` }"
      @click="emit('select-step', steps.indexOf(step))"
    >
      <div class="step-row__head">
        <span class="step-row__idx">{{ steps.indexOf(step) + 1 }}.</span>
        <span class="step-row__chip" :class="`node-type-${step.nodeType}`">
          {{ step.nodeType.replace(/_/g, ' ') }}
        </span>
        <span class="step-row__name">{{ step.name }}</span>
        <span v-if="step.resolveType" class="step-row__resolve">[{{ step.resolveType }}]</span>
      </div>
      <div v-if="step.result !== undefined && step.result !== null" class="step-row__result">
        = {{ truncate(step.result) }}
      </div>
      <div v-if="step.message" class="step-row__message">{{ step.message }}</div>
      <div v-if="step.edgeIds.length > 0 || step.nodeIds.length > 0" class="step-row__counts">
        <template v-if="step.edgeIds.length > 0">→ {{ step.edgeIds.length }} edge{{ step.edgeIds.length === 1 ? '' : 's' }}</template>
        <template v-if="step.nodeIds.length > 0">{{ step.edgeIds.length > 0 ? ', ' : '→ ' }}{{ step.nodeIds.length }} node{{ step.nodeIds.length === 1 ? '' : 's' }}</template>
      </div>
    </button>
    <div v-if="visibleSteps.length === 0" class="step-list__empty">
      Geen stappen met highlights. Zet het filter op "Alles" om alle {{ steps.length }} stappen te zien.
    </div>
  </div>
</template>

<style scoped>
.step-list {
  overflow-y: auto;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.step-row {
  display: block;
  width: 100%;
  border: 0;
  border-bottom: 1px solid #f3f4f6;
  background: transparent;
  padding: 4px 12px;
  text-align: left;
  font-size: 12px;
  cursor: pointer;
  font-family: inherit;
}
.step-row:hover { background: #f9fafb; }
.step-row--active {
  background: #fef3c7;
  font-weight: 600;
  color: #78350f;
}

.step-row__head {
  display: flex;
  align-items: baseline;
  gap: 4px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.step-row__idx {
  display: inline-block;
  width: 28px;
  text-align: right;
  color: #9ca3af;
}
.step-row__chip {
  font-size: 10px;
  padding: 1px 4px;
  border-radius: 3px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.step-row__name { overflow: hidden; text-overflow: ellipsis; }
.step-row__resolve {
  font-size: 10px;
  color: #4f46e5;
}
.step-row__result {
  padding-left: 32px;
  font-size: 11px;
  color: #047857;
  overflow: hidden;
  text-overflow: ellipsis;
}
.step-row__message {
  padding-left: 32px;
  font-size: 10px;
  font-style: italic;
  color: #6b7280;
  overflow: hidden;
  text-overflow: ellipsis;
}
.step-row__counts {
  padding-left: 32px;
  font-size: 10px;
  color: #d97706;
}

.step-list__empty {
  padding: 16px;
  font-size: 12px;
  color: var(--semantics-text-color-secondary, #6b7280);
}
</style>

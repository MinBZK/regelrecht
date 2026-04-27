<script setup>
import { computed, ref, watch, nextTick } from 'vue';
import { formatValue } from '../../utils/outputFormat.js';

const props = defineProps({
  steps: { type: Array, required: true },
  currentStepIdx: { type: Number, required: true },
  filter: { type: String, default: 'highlights' }, // 'highlights' | 'all'
});

const emit = defineEmits(['select-step']);

const listEl = ref(null);

// Tag each visible step with its index in the unfiltered `steps` array
// so the template can read it directly — avoids an O(n) `steps.indexOf`
// per row, per render (the loop itself was O(visibleSteps × steps)).
const visibleSteps = computed(() => {
  const all = props.steps;
  const entries = all.map((step, idx) => ({ step, idx }));
  if (props.filter === 'all') return entries;
  return entries.filter(({ step }) => step.edgeIds.length > 0 || step.nodeIds.length > 0);
});

function truncate(v) {
  const s = formatValue(v);
  return s.length > 60 ? `${s.substring(0, 57)}…` : s;
}

// Keep the active step in view when navigation moves it out of the
// visible area. queueMicrotask in demo; Vue's nextTick is the idiomatic
// equivalent. Scope the lookup to this component's root so a second
// graph pane (e.g. during a route transition) can't be matched.
watch(
  () => props.currentStepIdx,
  (idx) => {
    if (idx < 0) return;
    nextTick(() => {
      const root = listEl.value;
      if (!root) return;
      const el = root.querySelector(`[data-step-idx="${idx}"]`);
      if (el) el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    });
  },
);
</script>

<template>
  <div ref="listEl" class="step-list">
    <button
      v-for="entry in visibleSteps"
      :key="entry.idx"
      type="button"
      class="step-row"
      :class="{ 'step-row--active': entry.idx === currentStepIdx }"
      :data-step-idx="entry.idx"
      :style="{ paddingLeft: `${entry.step.depth * 10 + 12}px` }"
      @click="emit('select-step', entry.idx)"
    >
      <div class="step-row__head">
        <span class="step-row__idx">{{ entry.idx + 1 }}.</span>
        <span class="step-row__chip" :class="`node-type-${entry.step.nodeType}`">
          {{ entry.step.nodeType.replace(/_/g, ' ') }}
        </span>
        <span class="step-row__name">{{ entry.step.name }}</span>
        <span v-if="entry.step.resolveType" class="step-row__resolve">[{{ entry.step.resolveType }}]</span>
      </div>
      <div v-if="entry.step.result !== undefined && entry.step.result !== null" class="step-row__result">
        = {{ truncate(entry.step.result) }}
      </div>
      <div v-if="entry.step.message" class="step-row__message">{{ entry.step.message }}</div>
      <div v-if="entry.step.edgeIds.length > 0 || entry.step.nodeIds.length > 0" class="step-row__counts">
        <template v-if="entry.step.edgeIds.length > 0">→ {{ entry.step.edgeIds.length }} edge{{ entry.step.edgeIds.length === 1 ? '' : 's' }}</template>
        <template v-if="entry.step.nodeIds.length > 0">{{ entry.step.edgeIds.length > 0 ? ', ' : '→ ' }}{{ entry.step.nodeIds.length }} node{{ entry.step.nodeIds.length === 1 ? '' : 's' }}</template>
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

<script setup>
import { computed } from 'vue';
import { formatValue, formatOutputValue, normalizeForCompare, matchStatus as _matchStatus } from '../../utils/outputFormat.js';

const props = defineProps({
  step: { type: Object, default: null },
  outputs: { type: Object, default: () => ({}) },
  expectations: { type: Object, default: () => ({}) },
});

function matchStatus(name, value) {
  return _matchStatus(name, value, props.expectations);
}

function truncate(v) {
  const s = formatValue(v);
  return s.length > 80 ? `${s.substring(0, 77)}…` : s;
}

const outputEntries = computed(() => Object.entries(props.outputs || {}));
const expectationEntries = computed(() => Object.entries(props.expectations || {}));
</script>

<template>
  <div class="step-detail">
    <div v-if="step" class="step-detail__card">
      <div class="step-detail__head">
        <span class="step-detail__chip" :class="`node-type-${step.nodeType}`">
          {{ step.nodeType.replace(/_/g, ' ') }}
        </span>
        <span class="step-detail__name">{{ step.name }}</span>
      </div>
      <dl class="step-detail__dl">
        <div class="step-detail__row">
          <dt>Wet:</dt>
          <dd class="mono">{{ step.lawId }}</dd>
        </div>
        <div v-if="step.resolveType" class="step-detail__row">
          <dt>Resolve:</dt>
          <dd class="mono indigo">{{ step.resolveType }}</dd>
        </div>
        <div v-if="step.result !== undefined && step.result !== null" class="step-detail__row">
          <dt>Resultaat:</dt>
          <dd class="mono emerald">{{ truncate(step.result) }}</dd>
        </div>
        <div v-if="step.durationUs !== undefined" class="step-detail__row">
          <dt>Duur:</dt>
          <dd class="mono">{{ step.durationUs }}µs</dd>
        </div>
        <div v-if="step.message" class="step-detail__row">
          <dt>Bericht:</dt>
          <dd class="italic">{{ step.message }}</dd>
        </div>
      </dl>
    </div>

    <h3 class="step-detail__section-title">Outputs</h3>
    <dl class="step-detail__outputs">
      <div v-for="[k, v] in outputEntries" :key="k" class="step-detail__row">
        <dt>{{ k }}:</dt>
        <dd class="mono">{{ formatOutputValue(v, k) }}</dd>
      </div>
      <div v-if="outputEntries.length === 0" class="step-detail__empty">Geen outputs</div>
    </dl>

    <template v-if="expectationEntries.length > 0">
      <h3 class="step-detail__section-title">Verwachtingen</h3>
      <ul class="step-detail__expectations">
        <li v-for="[name, expected] in expectationEntries" :key="name">
          <span
            class="step-detail__marker"
            :class="{
              'pass': matchStatus(name, outputs[name]) === 'passed',
              'fail': matchStatus(name, outputs[name]) === 'failed',
            }"
          >{{ matchStatus(name, outputs[name]) === 'failed' ? '✗' : '✓' }}</span>
          <span class="mono">
            {{ name }} = {{ formatValue(normalizeForCompare(expected)) }}
            <span v-if="matchStatus(name, outputs[name]) === 'failed'" class="fail">
              (kreeg {{ formatValue(outputs[name]) }})
            </span>
          </span>
        </li>
      </ul>
    </template>
  </div>
</template>

<style scoped>
.step-detail {
  overflow-y: auto;
  padding: 12px;
  background: var(--semantics-surfaces-tinted-background-color, #f9fafb);
  font-size: 12px;
}

.step-detail__card {
  margin-bottom: 12px;
  border: 1px solid #fcd34d;
  background: #fef3c7;
  border-radius: 6px;
  padding: 8px;
}

.step-detail__head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.step-detail__chip {
  font-size: 10px;
  padding: 1px 4px;
  border-radius: 3px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.step-detail__name {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 12px;
  font-weight: 600;
}

.step-detail__dl,
.step-detail__outputs {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 11px;
  margin: 0;
}

.step-detail__row {
  display: flex;
  gap: 8px;
}
.step-detail__row dt {
  width: 80px;
  color: #6b7280;
  flex-shrink: 0;
}
.step-detail__row dd {
  margin: 0;
  color: #111827;
  min-width: 0;
  word-break: break-word;
}

.mono { font-family: 'SF Mono', 'Fira Code', monospace; }
.indigo { color: #4f46e5; }
.emerald { color: #047857; }
.italic { font-style: italic; color: #6b7280; }

.step-detail__section-title {
  margin: 8px 0 4px;
  font-size: 12px;
  font-weight: 600;
  color: #374151;
}

.step-detail__empty {
  color: #9ca3af;
  font-style: italic;
  font-size: 11px;
}

.step-detail__expectations {
  list-style: none;
  padding: 0;
  margin: 0;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 11px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.step-detail__expectations li {
  display: flex;
  gap: 6px;
}
.step-detail__marker {
  font-weight: 700;
}
.step-detail__marker.pass,
.pass { color: #047857; }
.step-detail__marker.fail,
.fail { color: #dc2626; }
</style>

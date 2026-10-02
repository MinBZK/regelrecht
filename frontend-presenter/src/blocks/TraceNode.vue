<script setup>
import { computed } from 'vue';
import { formatValue, humanize } from '@editor-utils/outputFormat.js';

// One node of the engine's calculation trace (packages/engine/src/trace.rs,
// PathNode), with its children folded underneath. The top levels start open;
// deeper ones are there for whoever asks "and where did that come from?".

defineOptions({ name: 'TraceNode' });
const props = defineProps({
  node: { type: Object, required: true },
  depth: { type: Number, default: 0 },
  openDepth: { type: Number, default: 2 },
});

const TYPE_LABELS = {
  Article: 'artikel',
  Action: 'regel',
  Operation: 'operatie',
  Resolve: 'waarde',
  Requirement: 'voorwaarde',
  CrossLawReference: 'andere wet',
  Cached: 'eerder berekend',
  OpenTermResolution: 'open norm',
  HookResolution: 'haak',
  OverrideResolution: 'voorrang',
};

const children = computed(() => props.node.children ?? []);
const label = computed(() => TYPE_LABELS[props.node.node_type] ?? String(props.node.node_type ?? '').toLowerCase());
const name = computed(() => humanize(String(props.node.name ?? '').replace(/^\$/, '')));
const hasResult = computed(() => props.node.result !== undefined);
</script>

<template>
  <details v-if="children.length" class="trace-node" :open="depth < openDepth">
    <summary>
      <span class="trace-type">{{ label }}</span>
      <span class="trace-name">{{ name }}</span>
      <span v-if="hasResult" class="trace-result">= {{ formatValue(node.result) }}</span>
    </summary>
    <TraceNode v-for="(c, i) in children" :key="i" :node="c" :depth="depth + 1" :open-depth="openDepth" />
  </details>
  <div v-else class="trace-node trace-leaf">
    <span class="trace-type">{{ label }}</span>
    <span class="trace-name">{{ name }}</span>
    <span v-if="hasResult" class="trace-result">= {{ formatValue(node.result) }}</span>
  </div>
</template>

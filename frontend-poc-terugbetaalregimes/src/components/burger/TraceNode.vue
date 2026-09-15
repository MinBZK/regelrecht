<template>
  <li class="trace-node">
    <div class="tn-line" :class="`tn-${node.node_type}`">
      <nldd-icon-button
        v-if="hasChildren"
        class="tn-toggle"
        size="xs"
        variant="neutral-transparent"
        :icon="open ? 'caret-down-small' : 'caret-right-small'"
        :expanded="open ? true : undefined"
        :accessible-label="open ? 'Inklappen' : 'Uitklappen'"
        @click="open = !open"
      ></nldd-icon-button>
      <span v-else class="tn-toggle tn-leaf" aria-hidden="true"></span>

      <span class="tn-type">{{ typeLabel }}</span>
      <span class="tn-name">{{ node.name }}</span>
      <span v-if="node.result !== null && node.result !== undefined" class="tn-result">
        = {{ formatResult(node.result) }}
      </span>
    </div>
    <ul v-if="hasChildren && open" class="tn-children">
      <trace-node v-for="(child, i) in node.children" :key="i" :node="child" :depth="depth + 1" />
    </ul>
  </li>
</template>

<script setup>
import { ref, computed } from 'vue';

const props = defineProps({
  node: { type: Object, required: true },
  depth: { type: Number, default: 0 },
});

const hasChildren = computed(() => Array.isArray(props.node.children) && props.node.children.length > 0);
// Toon de bovenste drie lagen open; dieper dichtgeklapt om de boom leesbaar te houden.
const open = ref(props.depth < 2);

const TYPE_LABELS = {
  article: 'artikel',
  action: 'berekening',
  operation: 'bewerking',
  resolve: 'waarde',
  open_term_resolution: 'open norm',
  cross_law_reference: 'verwijzing',
};
const typeLabel = computed(() => TYPE_LABELS[props.node.node_type] ?? props.node.node_type);

function formatResult(value) {
  if (typeof value === 'boolean') return value ? 'waar' : 'onwaar';
  if (typeof value === 'number') {
    if (!Number.isInteger(value)) return value.toLocaleString('nl-NL', { maximumFractionDigits: 2 });
    return value.toLocaleString('nl-NL');
  }
  return String(value);
}
</script>

<style scoped>
.trace-node { list-style: none; }
.tn-line {
  display: flex;
  align-items: baseline;
  gap: var(--primitives-space-4, 4px);
  padding: 2px 0;
  font-size: 0.85em;
  line-height: 1.5;
}
.tn-toggle {
  flex: none;
  align-self: center;
}
/* Uitlijning van bladeren (zonder knop) met de icon-button ernaast. */
.tn-leaf {
  display: inline-block;
  width: 24px;
  height: 24px;
}
.tn-type {
  flex: none;
  font-size: 0.75em;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--semantics-content-secondary-color);
  min-width: 5.5em;
}
.tn-name {
  font-family: var(--primitives-font-family-monospace, monospace);
  color: var(--semantics-content-color);
}
.tn-article > .tn-name { font-weight: 700; }
.tn-result {
  font-family: var(--primitives-font-family-monospace, monospace);
  color: var(--semantics-content-accent-color);
  font-weight: 600;
}
.tn-children {
  margin: 0;
  padding-left: 18px;
  border-left: 1px dashed var(--semantics-dividers-color);
  margin-left: 8px;
}
</style>

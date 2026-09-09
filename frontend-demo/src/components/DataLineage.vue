<script setup>
import { computed, reactive } from 'vue';
import OrgLogo from './OrgLogo.vue';
import { fieldSpec, formatValue, humanize } from '../data/format.js';
import { useDemo } from '../store/demoStore.js';

// The rows of the "Gebruikte gegevens" tree of one tile: every register value
// the law used, and under each law that supplied a computed value the values
// that law used in turn. Renders `nldd-list-item` rows only; the tile owns the
// `nldd-list type="tree"` around them. A value row is a button: the citizen
// can correct it, which becomes a claim. A pending correction shows old → new.

const props = defineProps({
  nodes: { type: Array, required: true },
  depth: { type: Number, default: 0 },
  /** True when these rows are the children of a branch row (slot="children"). */
  nested: { type: Boolean, default: false },
});
const emit = defineEmits(['edit']);
const { corpus, claimFor } = useDemo();

function specFor(node) {
  return fieldSpec(corpus.value?.lawById(node.law)?.doc, node.name);
}
function lawName(id) {
  return corpus.value?.lawById(id)?.name ?? id;
}
function lawService(id) {
  return corpus.value?.lawById(id)?.service ?? null;
}
const values = computed(() => props.nodes.filter((n) => n.kind === 'value'));
const laws = computed(() => props.nodes.filter((n) => n.kind === 'law'));

function pending(node) {
  const c = claimFor(node.law, node.name);
  return c && c.status === 'PENDING' ? c : null;
}

const open = reactive({});
function keyOf(node) {
  return `${node.law}#${node.name}`;
}
const slotName = computed(() => (props.nested ? 'children' : undefined));
</script>

<template>
  <nldd-list-item v-for="node in values" :key="`${node.law}|${node.name}`" :slot="slotName" size="sm" button @click="emit('edit', node)">
    <nldd-spacer-cell v-for="i in depth" :key="i" size="20"></nldd-spacer-cell>
    <nldd-cell v-if="node.service"><OrgLogo :service="node.service" size="sm" /></nldd-cell>
    <nldd-icon-cell v-else-if="node.corrected" icon="edit" size="16" color="accent"></nldd-icon-cell>
    <nldd-icon-cell v-else icon="question-mark-circle" size="16" color="secondary"></nldd-icon-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-text-cell size="sm" min-width="120px" :text="humanize(node.name)" :supporting-text="node.corrected ? 'Gecorrigeerd door u' : node.service ? undefined : 'Nog niet bekend'"></nldd-text-cell>
    <nldd-text-cell size="sm" width="fit-content" max-width="55%" horizontal-alignment="right" :color="pending(node) ? 'warning' : node.value === null ? 'secondary' : 'default'">
      <template v-if="pending(node)">
        <s>{{ formatValue(node.value, specFor(node)) }}</s> → {{ formatValue(pending(node).newValue, specFor(node)) }}
      </template>
      <template v-else>{{ formatValue(node.value, specFor(node)) }}</template>
    </nldd-text-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-icon-cell icon="edit" size="16" color="secondary"></nldd-icon-cell>
  </nldd-list-item>
  <nldd-list-item
    v-for="node in laws"
    :key="keyOf(node)"
    :slot="slotName"
    size="sm"
    button
    :expanded="!!open[keyOf(node)]"
    @click="open[keyOf(node)] = !open[keyOf(node)]"
  >
    <nldd-spacer-cell v-for="i in depth" :key="i" size="20"></nldd-spacer-cell>
    <nldd-cell v-if="lawService(node.law)"><OrgLogo :service="lawService(node.law)" size="sm" /></nldd-cell>
    <nldd-spacer-cell v-if="lawService(node.law)" size="8"></nldd-spacer-cell>
    <nldd-text-cell size="sm" :text="humanize(node.name)" :supporting-text="`berekend door ${lawName(node.law)}`"></nldd-text-cell>
    <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(node.value, specFor(node))"></nldd-text-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
    <DataLineage v-if="node.children?.length" :nodes="node.children" :depth="depth + 1" nested @edit="emit('edit', $event)" />
  </nldd-list-item>
</template>

<script setup>
import { computed } from 'vue';
import OrgLogo from './OrgLogo.vue';
import { fieldSpec, formatValue, humanize } from '../data/format.js';
import { useDemo } from '../store/demoStore.js';

// The "Gebruikte gegevens" tree of one tile: every register value the law
// used, grouped under the law that supplied it. A value row is a button: the
// citizen can correct it, which becomes a claim. A pending correction shows
// old → new.

const props = defineProps({
  nodes: { type: Array, required: true },
  depth: { type: Number, default: 0 },
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
</script>

<template>
  <nldd-list v-if="values.length" :variant="depth === 0 ? 'box' : 'simple'" accessible-label="Gebruikte gegevens">
    <nldd-list-item v-for="node in values" :key="`${node.law}|${node.name}`" size="sm" button @click="emit('edit', node)">
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
  </nldd-list>
  <template v-for="node in laws" :key="`${node.law}#${node.name}`">
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-container layout="row" gap="8" vertical-alignment="center" padding-block="4">
      <OrgLogo v-if="lawService(node.law)" :service="lawService(node.law)" size="sm" />
      <nldd-text-cell size="sm" :text="`${humanize(node.name)}: **${formatValue(node.value, specFor(node))}**`" :supporting-text="`berekend door ${lawName(node.law)}`"></nldd-text-cell>
    </nldd-container>
    <ul v-if="node.children?.length" class="data-tree">
      <li>
        <DataLineage :nodes="node.children" :depth="depth + 1" @edit="emit('edit', $event)" />
      </li>
    </ul>
  </template>
</template>

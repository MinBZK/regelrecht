<script setup>
import { computed, reactive } from 'vue';
import OrgLogo from './OrgLogo.vue';
import { fieldSpec, formatMissing, formatValue, humanize, isUnknown } from '../data/format.js';
import { useDemo } from '../store/demoStore.js';

// The rows of the "Gebruikte gegevens" tree of one tile: every register value
// the law used, and under each law that supplied a computed value the values
// that law used in turn. Renders `nldd-list-item` rows only; the tile owns the
// `nldd-list type="tree"` around them. A value row is a button: the citizen
// can correct it, which becomes a claim. A pending correction shows old → new
// and says that the outcome above already counts it: the citizen's view
// computes with pending claims (demoStore.claimsForEngine), as the POC does,
// so someone who corrects their income immediately sees what it would mean.
// Nothing is granted by it — the case still goes to a caseworker.

const props = defineProps({
  nodes: { type: Array, required: true },
  depth: { type: Number, default: 0 },
  /** True when these rows are the children of a branch row (slot="children"). */
  nested: { type: Boolean, default: false },
});
const emit = defineEmits(['edit']);
// `canSubmitClaims` is false wanneer iemand namens een ander handelt met
// alleen leesrecht: dan is een waarde te zien maar niet te corrigeren, en
// hoort de rij ook niet als knop te reageren.
const { corpus, claimFor, canSubmitClaims } = useDemo();

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

/**
 * The second line under a value: who corrected it, or, for an unknown value,
 * what is missing (RFC-036). A value that is only missing itself is simply
 * "Nog niet bekend"; an unknown that misses other facts names them.
 */
function supportingText(node) {
  if (node.corrected) return 'Gecorrigeerd door u';
  if (isUnknown(node.value)) {
    const missing = formatMissing(node.value, { ownLaw: node.law, lawName });
    return missing === `ontbreekt: ${humanize(node.name).toLowerCase()}` ? 'Nog niet bekend' : missing;
  }
  return node.service ? undefined : 'Nog niet bekend';
}

const open = reactive({});
function keyOf(node) {
  return `${node.law}#${node.name}`;
}
const slotName = computed(() => (props.nested ? 'children' : undefined));
</script>

<template>
  <nldd-list-item v-for="node in values" :key="`${node.law}|${node.name}`" :slot="slotName" size="sm" :button="canSubmitClaims || undefined" @click="canSubmitClaims && emit('edit', node)">
    <nldd-spacer-cell v-for="i in depth" :key="i" size="20"></nldd-spacer-cell>
    <nldd-cell v-if="node.service"><OrgLogo :service="node.service" size="sm" /></nldd-cell>
    <nldd-icon-cell v-else-if="node.corrected" icon="edit" size="16" color="accent"></nldd-icon-cell>
    <nldd-icon-cell v-else icon="question-mark-circle" size="16" color="secondary"></nldd-icon-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-text-cell size="sm" min-width="120px" :text="humanize(node.name)" :supporting-text="pending(node) ? `${supportingText(node) ? supportingText(node) + ' · ' : ''}meegerekend, nog te beoordelen` : supportingText(node)"></nldd-text-cell>
    <nldd-text-cell size="sm" width="fit-content" max-width="55%" horizontal-alignment="right" :color="pending(node) ? 'warning' : isUnknown(node.value) ? 'secondary' : 'default'">
      <template v-if="pending(node)">
        <s>{{ formatValue(node.value, specFor(node)) }}</s> → {{ formatValue(pending(node).newValue, specFor(node)) }}
      </template>
      <template v-else>{{ formatValue(node.value, specFor(node)) }}</template>
    </nldd-text-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-icon-cell v-if="canSubmitClaims" icon="edit" size="16" color="secondary"></nldd-icon-cell>
  </nldd-list-item>
  <!-- A law row expands only when it has children to show. A law whose inputs
       the trace does not carry (a register that answers straight from its own
       data) is a leaf: no disclosure chevron, because there is nothing under it
       to open. Every row stays correctable through its own pencil, an outcome
       of a law included: what the engine computed is a claim like any other,
       and citizen and caseworker may both dispute it. -->
  <nldd-list-item
    v-for="node in laws"
    :key="keyOf(node)"
    :slot="slotName"
    size="sm"
    :button="node.children?.length ? true : undefined"
    :expanded="node.children?.length && open[keyOf(node)] ? true : undefined"
    @click="node.children?.length && (open[keyOf(node)] = !open[keyOf(node)])"
  >
    <nldd-spacer-cell v-for="i in depth" :key="i" size="20"></nldd-spacer-cell>
    <nldd-cell v-if="lawService(node.law)"><OrgLogo :service="lawService(node.law)" size="sm" /></nldd-cell>
    <nldd-spacer-cell v-if="lawService(node.law)" size="8"></nldd-spacer-cell>
    <nldd-text-cell size="sm" :text="humanize(node.name)" :supporting-text="isUnknown(node.value) ? `berekend door ${lawName(node.law)} · ${formatMissing(node.value, { ownLaw: node.law, lawName })}` : `berekend door ${lawName(node.law)}`"></nldd-text-cell>
    <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="isUnknown(node.value) ? 'secondary' : 'default'" :text="formatValue(node.value, specFor(node))"></nldd-text-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-icon-cell v-if="canSubmitClaims" icon="edit" size="16" color="secondary" role="button" tabindex="0" accessible-label="Corrigeren" @click.stop="emit('edit', node)" @keydown.enter.stop="emit('edit', node)"></nldd-icon-cell>
    <nldd-spacer-cell v-if="node.children?.length" size="8"></nldd-spacer-cell>
    <nldd-icon-cell v-if="node.children?.length" disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
    <DataLineage v-if="node.children?.length" :nodes="node.children" :depth="depth + 1" nested @edit="emit('edit', $event)" />
  </nldd-list-item>
</template>

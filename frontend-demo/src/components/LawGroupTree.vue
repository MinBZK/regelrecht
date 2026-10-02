<script setup>
import { reactive } from 'vue';
import OrgLogo from './OrgLogo.vue';
import { useI18n } from '../i18n/index.js';

const { t } = useI18n();

// Laws grouped by the organisation that executes them, as one design-system
// tree: an organisation is a branch row (mark, name, count) that folds, its
// laws are the child rows. Two modes: `pick` (one law is open, the row is a
// button) and `check` (any number of laws are on, the row toggles a checkbox).
// Shared by the Wetten and Graaf side panels so they read the same.

const props = defineProps({
  groups: { type: Array, required: true }, // [{ service, info: {name}, laws }]
  mode: { type: String, default: 'pick' }, // 'pick' | 'check'
  activeId: { type: String, default: null },
  checked: { type: Object, default: () => new Set() },
  /** Optional supporting text per law. */
  supportingText: { type: Function, default: null },
});
const emit = defineEmits(['pick', 'toggle']);

const folded = reactive({});
function isOpen(service) {
  return !folded[service];
}
function checkedCount(group) {
  return group.laws.filter((l) => props.checked.has(l.id)).length;
}
function onRow(law) {
  if (props.mode === 'check') emit('toggle', law.id);
  else emit('pick', law.id);
}
</script>

<template>
  <nldd-list type="tree" :accessible-label="t('wet.tree.label')">
    <nldd-list-item v-for="group in groups" :key="group.service" size="md" button :expanded="isOpen(group.service)" @click="folded[group.service] = !folded[group.service]">
      <nldd-cell><OrgLogo :service="group.service" size="sm" /></nldd-cell>
      <nldd-spacer-cell size="12"></nldd-spacer-cell>
      <nldd-text-cell :text="group.info.name" :supporting-text="t.plural(group.laws.length, 'wet.sidebar.count')"></nldd-text-cell>
      <nldd-cell v-if="mode === 'check' && checkedCount(group)"><nldd-badge color="accent" :number="checkedCount(group)" :accessible-label="t('wet.tree.chosen', { n: checkedCount(group) })"></nldd-badge></nldd-cell>
      <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
      <nldd-list-item
        v-for="law in group.laws"
        :key="law.id"
        slot="children"
        size="sm"
        button
        :selected="mode === 'pick' && law.id === activeId ? true : undefined"
        :accessible-label="mode === 'check' ? t('wet.tree.law_state', { name: law.name, state: t(checked.has(law.id) ? 'wet.tree.law_on' : 'wet.tree.law_off') }) : undefined"
        @click.stop="onRow(law)"
      >
        <nldd-spacer-cell size="20"></nldd-spacer-cell>
        <nldd-cell v-if="mode === 'check'"><nldd-checkbox :checked="checked.has(law.id) || undefined" aria-hidden="true" tabindex="-1"></nldd-checkbox></nldd-cell>
        <nldd-spacer-cell v-if="mode === 'check'" size="12"></nldd-spacer-cell>
        <nldd-text-cell size="sm" :text="law.name" :supporting-text="supportingText ? supportingText(law) : undefined"></nldd-text-cell>
        <nldd-icon-cell v-if="mode === 'pick'" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
      </nldd-list-item>
    </nldd-list-item>
    <nldd-inline-dialog slot="empty" :text="t('wet.tree.empty.title')" :supporting-text="t('wet.tree.empty.body')"></nldd-inline-dialog>
  </nldd-list>
</template>

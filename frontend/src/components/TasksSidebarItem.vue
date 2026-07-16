<script setup>
/**
 * TasksSidebarItem - het "Taken"-item in de primary sidebar van Home, met
 * badge (open-takenaantal). Eigen component zodat `useTasks()` (start de
 * gedeelde 30s-poll bij de eerste aanroep) pas gebeurt wanneer dit item
 * daadwerkelijk gemount wordt - LibraryView rendert het alleen binnen een
 * actief traject (een ingelogde context), dus anonieme bezoekers pollen
 * /api/tasks nooit. Zelfde consumers-teller-patroon als TasksPane: samen
 * gemount blijft het één poll-interval.
 */
import { computed } from 'vue';
import { useTasks } from '../composables/useTasks.js';

defineProps({
  selected: { type: Boolean, default: false },
});

const { openCount, running } = useTasks();

// Stil "bezig"-signaal: geen open taken (actie nodig), maar wel een lopende
// verrijking/conversie - een neutrale dot-badge (geen tekst/getal/icoon ->
// nldd-badge rendert dan een stip) i.p.v. de rode aantal-badge.
const showRunningDot = computed(() => openCount.value === 0 && running.value.length > 0);
</script>

<template>
  <nldd-list-item size="md" button :selected="selected || undefined">
    <nldd-icon-cell size="20"><nldd-icon name="tasks"></nldd-icon></nldd-icon-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-text-cell text="Taken"></nldd-text-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-cell v-if="openCount > 0">
      <nldd-badge size="sm" :number="openCount" accessible-label="Open taken"></nldd-badge>
    </nldd-cell>
    <nldd-cell v-else-if="showRunningDot">
      <nldd-badge size="sm" color="neutral" accessible-label="Er loopt een taak"></nldd-badge>
    </nldd-cell>
    <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
  </nldd-list-item>
</template>

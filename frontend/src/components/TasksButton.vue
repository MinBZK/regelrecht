<script setup>
/**
 * TasksButton - topbar-knop met badge (open-takenaantal) die de taken-sheet
 * opent. Kleine eigen component zodat `useTasks()` (start de 30s-poll bij de
 * eerste aanroep) pas gebeurt wanneer dit component daadwerkelijk gemount
 * wordt - AppShell mount het alleen achter `v-if` op authenticated EN de
 * `tasks.job_review` flag (zie AppShell.vue). Mounten in twee responsieve
 * headers (md/lg) tegelijk is prima: useTasks() is een module-singleton met
 * een consumers-teller, dus de poll blijft één interval, en onUnmounted telt
 * netjes af.
 */
import { computed } from 'vue';
import { useTasks } from '../composables/useTasks.js';
import { useTasksSheet } from '../composables/useTasksSheet.js';

const props = defineProps({
  // Suffix zodat de knop-id uniek blijft wanneer dit component in meerdere
  // responsieve headers tegelijk gemount is (zelfde patroon als TrajectMenu).
  idSuffix: { type: String, default: '' },
});

const { openCount, running } = useTasks();
const { open } = useTasksSheet();

const btnId = computed(() => `tasks-btn-${props.idSuffix}`);
// Stil "bezig"-signaal: geen open taken (actie nodig), maar wel een lopende
// verrijking - een neutrale dot-badge (geen tekst/getal/icoon -> nldd-badge
// rendert dan een stip) i.p.v. de rode aantal-badge hierboven.
const showRunningDot = computed(() => openCount.value === 0 && running.value.length > 0);
</script>

<template>
  <span class="tasks-button">
    <nldd-icon-button
      :id="btnId"
      size="md"
      icon="tasks"
      text="Taken"
      tooltip-timing="never"
      @click="open"
    ></nldd-icon-button>
    <nldd-badge
      v-if="openCount > 0"
      class="tasks-button__badge"
      size="sm"
      :number="openCount"
      accessible-label="Open taken"
    ></nldd-badge>
    <nldd-badge
      v-else-if="showRunningDot"
      class="tasks-button__badge"
      size="sm"
      color="neutral"
      accessible-label="Verrijking loopt"
    ></nldd-badge>
  </span>
</template>

<style scoped>
/* nldd-badge heeft geen ingebouwde hoek-positionering t.o.v. een ander
   element (het is een los, inline-flex element) - deze wrapper legt 'm in de
   rechterbovenhoek van de icon-button, het gangbare notificatie-badge-patroon.
   Custom CSS bovenop het design system, gerapporteerd in de PR. */
.tasks-button {
  position: relative;
  display: inline-flex;
}
.tasks-button__badge {
  position: absolute;
  top: -4px;
  right: -4px;
  pointer-events: none;
}
</style>

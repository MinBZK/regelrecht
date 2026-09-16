<template>
  <div class="dk">
    <div class="dk-row">
      <nldd-button
        :text="anyRunning ? 'Bezig met rekenen…' : 'Doorrekenen'"
        start-icon="analytics"
        variant="primary"
        size="sm"
        :disabled="anyRunning ? true : undefined"
        @click="run"
      ></nldd-button>
      <nldd-tag v-if="stale && !anyRunning" color="warning" icon="warning" :text="populatieGewijzigd ? 'Nieuwe populatie, uitkomsten verouderd' : 'Uitkomsten verouderd'"></nldd-tag>
      <span v-else-if="!anyRunning" class="dk-status">dezelfde {{ number(n) }} debiteuren in {{ columns.length === 1 ? 'de kolom' : `alle ${columns.length} kolommen` }} · {{ autoRecompute ? 'rekent automatisch bij een wijziging' : 'automatisch herberekenen staat uit' }}</span>
    </div>
    <template v-for="col in columns" :key="col.key">
      <nldd-progress-bar
        v-if="running[col.key]"
        :text="`${kolomTitel(col)} doorrekenen`"
        :value="Math.round((progress[col.key] ?? 0) * 100)"
        :max="100"
      ></nldd-progress-bar>
      <nldd-banner v-if="errors[col.key]" variant="critical">
        {{ kolomTitel(col) }}: doorrekenen mislukt: {{ errors[col.key] }}
      </nldd-banner>
    </template>
  </div>
</template>

<script setup>
import { usePopulation } from '../../composables/usePopulation.js';
import { useLawStore } from '../../engine/lawStore.js';
import { number } from '../../lib/format.js';

const { n, autoRecompute, columns, running, anyRunning, progress, errors, stale, populatieGewijzigd, recompute } = usePopulation();
const { hasChanges } = useLawStore();

/** De kolom waarop je bewerkt heet anders zodra er bewerkingen in staan. */
function kolomTitel(col) {
  if (col.isWerkversie && hasChanges.value) return `${col.titel} (bewerkt)`;
  return col.titel;
}

async function run() {
  try {
    await recompute();
  } catch {
    // fouten staan per kolom in de banner
  }
}
</script>

<style scoped>
.dk { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.dk-row { display: flex; gap: var(--primitives-space-12); align-items: center; flex-wrap: wrap; }
.dk-status { font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>

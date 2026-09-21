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
      <span v-else-if="!anyRunning" class="dk-status">dezelfde {{ number(n) }} leerlingen in elke kolom · {{ autoRecompute ? 'rekent automatisch bij een wijziging' : 'automatisch herberekenen staat uit' }}</span>
    </div>
    <template v-for="col in columns" :key="col.key">
      <nldd-progress-bar
        v-if="running[col.key]"
        :text="`${col.key === 'ist' ? 'Huidig recht' : col.short} doorrekenen`"
        :value="Math.round((progress[col.key] ?? 0) * 100)"
        :max="100"
      ></nldd-progress-bar>
      <nldd-banner v-if="errors[col.key]" variant="critical">
        {{ col.key === 'ist' ? 'Huidig recht' : col.short }}: doorrekenen mislukt: {{ errors[col.key] }}
      </nldd-banner>
    </template>
  </div>
</template>

<script setup>
import { useSimulation } from '../../composables/useSimulation.js';
import { number } from '../../lib/format.js';

const { n, autoRecompute, columns, running, anyRunning, progress, errors, stale, populatieGewijzigd, recompute } = useSimulation();

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

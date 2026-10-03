<script setup>
// The processes and the cells of the runtime. A process with a portal lets
// people log in and submit, a process with handling lets a handler open a
// case and take actions in it, on trial and recorded. A cell shows its
// chronicle and its lexostatuses to whoever is logged in as a handler in a
// process that reads it. The frontend knows no scenario: which processes and
// cells there are comes from GET /api/processes and GET /api/cells.
import { computed, onMounted, ref } from 'vue';
import { fetchCells, fetchProcesses } from './api.js';
import { processLabel } from './text.js';
import CellView from './views/CellView.vue';
import MapView from './views/MapView.vue';
import ProcessView from './views/ProcessView.vue';

const cells = ref([]);
const processes = ref([]);
// What is open: `process:<id>`, `cell:<id>`, the map (MAP), or null for the
// overview.
const chosen = ref(null);
// The map page has a path of its own, so it can be linked: /opbouw.
const MAP = 'opbouw';
if (location.pathname.replace(/\/$/, '') === `/${MAP}`) chosen.value = MAP;
const error = ref('');
const loaded = ref(false);

const portals = computed(() => processes.value.filter((p) => p.portal));
const process = computed(() => processes.value.find((p) => `process:${p.id}` === chosen.value) ?? null);
const cell = computed(() => cells.value.find((c) => `cell:${c.id}` === chosen.value) ?? null);
const cellOf = (p) => cells.value.find((c) => c.id === p.cell) ?? null;

// Along which route the cells reduce (experiment A): `engine` or `compare`
// when the runtime runs with CELL_REDUCTION, otherwise null (the reduction
// DSL). The lexostatuses that deliberately go through the DSL, with reason.
const reduction = computed(() => {
  const r = cells.value.map((c) => c.reduction).filter(Boolean);
  if (!r.length) return null;
  return r.includes('compare') ? 'compare' : 'engine';
});
// The route as the user sees it (Dutch).
const reductionLabel = computed(() => (reduction.value === 'compare' ? 'vergelijk' : reduction.value));
const throughDsl = computed(() =>
  cells.value.flatMap((c) =>
    (c.lexostatuses ?? [])
      .filter((l) => l.reduction?.route === 'dsl')
      .map((l) => `${c.id}/${l.name} (${l.reduction.reason})`),
  ),
);
const reductionExplanation = computed(() => {
  const n = cells.value.flatMap((c) => c.lexostatuses ?? []).filter((l) => l.reduction?.route === 'engine').length;
  const compare = reduction.value === 'compare' ? ' Elke reductie gaat ook langs de DSL; een verschil is een fout.' : '';
  const dsl = throughDsl.value.length ? ` Bewust langs de DSL: ${throughDsl.value.join('; ')}.` : '';
  const runtime = [...new Set(cells.value.flatMap((c) => (c.lexostatuses ?? []).filter((l) => l.reduction?.route === 'runtime').map((l) => l.name)))];
  const itself = runtime.length ? ` Door de runtime zelf: ${runtime.join(', ')}.` : '';
  return `${n} lexostatussen reduceren als engine-run van een regeling (experiment A).${compare}${dsl}${itself}`;
});

onMounted(async () => {
  try {
    [cells.value, processes.value] = await Promise.all([fetchCells(), fetchProcesses()]);
    // If there is only one process with a portal, it opens directly.
    if (portals.value.length === 1 && chosen.value === null) chosen.value = `process:${portals.value[0].id}`;
  } catch (e) {
    error.value = e.message;
  } finally {
    loaded.value = true;
  }
});

function tab(e) {
  const to = e.detail?.item?.dataset?.item;
  chosen.value = to === '' ? null : (to ?? chosen.value);
  history.replaceState(null, '', chosen.value === MAP ? `/${MAP}` : '/');
}

function processText(p) {
  const parts = [`cel ${p.cell}`, p.portal ? 'portaal' : 'geen portaal'];
  if (p.handling) parts.push(`behandeling (werkvoorraad, ${p.handling.actions?.length ?? 0} handelingen)`);
  const sources = [...new Set(p.synthesis.filter((s) => !s.case).map((s) => s.cell))];
  if (sources.length) parts.push(`synthese uit ${sources.join(', ')}`);
  return parts.join('; ');
}

function cellText(c) {
  const parts = [`kroniek ${c.chronicles.join(', ')}`];
  if (c.lexostatuses.length) {
    const name = (l) => (l.reduction?.route ? `${l.name} (${l.reduction.route})` : l.name);
    parts.push(`lexostatus ${c.lexostatuses.map(name).join(', ')}`);
  }
  return parts.join('; ');
}
</script>

<template>
  <nldd-page>
    <nldd-top-navigation-bar
      slot="header"
      no-logo
      :website-title="reduction ? `Cellen en processen · reductie: ${reductionLabel}` : 'Cellen en processen'"
    ></nldd-top-navigation-bar>
    <nldd-simple-section>
      <template v-if="reduction">
        <nldd-inline-dialog icon="info" :text="`Reductie: ${reductionLabel}`" :supporting-text="reductionExplanation"></nldd-inline-dialog>
        <nldd-spacer size="16"></nldd-spacer>
      </template>
      <template v-if="error">
        <nldd-inline-dialog
          variant="alert"
          text="De processen en cellen zijn niet te laden"
          :supporting-text="error"
        ></nldd-inline-dialog>
      </template>
      <template v-else-if="loaded">
        <nldd-tab-bar size="md" accessible-label="Proces of cel" @tabchange="tab">
          <nldd-tab-bar-item data-item="" text="Overzicht" :current="chosen === null || undefined"></nldd-tab-bar-item>
          <nldd-tab-bar-item :data-item="MAP" text="Opbouw" :current="chosen === MAP || undefined"></nldd-tab-bar-item>
          <nldd-tab-bar-item
            v-for="p in processes"
            :key="`process:${p.id}`"
            :data-item="`process:${p.id}`"
            :text="processLabel(p)"
            :current="chosen === `process:${p.id}` || undefined"
          ></nldd-tab-bar-item>
          <nldd-tab-bar-item
            v-for="c in cells"
            :key="`cell:${c.id}`"
            :data-item="`cell:${c.id}`"
            :text="c.id"
            :current="chosen === `cell:${c.id}` || undefined"
          ></nldd-tab-bar-item>
        </nldd-tab-bar>
        <nldd-spacer size="24"></nldd-spacer>
        <MapView v-if="chosen === MAP" :processes="processes" />
        <ProcessView v-else-if="process && cellOf(process)" :key="chosen" :process="process" :cell="cellOf(process)" />
        <CellView
          v-else-if="cell"
          :key="chosen"
          :cell="cell"
          :processes="processes.filter((p) => (p.inspection ?? []).includes(cell.id))"
          @open="chosen = `process:${$event}`"
        />
        <template v-else>
          <nldd-title size="2"><h1>Processen in deze runtime</h1></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-table columns="120px minmax(200px,1fr) minmax(200px,2fr)" accessible-label="Processen" empty-text="Geen processen">
            <nldd-table-row slot="header">
              <nldd-text-cell text=""></nldd-text-cell>
              <nldd-text-cell text="Proces"></nldd-text-cell>
              <nldd-text-cell text="Mogelijkheden"></nldd-text-cell>
            </nldd-table-row>
            <nldd-table-row v-for="p in processes" :key="p.id">
              <nldd-cell>
                <nldd-button variant="secondary" text="Open" :accessible-label="`Open proces ${processLabel(p)}`" @click="chosen = `process:${p.id}`"></nldd-button>
              </nldd-cell>
              <nldd-text-cell :text="processLabel(p)" :supporting-text="p.title ?? undefined"></nldd-text-cell>
              <nldd-text-cell :text="processText(p)"></nldd-text-cell>
            </nldd-table-row>
          </nldd-table>
          <nldd-spacer size="32"></nldd-spacer>
          <nldd-title size="2"><h2>Cellen in deze runtime</h2></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-table columns="120px minmax(200px,1fr) minmax(200px,2fr)" accessible-label="Cellen" empty-text="Geen cellen">
            <nldd-table-row slot="header">
              <nldd-text-cell text=""></nldd-text-cell>
              <nldd-text-cell text="Cel"></nldd-text-cell>
              <nldd-text-cell text="Kronieken en lexostatussen"></nldd-text-cell>
            </nldd-table-row>
            <nldd-table-row v-for="c in cells" :key="c.id">
              <nldd-cell>
                <nldd-button variant="secondary" text="Open" :accessible-label="`Open cel ${c.id}`" @click="chosen = `cell:${c.id}`"></nldd-button>
              </nldd-cell>
              <nldd-text-cell :text="c.id" :supporting-text="c.recording_actor"></nldd-text-cell>
              <nldd-text-cell :text="cellText(c)"></nldd-text-cell>
            </nldd-table-row>
          </nldd-table>
        </template>
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>

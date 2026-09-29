<script setup>
// De processen en de cellen van de runtime. Een proces met een portaal laat
// inloggen en indienen, een proces met een behandeling laat een behandelaar
// een zaak openen en er handelingen in doen, op proef en vastgelegd. Een cel
// toont haar kroniek en haar lexostatussen aan wie als behandelaar is
// ingelogd in een proces dat haar leest. De frontend kent geen casus:
// welke processen en cellen er zijn, komt van GET /api/processes en
// GET /api/cells.
import { computed, onMounted, ref } from 'vue';
import { cellen as haalCellen, processen as haalProcessen } from './api.js';
import CelView from './views/CelView.vue';
import ProcesView from './views/ProcesView.vue';

const cellen = ref([]);
const processen = ref([]);
// Wat open staat: `proces:<id>` of `cel:<id>`, of null voor het overzicht.
const gekozen = ref(null);
const fout = ref('');
const geladen = ref(false);

const portalen = computed(() => processen.value.filter((p) => p.portal));
const proces = computed(() => processen.value.find((p) => `proces:${p.id}` === gekozen.value) ?? null);
const cel = computed(() => cellen.value.find((c) => `cel:${c.id}` === gekozen.value) ?? null);
const celVan = (p) => cellen.value.find((c) => c.id === p.cell) ?? null;

// Langs welke route de cellen reduceren (experiment A): `engine` of
// `vergelijk` (runtime: `compare`) als de runtime met CELL_REDUCTION draait, anders null (de
// reductie-DSL). De lexostatussen die bewust langs de DSL gaan, met reden.
const reductie = computed(() => {
  const r = cellen.value.map((c) => c.reduction).filter(Boolean);
  if (!r.length) return null;
  return r.includes('compare') ? 'vergelijk' : 'engine';
});
const langsDeDsl = computed(() =>
  cellen.value.flatMap((c) =>
    (c.lexostatuses ?? [])
      .filter((l) => l.reduction?.route === 'dsl')
      .map((l) => `${c.id}/${l.name} (${l.reduction.reason})`),
  ),
);
const reductieUitleg = computed(() => {
  const n = cellen.value.flatMap((c) => c.lexostatuses ?? []).filter((l) => l.reduction?.route === 'engine').length;
  const vergelijk = reductie.value === 'vergelijk' ? ' Elke reductie gaat ook langs de DSL; een verschil is een fout.' : '';
  const dsl = langsDeDsl.value.length ? ` Bewust langs de DSL: ${langsDeDsl.value.join('; ')}.` : '';
  const runtime = [...new Set(cellen.value.flatMap((c) => (c.lexostatuses ?? []).filter((l) => l.reduction?.route === 'runtime').map((l) => l.name)))];
  const zelf = runtime.length ? ` Door de runtime zelf: ${runtime.join(', ')}.` : '';
  return `${n} lexostatussen reduceren als engine-run van een regeling (experiment A).${vergelijk}${dsl}${zelf}`;
});

onMounted(async () => {
  try {
    [cellen.value, processen.value] = await Promise.all([haalCellen(), haalProcessen()]);
    // Is er maar een proces met een portaal, dan opent dat direct.
    if (portalen.value.length === 1) gekozen.value = `proces:${portalen.value[0].id}`;
  } catch (e) {
    fout.value = e.message;
  } finally {
    geladen.value = true;
  }
});

function tab(e) {
  const naar = e.detail?.item?.dataset?.item;
  gekozen.value = naar === '' ? null : (naar ?? gekozen.value);
}

function procesTekst(p) {
  const delen = [`cel ${p.cell}`, p.portal ? 'portaal' : 'geen portaal'];
  if (p.handling) delen.push(`behandeling (werkvoorraad, ${p.handling.actions?.length ?? 0} handelingen)`);
  const bronnen = [...new Set(p.synthesis.filter((s) => !s.case).map((s) => s.cell))];
  if (bronnen.length) delen.push(`synthese uit ${bronnen.join(', ')}`);
  return delen.join('; ');
}

function celTekst(c) {
  const delen = [`kroniek ${c.chronicles.join(', ')}`];
  if (c.lexostatuses.length) {
    const naam = (l) => (l.reduction?.route ? `${l.name} (${l.reduction.route})` : l.name);
    delen.push(`lexostatus ${c.lexostatuses.map(naam).join(', ')}`);
  }
  return delen.join('; ');
}
</script>

<template>
  <nldd-page>
    <nldd-top-navigation-bar
      slot="header"
      no-logo
      :website-title="reductie ? `Cellen en processen · reductie: ${reductie}` : 'Cellen en processen'"
    ></nldd-top-navigation-bar>
    <nldd-simple-section>
      <template v-if="reductie">
        <nldd-inline-dialog icon="info" :text="`Reductie: ${reductie}`" :supporting-text="reductieUitleg"></nldd-inline-dialog>
        <nldd-spacer size="16"></nldd-spacer>
      </template>
      <template v-if="fout">
        <nldd-inline-dialog
          variant="alert"
          text="De processen en cellen zijn niet te laden"
          :supporting-text="fout"
        ></nldd-inline-dialog>
      </template>
      <template v-else-if="geladen">
        <nldd-tab-bar size="md" accessible-label="Proces of cel" @tabchange="tab">
          <nldd-tab-bar-item data-item="" text="Overzicht" :current="gekozen === null || undefined"></nldd-tab-bar-item>
          <nldd-tab-bar-item
            v-for="p in processen"
            :key="`proces:${p.id}`"
            :data-item="`proces:${p.id}`"
            :text="p.id"
            :current="gekozen === `proces:${p.id}` || undefined"
          ></nldd-tab-bar-item>
          <nldd-tab-bar-item
            v-for="c in cellen"
            :key="`cel:${c.id}`"
            :data-item="`cel:${c.id}`"
            :text="c.id"
            :current="gekozen === `cel:${c.id}` || undefined"
          ></nldd-tab-bar-item>
        </nldd-tab-bar>
        <nldd-spacer size="24"></nldd-spacer>
        <ProcesView v-if="proces && celVan(proces)" :key="gekozen" :proces="proces" :cel="celVan(proces)" />
        <CelView
          v-else-if="cel"
          :key="gekozen"
          :cel="cel"
          :processen="processen.filter((p) => (p.inspection ?? []).includes(cel.id))"
          @open="gekozen = `proces:${$event}`"
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
            <nldd-table-row v-for="p in processen" :key="p.id">
              <nldd-cell>
                <nldd-button variant="secondary" text="Open" :accessible-label="`Open proces ${p.id}`" @click="gekozen = `proces:${p.id}`"></nldd-button>
              </nldd-cell>
              <nldd-text-cell :text="p.id" :supporting-text="p.title ?? undefined"></nldd-text-cell>
              <nldd-text-cell :text="procesTekst(p)"></nldd-text-cell>
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
            <nldd-table-row v-for="c in cellen" :key="c.id">
              <nldd-cell>
                <nldd-button variant="secondary" text="Open" :accessible-label="`Open cel ${c.id}`" @click="gekozen = `cel:${c.id}`"></nldd-button>
              </nldd-cell>
              <nldd-text-cell :text="c.id" :supporting-text="c.recording_actor"></nldd-text-cell>
              <nldd-text-cell :text="celTekst(c)"></nldd-text-cell>
            </nldd-table-row>
          </nldd-table>
        </template>
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>

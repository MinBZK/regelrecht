<script setup>
// De processen en de cellen van de runtime. Een proces met een portaal laat
// inloggen en indienen, een proces met een behandeling laat een behandelaar
// een zaak openen, een proefbesluit uitrekenen en het besluit nemen. Een cel
// toont haar kroniek en haar lexostatussen. De frontend kent geen casus:
// welke processen en cellen er zijn, komt van GET /api/processen en
// GET /api/cellen.
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

const portalen = computed(() => processen.value.filter((p) => p.portaal));
const proces = computed(() => processen.value.find((p) => `proces:${p.id}` === gekozen.value) ?? null);
const cel = computed(() => cellen.value.find((c) => `cel:${c.id}` === gekozen.value) ?? null);
const celVan = (p) => cellen.value.find((c) => c.id === p.cel) ?? null;

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
  const delen = [`cel ${p.cel}`, p.portaal ? 'portaal' : 'geen portaal'];
  if (p.behandeling) delen.push(`behandeling (werkvoorraad, besluit ${p.behandeling.regeling})`);
  const bronnen = [...new Set(p.synthese.filter((s) => !s.zaak).map((s) => s.cel))];
  if (bronnen.length) delen.push(`synthese uit ${bronnen.join(', ')}`);
  return delen.join('; ');
}

function celTekst(c) {
  const delen = [`kroniek ${c.kronieken.join(', ')}`];
  if (c.lexostatussen.length) delen.push(`lexostatus ${c.lexostatussen.map((l) => l.name).join(', ')}`);
  return delen.join('; ');
}
</script>

<template>
  <nldd-page>
    <nldd-top-navigation-bar slot="header" no-logo website-title="Cellen en processen"></nldd-top-navigation-bar>
    <nldd-simple-section>
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
        <CelView v-else-if="cel" :key="gekozen" :cel="cel" />
        <template v-else>
          <nldd-title size="2"><h1>Processen in deze runtime</h1></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-table columns="minmax(200px,1fr) minmax(200px,2fr) 120px" accessible-label="Processen" empty-text="Geen processen">
            <nldd-table-row slot="header">
              <nldd-text-cell text="Proces"></nldd-text-cell>
              <nldd-text-cell text="Mogelijkheden"></nldd-text-cell>
              <nldd-text-cell text=""></nldd-text-cell>
            </nldd-table-row>
            <nldd-table-row v-for="p in processen" :key="p.id">
              <nldd-text-cell :text="p.id" :supporting-text="p.titel ?? undefined"></nldd-text-cell>
              <nldd-text-cell :text="procesTekst(p)"></nldd-text-cell>
              <nldd-cell>
                <nldd-button variant="secondary" text="Open" @click="gekozen = `proces:${p.id}`"></nldd-button>
              </nldd-cell>
            </nldd-table-row>
          </nldd-table>
          <nldd-spacer size="32"></nldd-spacer>
          <nldd-title size="2"><h2>Cellen in deze runtime</h2></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-table columns="minmax(200px,1fr) minmax(200px,2fr) 120px" accessible-label="Cellen" empty-text="Geen cellen">
            <nldd-table-row slot="header">
              <nldd-text-cell text="Cel"></nldd-text-cell>
              <nldd-text-cell text="Kronieken en lexostatussen"></nldd-text-cell>
              <nldd-text-cell text=""></nldd-text-cell>
            </nldd-table-row>
            <nldd-table-row v-for="c in cellen" :key="c.id">
              <nldd-text-cell :text="c.id" :supporting-text="c.recording_actor"></nldd-text-cell>
              <nldd-text-cell :text="celTekst(c)"></nldd-text-cell>
              <nldd-cell>
                <nldd-button variant="secondary" text="Open" @click="gekozen = `cel:${c.id}`"></nldd-button>
              </nldd-cell>
            </nldd-table-row>
          </nldd-table>
        </template>
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>

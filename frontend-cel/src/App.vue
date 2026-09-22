<script setup>
// De cellen van de runtime. Een cel met een portaal laat inloggen en
// indienen; elke cel toont haar kroniek en haar lexostatussen. De frontend
// kent geen casus: welke cellen er zijn, komt van GET /api/cellen.
import { computed, onMounted, ref } from 'vue';
import { cellen as haalCellen } from './api.js';
import CelView from './views/CelView.vue';

const lijst = ref([]);
const gekozen = ref(null);
const fout = ref('');
const geladen = ref(false);

const portalen = computed(() => lijst.value.filter((c) => c.portaal));
const cel = computed(() => lijst.value.find((c) => c.id === gekozen.value) ?? null);

onMounted(async () => {
  try {
    lijst.value = await haalCellen();
    // Is er maar een cel met een portaal, dan opent die direct.
    if (portalen.value.length === 1) gekozen.value = portalen.value[0].id;
  } catch (e) {
    fout.value = e.message;
  } finally {
    geladen.value = true;
  }
});

function tab(e) {
  const naar = e.detail?.item?.dataset?.cel;
  gekozen.value = naar === '' ? null : (naar ?? gekozen.value);
}

function mogelijkheden(c) {
  const delen = [c.portaal ? 'portaal' : 'geen portaal'];
  if (c.lexostatussen.length) delen.push(`lexostatus ${c.lexostatussen.map((l) => l.name).join(', ')}`);
  if (c.synthese.length) delen.push(`synthese uit ${c.synthese.map((s) => s.cel).join(', ')}`);
  return delen.join('; ');
}
</script>

<template>
  <nldd-page>
    <nldd-top-navigation-bar slot="header" no-logo website-title="Cellen"></nldd-top-navigation-bar>
    <nldd-simple-section>
      <template v-if="fout">
        <nldd-inline-dialog variant="alert" text="De cellen zijn niet te laden" :supporting-text="fout"></nldd-inline-dialog>
      </template>
      <template v-else-if="geladen">
        <nldd-tab-bar size="md" accessible-label="Cel" @tabchange="tab">
          <nldd-tab-bar-item data-cel="" text="Overzicht" :current="gekozen === null || undefined"></nldd-tab-bar-item>
          <nldd-tab-bar-item
            v-for="c in lijst"
            :key="c.id"
            :data-cel="c.id"
            :text="c.id"
            :current="gekozen === c.id || undefined"
          ></nldd-tab-bar-item>
        </nldd-tab-bar>
        <nldd-spacer size="24"></nldd-spacer>
        <CelView v-if="cel" :key="cel.id" :cel="cel" />
        <template v-else>
          <nldd-title size="2"><h1>Cellen in deze runtime</h1></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-table columns="minmax(200px,1fr) minmax(200px,2fr) 120px" accessible-label="Cellen" empty-text="Geen cellen">
            <nldd-table-row slot="header">
              <nldd-text-cell text="Cel"></nldd-text-cell>
              <nldd-text-cell text="Mogelijkheden"></nldd-text-cell>
              <nldd-text-cell text=""></nldd-text-cell>
            </nldd-table-row>
            <nldd-table-row v-for="c in lijst" :key="c.id">
              <nldd-text-cell :text="c.id" :supporting-text="c.titel ?? undefined"></nldd-text-cell>
              <nldd-text-cell :text="mogelijkheden(c)"></nldd-text-cell>
              <nldd-cell>
                <nldd-button variant="secondary" text="Open" @click="gekozen = c.id"></nldd-button>
              </nldd-cell>
            </nldd-table-row>
          </nldd-table>
        </template>
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>

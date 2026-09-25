<script setup>
// Een zaak: waar zij staat in de procedure (RFC-008), welke
// rechtsbescherming daaruit volgt, de stand van wat nog te betalen is, en de
// handelingen die het proces in de zaak kent. Welke handelingen er zijn, wat
// ze vragen en of ze nu kunnen, zegt de runtime (uit de stage en de wet);
// deze pagina kent geen besluit, bekendmaking of betaling bij naam.
import { computed, inject, onMounted, ref } from 'vue';
import Grammen from '../components/Grammen.vue';
import Handeling from '../components/Handeling.vue';
import { soortVan, uitkomstTekst, waardeTekst } from '../tekst.js';

const props = defineProps({ zaakkenmerk: { type: String, required: true } });
const emit = defineEmits(['terug']);
const api = inject('api');

const zaak = ref(null);
const fout = ref('');
const gekozen = ref(null);
// Een nieuwe sleutel na het vastleggen: het paneel toont dan de nieuwe stand.
const versie = ref(0);

async function laad() {
  try {
    zaak.value = await api.zaak(props.zaakkenmerk);
  } catch (e) {
    fout.value = e.message;
  }
}

onMounted(laad);

const handeling = computed(() => zaak.value?.handelingen.find((h) => h.naam === gekozen.value) ?? null);

function status(h) {
  if (soortVan(h) !== 'feit' && h.vastgelegd > 0) return 'vastgelegd';
  if (!h.beschikbaar) return 'nog niet';
  return h.vastgelegd > 0 ? `kan (${h.vastgelegd} keer vastgelegd)` : 'kan';
}

function soortTekst(h) {
  const s = soortVan(h);
  if (s === 'feit') return 'feit';
  return `${s}, stage ${h.stage}`;
}

// De stand van de feiten met een bedrag (zoals wat er nog te betalen is): de
// uitkomsten van hun proef zonder formulier, dus zoals de zaak nu is.
const stand = computed(() => {
  const uit = [];
  for (const h of zaak.value?.handelingen ?? []) {
    if (!h.formulier.some((v) => v.type === 'bedrag')) continue;
    for (const [naam, w] of Object.entries(h.proef?.uitkomsten ?? {})) {
      uit.push({ sleutel: h.naam + naam, handeling: h.label, naam, waarde: uitkomstTekst(naam, w) });
    }
  }
  return uit;
});

const route = computed(() => zaak.value?.rechtsbescherming ?? null);
const routeUitkomsten = computed(() =>
  Object.entries(route.value?.uitkomsten ?? {}).map(([naam, w]) => ({ naam, waarde: waardeTekst(w) })),
);

async function vastgelegd() {
  await laad();
  versie.value++;
}
</script>

<template>
  <nldd-button variant="neutral-transparent" start-icon="arrow-left" text="Werkvoorraad" @click="emit('terug')"></nldd-button>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-title size="2">
    <h1>Zaak {{ zaakkenmerk }}</h1>
    <span slot="subtitle" v-if="zaak?.procedure">Procedure {{ zaak.procedure.id }}</span>
  </nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="zaak">
    <template v-if="zaak.procedure">
      <nldd-title size="3"><h2>Procedure</h2></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(160px,1fr) minmax(280px,2fr) 140px minmax(160px,1fr)" accessible-label="Stages van de procedure">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Stage"></nldd-text-cell>
          <nldd-text-cell text="Omschrijving"></nldd-text-cell>
          <nldd-text-cell text="In de zaak"></nldd-text-cell>
          <nldd-text-cell text="Handeling"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="s in zaak.procedure.stages" :key="s.name">
          <nldd-text-cell :text="s.name"></nldd-text-cell>
          <nldd-text-cell :text="s.description ?? ''"></nldd-text-cell>
          <nldd-text-cell :text="s.vastgelegd ? 'vastgelegd' : ''"></nldd-text-cell>
          <nldd-text-cell :text="s.handeling ?? ''"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <template v-if="route">
      <nldd-inline-dialog
        text="Rechtsbescherming"
        :supporting-text="`Stage ${route.stage} loopt na ${route.na}: ${route.description ?? ''} Afgeleid uit de procedure en ${route.grondslag.join(', ')}.`"
      ></nldd-inline-dialog>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(240px,1fr) minmax(160px,1fr)" accessible-label="Rechtsbescherming">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Uitkomst"></nldd-text-cell>
          <nldd-text-cell text="Waarde"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="u in routeUitkomsten" :key="u.naam">
          <nldd-text-cell :text="u.naam"></nldd-text-cell>
          <nldd-text-cell :text="u.waarde"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <template v-if="stand.length">
      <nldd-title size="3">
        <h2>Betaalstand</h2>
        <span slot="subtitle">Zoals de wet het nu uitrekent over de zaak</span>
      </nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(200px,1fr) minmax(240px,1fr) minmax(160px,1fr)" accessible-label="Betaalstand">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Handeling"></nldd-text-cell>
          <nldd-text-cell text="Uitkomst"></nldd-text-cell>
          <nldd-text-cell text="Waarde"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="s in stand" :key="s.sleutel">
          <nldd-text-cell :text="s.handeling"></nldd-text-cell>
          <nldd-text-cell :text="s.naam"></nldd-text-cell>
          <nldd-text-cell :text="s.waarde"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <nldd-title size="3"><h2>Handelingen</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-table columns="minmax(240px,2fr) minmax(160px,1fr) minmax(160px,1fr) 110px" accessible-label="Handelingen in de zaak">
      <nldd-table-row slot="header">
        <nldd-text-cell text="Handeling"></nldd-text-cell>
        <nldd-text-cell text="Soort"></nldd-text-cell>
        <nldd-text-cell text="Stand"></nldd-text-cell>
        <nldd-text-cell text=""></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="h in zaak.handelingen" :key="h.naam">
        <nldd-text-cell :text="h.label" :supporting-text="h.artikel"></nldd-text-cell>
        <nldd-text-cell :text="soortTekst(h)"></nldd-text-cell>
        <nldd-text-cell :text="status(h)" :supporting-text="h.reden ?? ''"></nldd-text-cell>
        <nldd-cell>
          <nldd-button variant="secondary" text="Open" @click="gekozen = h.naam"></nldd-button>
        </nldd-cell>
      </nldd-table-row>
    </nldd-table>

    <template v-if="handeling">
      <nldd-spacer size="24"></nldd-spacer>
      <Handeling :key="`${handeling.naam}-${versie}`" :zaakkenmerk="zaakkenmerk" :handeling="handeling" @vastgelegd="vastgelegd" />
    </template>

    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="3"><h2>Grammen van de zaak</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <Grammen :items="zaak.grammen" />
  </template>
</template>

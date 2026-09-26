<script setup>
// Een zaak: de besluiten erin, elk met zijn stages in de procedure (RFC-008),
// de rechtsbescherming die daaruit volgt, de stand van wat er op dat besluit
// nog te betalen is, en de handelingen die op dat besluit handelen; daarna
// wat bij de zaak zelf hoort (de aanvraag, een besluit dat nog genomen kan
// worden, een feit uit het verloop). Welke besluiten er zijn, welke
// handelingen, wat ze vragen en of ze nu kunnen, zegt de runtime (uit de
// stage en de wet); deze pagina kent geen besluit, bekendmaking of betaling
// bij naam.
import { computed, inject, onMounted, ref } from 'vue';
import Betaalstand from '../components/Betaalstand.vue';
import Grammen from '../components/Grammen.vue';
import Handeling from '../components/Handeling.vue';
import Handelingen from '../components/Handelingen.vue';
import { waardeTekst } from '../tekst.js';
import { besluitKop, indeling } from '../zaak.js';

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
const delen = computed(() => indeling(zaak.value));

function routeTekst(r) {
  return `Stage ${r.stage} loopt na ${r.na}: ${r.description ?? ''} Afgeleid uit de procedure en ${r.grondslag.join(', ')}.`;
}

function routeUitkomsten(r) {
  return Object.entries(r?.uitkomsten ?? {}).map(([naam, w]) => ({ naam, waarde: waardeTekst(w) }));
}

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
    <span slot="subtitle" v-if="zaak">{{ delen.besluiten.length }} besluit(en) in de zaak</span>
  </nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="zaak">
    <template v-for="b in delen.besluiten" :key="b.besluitkenmerk">
      <nldd-title size="3">
        <h2>Besluit {{ b.nummer }}: {{ b.label }}</h2>
        <span slot="subtitle">{{ besluitKop(b) }} ({{ b.artikel }})</span>
      </nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <template v-if="b.procedure">
        <nldd-table columns="minmax(160px,1fr) minmax(280px,2fr) 140px minmax(160px,1fr)" :accessible-label="`Stages van besluit ${b.nummer}`">
          <nldd-table-row slot="header">
            <nldd-text-cell text="Stage"></nldd-text-cell>
            <nldd-text-cell text="Omschrijving"></nldd-text-cell>
            <nldd-text-cell text="In de zaak"></nldd-text-cell>
            <nldd-text-cell text="Handeling"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="s in b.procedure.stages" :key="s.name">
            <nldd-text-cell :text="s.name"></nldd-text-cell>
            <nldd-text-cell :text="s.description ?? ''"></nldd-text-cell>
            <nldd-text-cell :text="s.vastgelegd ? 'vastgelegd' : ''"></nldd-text-cell>
            <nldd-text-cell :text="s.handeling ?? ''"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <template v-if="b.rechtsbescherming">
        <nldd-inline-dialog text="Rechtsbescherming" :supporting-text="routeTekst(b.rechtsbescherming)"></nldd-inline-dialog>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-table columns="minmax(240px,1fr) minmax(160px,1fr)" :accessible-label="`Rechtsbescherming bij besluit ${b.nummer}`">
          <nldd-table-row slot="header">
            <nldd-text-cell text="Uitkomst"></nldd-text-cell>
            <nldd-text-cell text="Waarde"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="u in routeUitkomsten(b.rechtsbescherming)" :key="u.naam">
            <nldd-text-cell :text="u.naam"></nldd-text-cell>
            <nldd-text-cell :text="u.waarde"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <template v-if="b.betaalstand.length">
        <nldd-title size="4"><h3>Betaalstand</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <Betaalstand :stand="b.betaalstand" />
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <Handelingen :handelingen="b.handelingen" :label="`Handelingen bij besluit ${b.nummer}`" @open="gekozen = $event" />
      <nldd-spacer size="24"></nldd-spacer>
    </template>

    <nldd-title size="3">
      <h2>De zaak</h2>
      <span slot="subtitle" v-if="zaak.procedure">Procedure {{ zaak.procedure.id }}; wat bij geen besluit hoort</span>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <template v-if="zaak.procedure">
      <nldd-table columns="minmax(160px,1fr) minmax(280px,2fr) 140px" accessible-label="Stages van de zaak">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Stage"></nldd-text-cell>
          <nldd-text-cell text="Omschrijving"></nldd-text-cell>
          <nldd-text-cell text="In de zaak"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="s in zaak.procedure.stages.filter((s) => s.vastgelegd)" :key="s.name">
          <nldd-text-cell :text="s.name"></nldd-text-cell>
          <nldd-text-cell :text="s.description ?? ''"></nldd-text-cell>
          <nldd-text-cell text="vastgelegd"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <nldd-spacer size="8"></nldd-spacer>
    </template>
    <template v-if="delen.betaalstand.length">
      <Betaalstand :stand="delen.betaalstand" />
      <nldd-spacer size="8"></nldd-spacer>
    </template>
    <Handelingen v-if="delen.overig.length" :handelingen="delen.overig" label="Handelingen in de zaak" @open="gekozen = $event" />

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

<script setup>
// Een zaak: haar grammen, het besluitformulier met de oordelen van de
// behandelaar, een proefbesluit en het besluit zelf. Een proefbesluit legt
// niets vast: het zegt welke uitkomsten de engine geeft, of wat er nog mist,
// en per parameter waar hij vandaan kwam. "Besluit nemen" legt datzelfde
// vast als decretogram; daarna verdwijnt de zaak uit de werkvoorraad.
import { computed, inject, onMounted, ref } from 'vue';
import Grammen from '../components/Grammen.vue';
import Invoer from '../components/Invoer.vue';
import { herkomstRijen, waardeTekst } from '../tekst.js';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';

const props = defineProps({ zaakkenmerk: { type: String, required: true } });
const emit = defineEmits(['terug']);
const api = inject('api');

const zaak = ref(null);
const oordelen = ref({});
const proef = ref(null);
const besluit = ref(null);
const fout = ref('');
const bezig = ref(false);
const beslissen = ref(false);

onMounted(async () => {
  try {
    zaak.value = await api.zaak(props.zaakkenmerk);
    proef.value = zaak.value.proefbesluit;
    for (const v of zaak.value.besluit.formulier) oordelen.value[v.naam] = null;
  } catch (e) {
    fout.value = e.message;
  }
});

const groepen = computed(() => {
  const uit = [];
  for (const v of zaak.value?.besluit.formulier ?? []) {
    const titel = v.groep ?? '';
    let g = uit.find((x) => x.titel === titel);
    if (!g) uit.push((g = { titel, velden: [] }));
    g.velden.push(v);
  }
  return uit;
});

function zet(naam, waarde) {
  oordelen.value = { ...oordelen.value, [naam]: waarde === '' ? null : waarde };
}

function ingevuld() {
  return Object.fromEntries(Object.entries(oordelen.value).filter(([, w]) => w !== null));
}

async function proefbesluit() {
  fout.value = '';
  bezig.value = true;
  try {
    proef.value = await api.proefbesluit(props.zaakkenmerk, ingevuld());
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

// Het besluit nemen: de engine rekent opnieuw, en de cel legt de uitkomst
// vast als decretogram. Weigert zij, dan blijft de kroniek zoals hij was.
async function besluitNemen() {
  fout.value = '';
  beslissen.value = true;
  try {
    const uitslag = await api.besluit(props.zaakkenmerk, ingevuld());
    besluit.value = uitslag;
    proef.value = uitslag.proefbesluit;
    zaak.value = await api.zaak(props.zaakkenmerk);
  } catch (e) {
    fout.value = e.message;
  } finally {
    beslissen.value = false;
  }
}

const uitkomsten = computed(() => Object.entries(proef.value?.uitkomsten ?? {}).map(([naam, w]) => ({ naam, waarde: waardeTekst(w) })));
const herkomst = computed(() => herkomstRijen(proef.value?.parameters, proef.value?.herkomst));
const nietGeleverd = computed(() => proef.value?.niet_geleverd ?? []);
</script>

<template>
  <nldd-button variant="neutral-transparent" start-icon="arrow-left" text="Werkvoorraad" @click="emit('terug')"></nldd-button>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-title size="2">
    <h1>Zaak {{ zaakkenmerk }}</h1>
    <span slot="subtitle" v-if="zaak">Besluit: {{ zaak.besluit.artikel }}</span>
  </nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="zaak">
    <nldd-title size="3"><h2>Besluitformulier</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-form novalidate @submit.prevent="proefbesluit">
      <template v-for="g in groepen" :key="g.titel">
        <nldd-form-section v-if="g.titel" :text="g.titel"></nldd-form-section>
        <nldd-form-field v-for="v in g.velden" :key="v.naam" :label="v.label" :supporting-label="v.naam">
          <Invoer :soort="v.type" :label="v.label" :model-value="oordelen[v.naam]" @update:model-value="zet(v.naam, $event)" />
        </nldd-form-field>
      </template>
      <nldd-form-actions>
        <nldd-button variant="primary" type="submit" text="Proefbesluit" :loading="bezig || undefined"></nldd-button>
        <nldd-button
          variant="secondary"
          type="button"
          text="Besluit nemen"
          :loading="beslissen || undefined"
          :disabled="besluit !== null || undefined"
          @click="besluitNemen"
        ></nldd-button>
      </nldd-form-actions>
    </nldd-form>

    <template v-if="proef">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="3">
        <h2>Proefbesluit</h2>
        <span slot="subtitle">{{ proef.artikel }}, peildatum {{ proef.peildatum }}. Er is niets vastgelegd.</span>
      </nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-container layout="row" gap="8" vertical-alignment="center">
        <nldd-inline-dialog
          :variant="proef.te_nemen ? 'success' : 'alert'"
          :text="proef.te_nemen ? 'Het besluit is te nemen' : 'Niet te nemen'"
          :supporting-text="proef.reden"
        ></nldd-inline-dialog>
        <TraceKnop v-if="proef.trace" :trace="proef.trace" :titel="proef.artikel" />
      </nldd-container>
      <template v-if="uitkomsten.length">
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-table columns="minmax(240px,1fr) minmax(160px,1fr)" accessible-label="Uitkomsten">
          <nldd-table-row slot="header">
            <nldd-text-cell text="Uitkomst"></nldd-text-cell>
            <nldd-text-cell text="Waarde"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="u in uitkomsten" :key="u.naam">
            <nldd-text-cell :text="u.naam"></nldd-text-cell>
            <nldd-text-cell :text="u.waarde"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
      </template>
      <template v-for="r in proef.rijen ?? []" :key="r.parameter">
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-title size="4"><h3>Samengesteld per regel: {{ r.parameter }}</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-table columns="minmax(200px,1fr) minmax(200px,1fr) 120px 140px" accessible-label="Bronnen per regel">
          <nldd-table-row slot="header">
            <nldd-text-cell text="Cel"></nldd-text-cell>
            <nldd-text-cell text="Lexostatus"></nldd-text-cell>
            <nldd-text-cell text="Regels"></nldd-text-cell>
            <nldd-text-cell text="Status"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="b in r.bronnen" :key="b.cel + b.lexostatus">
            <nldd-text-cell :text="b.cel" :supporting-text="b.transport"></nldd-text-cell>
            <nldd-text-cell :text="b.lexostatus"></nldd-text-cell>
            <nldd-text-cell :text="String(b.bevraagd)"></nldd-text-cell>
            <nldd-text-cell :text="b.status.replace('_', ' ')" :supporting-text="b.fout"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
        <template v-if="r.mist?.length">
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-inline-dialog
            icon="warning"
            icon-color="warning"
            :text="`Kolommen zonder waarde: ${r.mist.join(', ')}`"
            supporting-text="Er wordt niets aangevuld."
          ></nldd-inline-dialog>
        </template>
      </template>
      <template v-for="b in proef.bronnen" :key="b.cel + b.lexostatus">
        <nldd-inline-dialog
          v-if="b.status !== 'bevraagd'"
          variant="alert"
          :text="`Bron ${b.cel}: ${b.status.replace('_', ' ')}`"
          :supporting-text="b.fout"
        ></nldd-inline-dialog>
      </template>
      <template v-if="nietGeleverd.length">
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-title size="4"><h3>Niet geleverd</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-table columns="minmax(220px,1fr) minmax(200px,1fr) minmax(300px,2fr)" accessible-label="Niet geleverde parameters">
          <nldd-table-row slot="header">
            <nldd-text-cell text="Parameter"></nldd-text-cell>
            <nldd-text-cell text="Artikel"></nldd-text-cell>
            <nldd-text-cell text="Herkomst volgens het model"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="n in nietGeleverd" :key="n.naam">
            <nldd-text-cell :text="n.naam" :supporting-text="n.type"></nldd-text-cell>
            <nldd-text-cell :text="n.artikel"></nldd-text-cell>
            <nldd-text-cell :text="n.omschrijving ?? ''"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
      </template>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="4"><h3>Herkomst per parameter</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(220px,1fr) 160px minmax(220px,1fr)" accessible-label="Herkomst per parameter">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Parameter"></nldd-text-cell>
          <nldd-text-cell text="Waarde"></nldd-text-cell>
          <nldd-text-cell text="Herkomst"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="h in herkomst" :key="h.naam">
          <nldd-text-cell :text="h.naam"></nldd-text-cell>
          <nldd-text-cell :text="h.waarde"></nldd-text-cell>
          <nldd-text-cell :text="h.bron"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
    </template>

    <template v-if="besluit">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="3">
        <h2>Het besluit</h2>
        <span slot="subtitle">Vastgelegd als decretogram, stage {{ besluit.gram.stage }}, in zaak {{ zaakkenmerk }}</span>
      </nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-inline-dialog
        variant="success"
        text="Het besluit is vastgelegd"
        :supporting-text="`${besluit.gram.legal_character} / ${besluit.gram.decision_type}, ${besluit.gram.regulation} (${besluit.gram.regulation_valid_from})`"
      ></nldd-inline-dialog>
      <nldd-inline-dialog
        v-for="w in besluit.waarschuwingen"
        :key="w"
        icon="warning"
        icon-color="warning"
        text="Waarschuwing"
        :supporting-text="w"
      ></nldd-inline-dialog>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-code-viewer language="yaml" wrap>{{ besluit.yaml }}</nldd-code-viewer>
    </template>

    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="3"><h2>Grammen van de zaak</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <Grammen :items="zaak.grammen" />
  </template>
</template>

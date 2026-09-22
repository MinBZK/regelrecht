<script setup>
// Het formulier, opgebouwd uit GET /api/stroom. Labels, soorten en volgorde
// komen uit het formulierbestand dat de cel meelevert; zonder dat bestand is
// het label de veldnaam en elk veld tekst.
import { computed, inject, onMounted, ref } from 'vue';
import { external } from '../formulier.js';
import { herkomstRijen } from '../tekst.js';
import Invoer from '../components/Invoer.vue';
import TabelInvoer from '../components/TabelInvoer.vue';

const api = inject('api');

const emit = defineEmits(['ingediend']);

const stroom = ref(null);
const waarden = ref({});
const fout = ref('');
const toets = ref(null);
const bezig = ref('');

onMounted(async () => {
  try {
    stroom.value = await api.stroom();
    for (const v of stroom.value.velden) {
      waarden.value[v.naam] = v.type === 'tabel' ? [{}] : null;
    }
  } catch (e) {
    fout.value = e.message;
  }
});

// Velden per groep, in de volgorde van het formulier.
const groepen = computed(() => {
  const uit = [];
  for (const v of stroom.value?.velden ?? []) {
    const titel = v.groep ?? '';
    let g = uit.find((x) => x.titel === titel);
    if (!g) uit.push((g = { titel, velden: [] }));
    g.velden.push(v);
  }
  return uit;
});

function zet(naam, waarde) {
  waarden.value = { ...waarden.value, [naam]: waarde };
  toets.value = null;
}

async function controleer() {
  fout.value = '';
  bezig.value = 'toets';
  try {
    toets.value = await api.toets(external(waarden.value));
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = '';
  }
}

async function indienen() {
  fout.value = '';
  bezig.value = 'indienen';
  try {
    const r = await api.indienen(external(waarden.value));
    emit('ingediend', r.gram);
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = '';
  }
}

const uitslag = computed(() => toets.value?.uitslag ?? null);

// Per parameter die naar de engine ging: de waarde en waar hij vandaan kwam,
// de eigen lexostatus of een andere cel.
const herkomst = computed(() => herkomstRijen(toets.value?.parameters, toets.value?.herkomst));

const bronnen = computed(() => toets.value?.bronnen ?? []);

const uitslagTekst = computed(() => {
  const u = uitslag.value;
  if (!u) return '';
  if (!u.te_beoordelen) return 'Niet te beoordelen';
  return `${u.uitkomst}: ${u.waarde === true ? 'ja' : u.waarde === false ? 'nee' : JSON.stringify(u.waarde)}`;
});
const uitslagToelichting = computed(() => {
  const u = uitslag.value;
  if (!u) return '';
  const delen = [];
  if (u.reden) delen.push(u.reden);
  if (u.ontbreekt?.length) delen.push(`Ontbreekt: ${u.ontbreekt.join(', ')}`);
  const niet = toets.value?.lexostatus?.niet_afgeleid ?? [];
  if (niet.length) delen.push(`Niet af te leiden uit het concept: ${niet.join(', ')}`);
  return delen.join('. ');
});
</script>

<template>
  <nldd-title size="2">
    <h1>{{ stroom?.titel ?? 'Indienen' }}</h1>
    <span slot="subtitle" v-if="stroom">{{ stroom.event }} in stroom {{ stroom.stroom?.$id }}</span>
  </nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="stroom">
    <nldd-form novalidate @submit.prevent="indienen">
      <template v-for="g in groepen" :key="g.titel">
        <nldd-form-section v-if="g.titel" :text="g.titel"></nldd-form-section>
        <template v-for="v in g.velden" :key="v.naam">
          <nldd-form-field v-if="v.type === 'vink'" label="">
            <Invoer :soort="v.type" :label="v.label" :model-value="waarden[v.naam]" @update:model-value="zet(v.naam, $event)" />
          </nldd-form-field>
          <nldd-form-field v-else :label="v.label" :supporting-label="v.naam !== v.label ? v.naam : undefined">
            <TabelInvoer
              v-if="v.type === 'tabel'"
              :label="v.label"
              :kolommen="v.kolommen ?? []"
              :model-value="waarden[v.naam] ?? []"
              @update:model-value="zet(v.naam, $event)"
            />
            <Invoer
              v-else
              :soort="v.type"
              :label="v.label"
              :keuzes="v.opties"
              :model-value="waarden[v.naam]"
              @update:model-value="zet(v.naam, $event)"
            />
          </nldd-form-field>
        </template>
      </template>
      <template v-if="uitslag">
        <nldd-inline-dialog
          :variant="uitslag.te_beoordelen && uitslag.waarde === true ? 'success' : 'alert'"
          :text="uitslagTekst"
          :supporting-text="uitslagToelichting"
        ></nldd-inline-dialog>
      </template>
      <template v-if="herkomst.length">
        <nldd-table columns="minmax(200px,1fr) 160px minmax(200px,1fr)" accessible-label="Herkomst per parameter">
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
        <template v-for="b in bronnen" :key="b.cel + b.lexostatus">
          <nldd-inline-dialog
            v-if="b.status !== 'bevraagd'"
            variant="alert"
            :text="`Bron ${b.cel}: ${b.status.replace('_', ' ')}`"
            :supporting-text="b.fout"
          ></nldd-inline-dialog>
        </template>
      </template>
      <template v-if="fout">
        <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="fout"></nldd-inline-dialog>
      </template>
      <nldd-form-actions>
        <nldd-button-group>
          <nldd-button
            variant="secondary"
            text="Controleer"
            :loading="bezig === 'toets' || undefined"
            @click="controleer"
          ></nldd-button>
          <nldd-button
            variant="primary"
            type="submit"
            text="Indienen"
            :loading="bezig === 'indienen' || undefined"
          ></nldd-button>
        </nldd-button-group>
      </nldd-form-actions>
    </nldd-form>
  </template>
  <template v-else-if="fout">
    <nldd-inline-dialog variant="alert" text="De stroom is niet te laden" :supporting-text="fout"></nldd-inline-dialog>
  </template>
</template>

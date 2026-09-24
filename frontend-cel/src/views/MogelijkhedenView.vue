<script setup>
// Wat kan de ingelogde persoon hier aanvragen? Niemand somt dat op: het
// proces voert het dienstverleningsbeleid uit voor deze persoon en organisatie, met
// alleen wat de inlog en de andere cellen al weten (GET /api/mogelijkheden).
// Per subsidiejaar geeft het beleid één aanbod: mogelijk, uitgesloten of niet
// te bepalen, met de uiterste indieningsdatum. Per jaar is er een knop die
// alleen bij "mogelijk" actief is, met een (?) die de redenen en de trace
// toont.
import { computed, inject, onMounted, ref } from 'vue';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';

const api = inject('api');
const emit = defineEmits(['aanvragen', 'geladen']);

const data = ref(null);
const fout = ref('');

onMounted(async () => {
  try {
    data.value = await api.mogelijkheden();
    emit('geladen', data.value.mogelijkheden.map((m) => m.mogelijkheid));
  } catch (e) {
    fout.value = e.message;
  }
});

const mogelijkheden = computed(() => (data.value?.mogelijkheden ?? []).map((m) => m.mogelijkheid));

// De datum van de runtime, niet van de browser: "verstreken" hoort bij dezelfde
// klok als het aanbod.
const vandaag = computed(() => data.value?.datum ?? '');

function antwoord(m) {
  if (m.oordeel === 'uitgesloten') return 'Nee';
  if (m.oordeel === 'niet_te_bepalen') return 'Niet te bepalen';
  return 'Ja';
}

function termijnTekst(m) {
  if (m.termijn == null) return 'Onbekend';
  return vandaag.value && m.termijn < vandaag.value ? `${m.termijn} (verstreken)` : m.termijn;
}

function grond(m) {
  if (m.reden) return m.reden;
  if (m.oordeel === 'mogelijk' && m.mist?.length) return `Hangt af van uw aanvraag: ${m.mist.join(', ')}`;
  return `${m.regeling}: ${m.uitkomst}`;
}
</script>

<template>
  <nldd-title size="2">
    <h1>Wat kunt u aanvragen?</h1>
    <span slot="subtitle" v-if="data">KvK {{ data.kvk }}, ingelogd als {{ data.persoon }}</span>
  </nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>Dit volgt uit het dienstverleningsbeleid, voor u en uw organisatie. Het vraagteken naast een knop zegt waarom.</p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="De mogelijkheden zijn niet te bepalen" :supporting-text="fout"></nldd-inline-dialog>
  </template>
  <template v-for="m in mogelijkheden" :key="m.subsidiejaar">
    <nldd-container layout="row" gap="8" vertical-alignment="center">
      <nldd-button
        variant="primary"
        :text="`Aanvraag doen voor ${m.subsidiejaar}`"
        :disabled="m.oordeel !== 'mogelijk' || undefined"
        @click="emit('aanvragen', { subsidiejaar: m.subsidiejaar })"
      ></nldd-button>
      <TraceKnop
        icon="help"
        overline="Waarom"
        :titel="`Aanvraag voor ${m.subsidiejaar}`"
        :accessible-label="`Waarom: aanvraag voor ${m.subsidiejaar}`"
        :trace-text="m.trace_text"
      >
        <nldd-table columns="minmax(180px,1fr) minmax(240px,2fr)" :accessible-label="`Aanvraag voor ${m.subsidiejaar}`">
          <nldd-table-row>
            <nldd-text-cell text="Kunt u deze aanvraag doen?"></nldd-text-cell>
            <nldd-text-cell :text="antwoord(m)"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row>
            <nldd-text-cell text="Waarom"></nldd-text-cell>
            <nldd-text-cell :text="grond(m)"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row>
            <nldd-text-cell text="Indienen vóór"></nldd-text-cell>
            <nldd-text-cell :text="termijnTekst(m)" :supporting-text="m.regeling"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
      </TraceKnop>
    </nldd-container>
    <nldd-spacer size="12"></nldd-spacer>
  </template>
</template>

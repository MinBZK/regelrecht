<script setup>
// Wat kan de ingelogde persoon hier aanvragen? Niemand somt dat op: de cel
// voert het dienstverleningsbeleid uit voor deze persoon en organisatie, met
// alleen wat de inlog en de andere cellen al weten (GET /api/mogelijkheden).
// Per subsidiejaar geeft het beleid één aanbod: mogelijk, uitgesloten of niet
// te bepalen, met de uiterste indieningsdatum. Bij het aanbod staat een
// RR-icoon met de trace.
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

// De datum van de cel, niet van de browser: "verstreken" hoort bij dezelfde
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

function kop(m) {
  if (m.oordeel === 'mogelijk') return `Subsidiejaar ${m.subsidiejaar}: u kunt aanvragen`;
  if (m.oordeel === 'uitgesloten') return `Subsidiejaar ${m.subsidiejaar}: geen aanvraag mogelijk`;
  return `Subsidiejaar ${m.subsidiejaar}: niet te bepalen`;
}
</script>

<template>
  <nldd-title size="2">
    <h1>Wat kunt u aanvragen?</h1>
    <span slot="subtitle" v-if="data">KvK {{ data.kvk }}, ingelogd als {{ data.persoon }}</span>
  </nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>
      Dit volgt uit het dienstverleningsbeleid, uitgevoerd voor u met wat nu bekend is: uw inlog en de registers.
      U vult niets in. Het RR-icoon laat zien hoe de uitkomst tot stand kwam.
    </p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="De mogelijkheden zijn niet te bepalen" :supporting-text="fout"></nldd-inline-dialog>
  </template>
  <template v-for="m in mogelijkheden" :key="m.subsidiejaar">
    <nldd-title size="4"><h2>{{ kop(m) }}</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-table columns="minmax(200px,1fr) minmax(160px,1fr) minmax(200px,2fr) 48px" :accessible-label="kop(m)">
      <nldd-table-row slot="header">
        <nldd-text-cell text="Vraag"></nldd-text-cell>
        <nldd-text-cell text="Antwoord"></nldd-text-cell>
        <nldd-text-cell text="Grond"></nldd-text-cell>
        <nldd-text-cell text=""></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row>
        <nldd-text-cell text="Kunt u deze aanvraag doen?"></nldd-text-cell>
        <nldd-text-cell :text="antwoord(m)"></nldd-text-cell>
        <nldd-text-cell :text="m.reden ?? `${m.regeling}: ${m.uitkomst}`"></nldd-text-cell>
        <nldd-cell>
          <TraceKnop v-if="m.trace_text" :trace-text="m.trace_text" :titel="`Aanbod ${m.subsidiejaar}`" />
        </nldd-cell>
      </nldd-table-row>
      <nldd-table-row>
        <nldd-text-cell text="Uiterste indieningsdatum"></nldd-text-cell>
        <nldd-text-cell :text="termijnTekst(m)"></nldd-text-cell>
        <nldd-text-cell :text="`${m.regeling}`"></nldd-text-cell>
        <nldd-text-cell text=""></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
    <nldd-spacer size="12"></nldd-spacer>
    <nldd-button
      v-if="m.oordeel === 'mogelijk'"
      variant="primary"
      :text="`Aanvraag doen voor ${m.subsidiejaar}`"
      @click="emit('aanvragen', { subsidiejaar: m.subsidiejaar })"
    ></nldd-button>
    <nldd-spacer size="32"></nldd-spacer>
  </template>
</template>

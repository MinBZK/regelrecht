<script setup>
// Wat kan de ingelogde organisatie hier aanvragen? Niemand somt dat op: de
// cel voert de wet uit voor deze organisatie, met alleen wat de eHerkenning
// en de andere cellen al weten (GET /api/mogelijkheden). Per subsidiejaar
// zegt elke toets mogelijk, uitgesloten of niet te bepalen; een toets die
// uitsluit, beslist. Bij elke uitkomst staat een RR-icoon met de trace.
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

const VRAAG = {
  mandaat: 'Mag u namens deze organisatie aanvragen?',
  besluit: 'Kan deze organisatie iets krijgen?',
  termijn: 'Uiterste indieningsdatum',
};

// De datum van de cel, niet van de browser: "verstreken" hoort bij dezelfde
// klok als de toets.
const vandaag = computed(() => data.value?.datum ?? '');

function antwoord(t) {
  if (t.vraag === 'termijn') {
    if (t.waarde == null) return 'Onbekend';
    return vandaag.value && t.waarde < vandaag.value ? `${t.waarde} (verstreken)` : t.waarde;
  }
  if (t.oordeel === 'uitgesloten') return 'Nee';
  if (t.oordeel === 'niet_te_bepalen') return 'Niet te bepalen';
  if (t.waarde === true) return 'Ja';
  if (t.waarde != null && t.waarde !== true) {
    return typeof t.waarde === 'object' ? JSON.stringify(t.waarde) : String(t.waarde);
  }
  return 'Waarschijnlijk: dat hangt af van uw aanvraag';
}

function toelichting(t) {
  if (t.vraag === 'termijn') return `${t.regeling}: ${t.uitkomst}`;
  if (t.reden) return t.reden;
  const delen = [];
  if (t.mist?.length) delen.push(`Hangt af van uw aanvraag: ${t.mist.join(', ')}`);
  if (t.mist_behandeling?.length) delen.push(`Later bij de behandeling: ${t.mist_behandeling.join(', ')}`);
  if (delen.length) return delen.join('. ');
  return `${t.regeling}: ${t.uitkomst}`;
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
      Dit volgt uit de wet, uitgevoerd voor uw organisatie met wat nu al bekend is: uw inlog en de registers
      van andere organisaties. U vult niets in. Het RR-icoon laat zien hoe elke uitkomst tot stand kwam.
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
      <nldd-table-row v-for="t in m.toetsen" :key="t.vraag">
        <nldd-text-cell :text="VRAAG[t.vraag] ?? t.vraag"></nldd-text-cell>
        <nldd-text-cell :text="antwoord(t)"></nldd-text-cell>
        <nldd-text-cell :text="toelichting(t)"></nldd-text-cell>
        <nldd-cell>
          <TraceKnop v-if="t.trace_text" :trace-text="t.trace_text" :titel="`${VRAAG[t.vraag] ?? t.vraag} (${m.subsidiejaar})`" />
        </nldd-cell>
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

<script setup>
// Wat kan de ingelogde persoon hier aanvragen? Niemand somt dat op: het
// proces voert het dienstverleningsbeleid uit voor deze persoon en organisatie, met
// alleen wat de inlog en de andere cellen al weten (GET /api/possibilities):
// alleen voorwaarden die vooraf vaststaan; onbekend is geen aanbod.
// Per tijdvak geeft het beleid één aanbod: mogelijk, uitgesloten of niet te
// bepalen, met de uiterste indieningsdatum. Het tijdvak is de parameter die de
// wet als gevraagde beschikking aanwijst (Awb 4:2 lid 1), met de waarden die
// het proces aanbiedt; de runtime zegt welke parameter en welk veld van de
// aanvraag. Per tijdvak is er een knop die alleen bij "mogelijk" actief is,
// met een (?) die de redenen en de trace toont.
import { computed, inject, onMounted, ref } from 'vue';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';
import { sessieTekst } from '../kanaal.js';

const api = inject('api');
const proces = inject('proces');
const emit = defineEmits(['aanvragen', 'geladen']);

const data = ref(null);
const fout = ref('');

onMounted(async () => {
  try {
    data.value = await api.mogelijkheden();
    emit('geladen', data.value.possibilities.map((m) => m.possibility));
  } catch (e) {
    fout.value = e.message;
  }
});

const mogelijkheden = computed(() => (data.value?.possibilities ?? []).map((m) => m.possibility));

// De datum van de runtime, niet van de browser: "verstreken" hoort bij dezelfde
// klok als het aanbod.
const vandaag = computed(() => data.value?.date ?? '');

function antwoord(m) {
  if (m.verdict === 'excluded') return 'Nee';
  if (m.verdict === 'undeterminable') return 'Niet te bepalen';
  return 'Ja';
}

function termijnTekst(m) {
  if (m.deadline == null) return 'Onbekend';
  return vandaag.value && m.deadline < vandaag.value ? `${m.deadline} (verstreken)` : m.deadline;
}

// Het gekozen tijdvak in woorden, of niets zonder tijdvak.
function tijdvak(m) {
  return m.window ? `${m.window.value}` : '';
}

function titel(m) {
  return m.window ? `Aanvraag voor ${tijdvak(m)}` : 'Aanvraag';
}

// Wat het aanvraagformulier vooraf invult: het veld van het tijdvak.
function vooraf(m) {
  return m.window?.field ? { [m.window.field]: m.window.value } : {};
}

function grond(m) {
  if (m.reason) return m.reason;
  return `${m.regulation}: ${m.output}`;
}
</script>

<template>
  <nldd-title size="2">
    <h1>Wat kunt u aanvragen?</h1>
    <span slot="subtitle" v-if="data">Ingelogd als {{ sessieTekst(proces, data.session) }}</span>
  </nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>Dit volgt uit het dienstverleningsbeleid, voor u en uw organisatie. Het vraagteken naast een knop zegt waarom.</p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="De mogelijkheden zijn niet te bepalen" :supporting-text="fout"></nldd-inline-dialog>
  </template>
  <template v-for="m in mogelijkheden" :key="tijdvak(m)">
    <nldd-container layout="row" gap="8" vertical-alignment="center">
      <nldd-button
        variant="primary"
        :text="m.window ? `Aanvraag doen voor ${tijdvak(m)}` : 'Aanvraag doen'"
        :disabled="m.verdict !== 'possible' || undefined"
        @click="emit('aanvragen', vooraf(m))"
      ></nldd-button>
      <TraceKnop
        icon="help"
        overline="Waarom"
        :titel="titel(m)"
        :accessible-label="`Waarom: ${titel(m).toLowerCase()}`"
        :trace-text="m.trace_text"
      >
        <nldd-table columns="minmax(180px,1fr) minmax(240px,2fr)" :accessible-label="titel(m)">
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
            <nldd-text-cell :text="termijnTekst(m)" :supporting-text="m.regulation"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
      </TraceKnop>
    </nldd-container>
    <nldd-spacer size="12"></nldd-spacer>
  </template>
</template>

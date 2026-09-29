<script setup>
// What can the logged-in person apply for here? Nobody lists that: the
// process executes the service policy for this person and organization, with
// only what the login and the other cells already know
// (GET /api/possibilities): only conditions that are fixed beforehand;
// unknown is no offer. Per window the policy gives one offer: possible,
// excluded or undeterminable, with the last day to submit. The window is the
// parameter the law designates as the requested decision (Awb 4:2 lid 1),
// with the values the process offers; the runtime says which parameter and
// which field of the application. Per window there is a button that is only
// active when "possible", with a (?) that shows the reasons and the trace.
import { computed, inject, onMounted, ref } from 'vue';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';
import { sessionText } from '../channel.js';

const api = inject('api');
const process = inject('process');
const emit = defineEmits(['apply', 'loaded']);

const data = ref(null);
const error = ref('');

onMounted(async () => {
  try {
    data.value = await api.possibilities();
    emit('loaded', data.value.possibilities.map((p) => p.possibility));
  } catch (e) {
    error.value = e.message;
  }
});

const possibilities = computed(() => (data.value?.possibilities ?? []).map((p) => p.possibility));

// The date of the runtime, not of the browser: "expired" belongs to the same
// clock as the offer.
const today = computed(() => data.value?.date ?? '');

function answer(p) {
  if (p.verdict === 'excluded') return 'Nee';
  if (p.verdict === 'undeterminable') return 'Niet te bepalen';
  return 'Ja';
}

function deadlineText(p) {
  if (p.deadline == null) return 'Onbekend';
  return today.value && p.deadline < today.value ? `${p.deadline} (verstreken)` : p.deadline;
}

// The chosen window in words, or nothing without a window.
function windowText(p) {
  return p.window ? `${p.window.value}` : '';
}

function title(p) {
  return p.window ? `Aanvraag voor ${windowText(p)}` : 'Aanvraag';
}

// What the application form fills in beforehand: the field of the window.
function prefilled(p) {
  return p.window?.field ? { [p.window.field]: p.window.value } : {};
}

function ground(p) {
  if (p.reason) return p.reason;
  return `${p.regulation}: ${p.output}`;
}
</script>

<template>
  <nldd-title size="2">
    <h1>Wat kunt u aanvragen?</h1>
    <span slot="subtitle" v-if="data">Ingelogd als {{ sessionText(process, data.session) }}</span>
  </nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>Dit volgt uit het dienstverleningsbeleid, voor u en uw organisatie. Het vraagteken naast een knop zegt waarom.</p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="error">
    <nldd-inline-dialog variant="alert" text="De mogelijkheden zijn niet te bepalen" :supporting-text="error"></nldd-inline-dialog>
  </template>
  <template v-for="p in possibilities" :key="windowText(p)">
    <nldd-container layout="row" gap="8" vertical-alignment="center">
      <nldd-button
        variant="primary"
        :text="p.window ? `Aanvraag doen voor ${windowText(p)}` : 'Aanvraag doen'"
        :disabled="p.verdict !== 'possible' || undefined"
        @click="emit('apply', prefilled(p))"
      ></nldd-button>
      <TraceKnop
        icon="help"
        overline="Waarom"
        :titel="title(p)"
        :accessible-label="`Waarom: ${title(p).toLowerCase()}`"
        :trace-text="p.trace_text"
      >
        <nldd-table columns="minmax(180px,1fr) minmax(240px,2fr)" :accessible-label="title(p)">
          <nldd-table-row>
            <nldd-text-cell text="Kunt u deze aanvraag doen?"></nldd-text-cell>
            <nldd-text-cell :text="answer(p)"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row>
            <nldd-text-cell text="Waarom"></nldd-text-cell>
            <nldd-text-cell :text="ground(p)"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row>
            <nldd-text-cell text="Indienen vóór"></nldd-text-cell>
            <nldd-text-cell :text="deadlineText(p)" :supporting-text="p.regulation"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
      </TraceKnop>
    </nldd-container>
    <nldd-spacer size="12"></nldd-spacer>
  </template>
</template>

<script setup>
// A case: the decisions in it, each with its stages in the procedure
// (RFC-008), the legal protection that follows from them, the state of what
// is still to be paid on that decision, and the actions that act on that
// decision; then what belongs to the case itself (the application, a
// decision that can still be taken, a fact from its course). Which decisions
// there are, which actions, what they ask for and whether they can be taken
// now, the runtime says (from the stage and the law); this page knows no
// decision, publication or payment by name.
import { computed, inject, nextTick, onMounted, ref } from 'vue';
import PaymentStatus from '../components/PaymentStatus.vue';
import Grams from '../components/Grams.vue';
import Action from '../components/Action.vue';
import Actions from '../components/Actions.vue';
import { valueText } from '../text.js';
import { caseLayout, decisionHeading } from '../case.js';

const props = defineProps({
  root: { type: String, required: true },
  // The list the case was opened from, for the back button.
  backLabel: { type: String, default: 'Werkvoorraad' },
});
const emit = defineEmits(['back']);
const api = inject('api');

const caseData = ref(null);
const error = ref('');
const chosen = ref(null);
// The form of the chosen action sits below the tables, usually out of view.
// On open it scrolls into view, also when the same action was already open;
// otherwise the button seems to do nothing.
const actionEl = ref(null);
async function open(name) {
  chosen.value = name;
  await nextTick();
  const calm = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
  actionEl.value?.scrollIntoView({ behavior: calm ? 'auto' : 'smooth', block: 'start' });
}
// A new key after recording: the panel then shows the new state.
const version = ref(0);

async function load() {
  try {
    caseData.value = await api.fetchCase(props.root);
  } catch (e) {
    error.value = e.message;
  }
}

onMounted(load);

const action = computed(() => caseData.value?.actions.find((a) => a.name === chosen.value) ?? null);
const layout = computed(() => caseLayout(caseData.value));

function protectionText(p) {
  return `Stage ${p.stage} loopt na ${p.after}: ${p.description ?? ''} Afgeleid uit de procedure en ${p.legal_basis.join(', ')}.`;
}

function protectionOutputs(p) {
  return Object.entries(p?.outputs ?? {}).map(([name, v]) => ({ name, value: valueText(v) }));
}

async function recorded() {
  await load();
  version.value++;
}
</script>

<template>
  <nldd-button variant="neutral-transparent" start-icon="arrow-left" :text="backLabel" @click="emit('back')"></nldd-button>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-title size="2">
    <h1>Aanvraag {{ root }} en wat erop volgt</h1>
    <span slot="subtitle" v-if="caseData">{{ layout.decisions.length }} besluit(en) in de zaak</span>
  </nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="error">
    <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="error"></nldd-inline-dialog>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="caseData">
    <template v-for="d in layout.decisions" :key="d.id">
      <nldd-title size="3">
        <h2>Besluit {{ d.number }}: {{ d.label }}</h2>
        <span slot="subtitle">{{ decisionHeading(d) }} ({{ d.article }})</span>
      </nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <template v-if="d.procedure">
        <nldd-table columns="minmax(160px,1fr) minmax(280px,2fr) 140px minmax(160px,1fr)" :accessible-label="`Stages van besluit ${d.number}`">
          <nldd-table-row slot="header">
            <nldd-text-cell text="Stage"></nldd-text-cell>
            <nldd-text-cell text="Omschrijving"></nldd-text-cell>
            <nldd-text-cell text="In de zaak"></nldd-text-cell>
            <nldd-text-cell text="Handeling"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="s in d.procedure.stages" :key="s.name">
            <nldd-text-cell :text="s.name"></nldd-text-cell>
            <nldd-text-cell :text="s.description ?? ''"></nldd-text-cell>
            <nldd-text-cell :text="s.recorded ? 'vastgelegd' : ''"></nldd-text-cell>
            <nldd-text-cell :text="s.action ?? ''"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <template v-if="d.legal_protection">
        <nldd-inline-dialog text="Rechtsbescherming" :supporting-text="protectionText(d.legal_protection)"></nldd-inline-dialog>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-table columns="minmax(240px,1fr) minmax(160px,1fr)" :accessible-label="`Rechtsbescherming bij besluit ${d.number}`">
          <nldd-table-row slot="header">
            <nldd-text-cell text="Uitkomst"></nldd-text-cell>
            <nldd-text-cell text="Waarde"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="o in protectionOutputs(d.legal_protection)" :key="o.name">
            <nldd-text-cell :text="o.name"></nldd-text-cell>
            <nldd-text-cell :text="o.value"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <template v-if="d.paymentStatus.length">
        <nldd-title size="4"><h3>Betaalstand</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <PaymentStatus :entries="d.paymentStatus" />
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <Actions :actions="d.ownActions" :label="`Handelingen bij besluit ${d.number}`" @open="open" />
      <nldd-spacer size="24"></nldd-spacer>
    </template>

    <nldd-title size="3">
      <h2>De zaak</h2>
      <span slot="subtitle" v-if="caseData.procedure">Procedure {{ caseData.procedure.id }}; wat bij geen besluit hoort</span>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <template v-if="caseData.procedure">
      <nldd-table columns="minmax(160px,1fr) minmax(280px,2fr) 140px" accessible-label="Stages van de zaak">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Stage"></nldd-text-cell>
          <nldd-text-cell text="Omschrijving"></nldd-text-cell>
          <nldd-text-cell text="In de zaak"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="s in caseData.procedure.stages.filter((s) => s.recorded)" :key="s.name">
          <nldd-text-cell :text="s.name"></nldd-text-cell>
          <nldd-text-cell :text="s.description ?? ''"></nldd-text-cell>
          <nldd-text-cell text="vastgelegd"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <nldd-spacer size="8"></nldd-spacer>
    </template>
    <template v-if="layout.paymentStatus.length">
      <PaymentStatus :entries="layout.paymentStatus" />
      <nldd-spacer size="8"></nldd-spacer>
    </template>
    <Actions v-if="layout.other.length" :actions="layout.other" label="Handelingen in de zaak" @open="open" />

    <template v-if="action">
      <nldd-spacer size="24"></nldd-spacer>
      <div ref="actionEl"></div>
      <Action :key="`${action.name}-${version}`" :root="root" :action="action" @recorded="recorded" />
    </template>

    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="3"><h2>Grammen van de zaak</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <Grams :items="caseData.grams" />
  </template>
</template>

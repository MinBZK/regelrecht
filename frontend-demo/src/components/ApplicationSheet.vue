<script setup>
import { computed, nextTick, reactive, ref, watch } from 'vue';
import DataLineage from './DataLineage.vue';
import { fieldSpec, formatDateTime, formatMissing, formatValue, humanize, verdictOf } from '../data/format.js';
import { lineageFromTrace, leafValues } from '../data/lineage.js';
import { askedInputsFor, claimKeyFor, evaluationParamsFor, inputKind, nextQuestions, parseAnswer } from '../data/askedInputs.js';
import { useDemo } from '../store/demoStore.js';
import { awbOutcomes, objectionOpen, statusOf } from '../data/lifecycle.js';
import { driftRows, driftSentence } from '../data/caseDrift.js';

// The citizen's side of an application, inside the portal. The flow the POC
// generated per regeling: first the questions only the citizen can answer
// (rent, service costs), then the outcome and the data it rests on to check,
// then the declaration and the submission, and afterwards the status of the
// application with, once decided, the possibility to object. The caseworker's
// side lives in the Zaaksysteem tab; nothing here jumps there.

const props = defineProps({
  open: { type: Boolean, default: false },
  law: { type: Object, default: null },
  /** The tile's latest evaluation of this law (re-evaluated as data changes). */
  evaluation: { type: Object, default: null },
});
// `edit-value` carries `{ node, law }`, the shape LawTile emits, so the portal
// opens the same correction sheet for a value corrected from inside the application.
const emit = defineEmits(['close', 'edit-value']);
const demo = useDemo();
const { corpus, profile, personaParams, claimFor, findCase, caseDrift, dataVersion } = demo;

const sheet = ref(null);
const step = ref('gegevens'); // gegevens | controleren | status
const answers = reactive({});
const declared = ref(false);
const error = ref('');
const objectionReason = ref('');
const justSubmitted = ref(false);

const service = computed(() => (props.law ? corpus.value?.services?.[props.law.service]?.name ?? props.law.service : ''));
const asked = computed(() => {
  void dataVersion.value;
  return props.law && corpus.value ? askedInputsFor(corpus.value, props.law, claimFor) : [];
});
// Just in time, as the POC did it: the engine runs with what it has, and the
// next question is the first thing it ran into that only the citizen knows.
const missing = computed(() => (props.law ? nextQuestions(asked.value, props.evaluation, props.law.id) : []));
const question = computed(() => missing.value[0] ?? null);
const answered = ref(0);
const currentCase = computed(() => {
  void dataVersion.value;
  return props.law ? findCase(props.law) : null;
});
const doc = computed(() => props.law?.doc);

// true/false when the law decided, 'unknown' when it could not for lack of
// facts (RFC-036): then there is nothing to submit yet. A law without a
// verdict output is computed for everyone.
const verdict = computed(() => {
  if (!props.evaluation?.ok) return null;
  const v = verdictOf(props.evaluation.outputs);
  return v === null ? true : v;
});
const requirementsMet = computed(() => verdict.value === true);
const lawName = (id) => corpus.value?.lawById(id)?.name ?? id;
const verdictMissing = computed(() => (verdict.value === 'unknown' ? formatMissing(props.evaluation.outputs.voldoet_aan_voorwaarden, { ownLaw: props.law?.id, lawName }) : ''));
const primaryName = computed(() => corpus.value?.config?.dashboard_outputs?.[`${props.law?.service}/${props.law?.law_path}`] ?? null);
const outcomeRows = computed(() => {
  const o = props.evaluation?.ok ? props.evaluation.outputs : {};
  const rows = Object.entries(o).filter(([k]) => k !== 'voldoet_aan_voorwaarden');
  rows.sort(([a], [b]) => (a === primaryName.value ? -1 : b === primaryName.value ? 1 : 0));
  return rows.slice(0, 6);
});
// The same tree the tile shows under "Gebruikte gegevens": register values,
// and under each law that computed a value the values that law used in turn.
const lineage = computed(() => {
  if (!props.evaluation?.trace || !props.law) return [];
  return lineageFromTrace(props.evaluation.trace, props.law.id, evaluationParamsFor(personaParams(), asked.value));
});
const usedCount = computed(() => leafValues(lineage.value).length);

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      sheet.value?.hide?.();
      return;
    }
    error.value = '';
    declared.value = false;
    objectionReason.value = '';
    justSubmitted.value = false;
    for (const k of Object.keys(answers)) delete answers[k];
    answered.value = 0;
    step.value = currentCase.value ? 'status' : missing.value.length ? 'gegevens' : 'controleren';
    await nextTick();
    sheet.value?.show?.();
  },
  { immediate: true },
);

// ---- step 1: the questions ----------------------------------------------------
function kindOf(a) {
  if (Array.isArray(a.spec?.type_spec?.enum)) return 'enum';
  return inputKind(a.spec, a.claim?.newValue ?? null);
}
function enumOptions(a) {
  const labels = a.spec?.type_spec?.enum_labels ?? {};
  return (a.spec?.type_spec?.enum ?? []).map((v) => ({ value: String(v), label: labels[v] ?? labels[String(v)] ?? String(v) }));
}
function placeholderFor(a) {
  const k = kindOf(a);
  return k === 'amount' ? 'bijvoorbeeld 650,00' : k === 'date' ? 'JJJJ-MM-DD' : k === 'number' ? 'getal' : '';
}
function labelFor(a) {
  const k = kindOf(a);
  return k === 'amount' ? `${humanize(a.name)} (in euro)` : humanize(a.name);
}
function setAnswer(a, e) {
  answers[a.name] = e.detail?.value ?? e.target?.value ?? '';
}
const canContinue = computed(() => !!question.value && parseAnswer(kindOf(question.value), answers[question.value.name]) !== undefined);

// One answer; the engine recomputes (the tile re-evaluates on the new claim)
// and `missing` tells whether it ran into another question.
function submitAnswer() {
  error.value = '';
  const a = question.value;
  if (!a) return;
  const value = parseAnswer(kindOf(a), answers[a.name]);
  if (value === undefined) {
    error.value = `Vul ${humanize(a.name).toLowerCase()} in.`;
    return;
  }
  demo.submitClaim({
    lawId: props.law.id,
    tileLawId: props.law.id,
    input: a.name,
    ...claimKeyFor(props.law, personaParams()),
    oldValue: null,
    newValue: value,
    reason: 'Opgegeven bij de aanvraag.',
    selfDeclared: true,
  });
  answered.value += 1;
}
// When the recomputation has no further question, move on by itself.
watch([missing, step], ([m, st]) => {
  if (st === 'gegevens' && m.length === 0 && answered.value > 0) step.value = 'controleren';
});

/**
 * Het invoerveld krijgt de aandacht, bij elke volgende vraag opnieuw.
 *
 * De wet stelt haar vragen één voor één (just in time), en na een antwoord
 * staat de volgende vraag er met een leeg veld. Zonder dit moet je er elke keer
 * eerst heen klikken; met dit kun je een aanvraag al typend en enterend
 * afmaken, wat in een demo voor de zaal het verschil is tussen vertellen en
 * laten zien. Enter zit op het omhullende veld, zodat elk soort vraag het erft.
 *
 * Welk veld dat is, hangt van de vraag af (tekst, datum, keuzelijst). Ze
 * melden zich alle drie als invoerveld met `static isFormInput = true` — de
 * afspraak waarop `nldd-form-field` zelf ook zijn label laat wijzen — en
 * hebben een eigen `focus()` die het schaduw-DOM afhandelt. Dat is stabieler
 * dan hier een lijst met tagnamen bijhouden.
 */
const questionField = ref(null);
function isFormInput(el) {
  return el instanceof HTMLInputElement || el instanceof HTMLSelectElement || el.constructor?.isFormInput === true;
}
watch(
  [question, () => props.open],
  async ([q, open]) => {
    if (!q || !open) return;
    await nextTick();
    const control = [...(questionField.value?.querySelectorAll?.('*') ?? [])].find(isFormInput);
    control?.focus?.();
  },
  { immediate: true },
);

// ---- step 2/3: check and submit --------------------------------------------------
const canSubmit = computed(() => props.evaluation?.ok && verdict.value === true && declared.value && missing.value.length === 0);
function submitApplication() {
  const c = demo.submitCase(props.law, props.evaluation, evaluationParamsFor(personaParams(), asked.value));
  if (c) {
    justSubmitted.value = true;
    step.value = 'status';
  }
}

// ---- status and objection --------------------------------------------------------
// `variant` decides the banner's colour AND its default icon; there is no
// `info` variant (neutral | accent | success | warning | critical), so the two
// statuses that used it rendered without an icon at all — an empty column to
// the left of the text. `accent` is the neutral-but-noticed one. The icons
// mirror the tile's status tag, so the same case looks the same in both.
const statusView = computed(() => {
  const c = currentCase.value;
  if (!c) return null;
  if (c.objection?.status === 'PENDING') return { variant: 'warning', icon: 'flag', text: 'Bezwaar ingediend', supporting: 'De gemeente of dienst beoordeelt uw bezwaar.' };
  // Uit de fase, niet uit het opgeslagen veld (zie lifecycle.js).
  const status = statusOf(c);
  if (status === 'DECIDED') {
    if (c.objection) return c.approved ? { variant: 'success', icon: 'check-mark-circle', text: 'Toegekend na bezwaar', supporting: c.reason } : { variant: 'critical', icon: 'dismiss-circle', text: 'Afgewezen, bezwaar ongegrond', supporting: c.reason };
    return c.approved ? { variant: 'success', icon: 'check-mark-circle', text: 'Toegekend', supporting: c.reason } : { variant: 'critical', icon: 'dismiss-circle', text: 'Afgewezen', supporting: c.reason };
  }
  if (status === 'IN_REVIEW') return { variant: 'accent', icon: 'clock', text: 'In behandeling', supporting: 'Een behandelaar beoordeelt uw aanvraag. U ontvangt bericht.' };
  return { variant: 'accent', icon: 'paper-plane', text: 'Ingediend', supporting: 'Uw aanvraag is ontvangen.' };
});
const citizenEvents = computed(() =>
  (currentCase.value?.events ?? []).map((e) => ({
    at: e.at,
    text: e.type === 'SUBMITTED' ? 'U heeft de aanvraag ingediend.'
      : e.type === 'IN_REVIEW' ? 'Uw aanvraag wordt door een behandelaar beoordeeld.'
      : e.type === 'DECIDED' ? (e.text.startsWith('Toegekend') || e.text.startsWith('Automatisch toegekend') ? 'Uw aanvraag is toegekend.' : 'Uw aanvraag is afgewezen.')
      : e.type === 'OBJECTION' ? 'U heeft bezwaar gemaakt.'
      : e.type === 'OBJECTION_DECIDED' ? (e.text.includes('gegrond:') && !e.text.includes('ongegrond') ? 'Uw bezwaar is gegrond verklaard.' : 'Uw bezwaar is ongegrond verklaard.')
      : e.text,
  })),
);
// Bezwaar kan pas als de bezwaartermijn loopt, en die begint de dag ná de
// bekendmaking (Awb 6:8). Zolang het besluit nog niet is bekendgemaakt weet de
// burger van niets en is er niets om bezwaar tegen te maken.
const canObject = computed(() => objectionOpen(currentCase.value));
/** Wat de Awb aan dit besluit heeft toegevoegd: de termijn, de einddatum. */
const awb = computed(() => awbOutcomes(currentCase.value));

/**
 * De aanvraag die er ligt, tegen wat de wet nu zegt. Zelfde vergelijking als
 * op de tegel; hier staat de knop waarmee de burger er iets aan kan doen.
 */
const drift = computed(() => {
  void dataVersion.value;
  return props.law ? caseDrift(props.law, props.evaluation) : null;
});
const rows = computed(() => driftRows(drift.value, doc.value));
const driftText = computed(() => driftSentence(drift.value, doc.value));
function resubmit() {
  demo.resubmitCase(currentCase.value.id, props.evaluation, evaluationParamsFor(personaParams(), asked.value));
}
function fileObjection() {
  demo.objectToCase(currentCase.value.id, objectionReason.value.trim() || 'Ik ben het niet eens met het besluit.');
  objectionReason.value = '';
}
const claimedPrimary = computed(() => {
  const c = currentCase.value;
  if (!c) return null;
  const name = primaryName.value ?? Object.keys(c.claimedResult ?? {}).find((k) => typeof c.claimedResult[k] === 'number');
  return name ? { name, value: c.claimedResult?.[name] } : null;
});
// The corrections on this case, the citizen's own and the caseworker's: the
// same list the caseworker reads in the Zaaksysteem, so both sides see one thing.
const caseClaims = computed(() => {
  const c = currentCase.value;
  if (!c) return [];
  return demo.state.claims.filter((cl) => cl.caseId === c.id || (cl.bsn === c.bsn && cl.tileLawId === c.lawId));
});
function claimSpec(cl) {
  return fieldSpec(corpus.value?.lawById(cl.lawId)?.doc, cl.input);
}
function claimStatus(cl) {
  if (cl.status === 'APPROVED') return { color: 'success', text: cl.claimant === 'BEHANDELAAR' ? 'Doorgevoerd' : 'Goedgekeurd' };
  if (cl.status === 'REJECTED') return { color: 'critical', text: 'Afgewezen' };
  return { color: 'neutral', text: 'In beoordeling' };
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheet" placement="right" width="560px" accessible-label="Aanvraag" @close="emit('close')">
      <nldd-page v-if="law">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="law.name" :supporting-text="service" dismiss-text="Sluiten" @dismiss="emit('close')"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="16">
          <nldd-step-indicator v-if="step !== 'status'" :current="step === 'gegevens' ? 1 : 2" accessible-label="Stappen van de aanvraag">
            <nldd-step-indicator-item text="Gegevens"></nldd-step-indicator-item>
            <nldd-step-indicator-item text="Controleren en indienen"></nldd-step-indicator-item>
          </nldd-step-indicator>

          <!-- Step 1: the questions only the citizen can answer -->
          <template v-if="step === 'gegevens'">
            <nldd-rich-text spacing="tight">
              <p v-if="answered === 0">De wet is voor u doorgerekend met wat de overheid al weet. Eén gegeven staat in geen register; dat kunt alleen u opgeven.</p>
              <p v-else>Dank u. De wet is opnieuw doorgerekend en loopt tegen nog een gegeven aan dat alleen u weet.</p>
            </nldd-rich-text>
            <template v-if="question">
              <!-- Enter op het veld is Verder, wat voor soort vraag het ook is:
                   een demo loopt zo van vraag naar vraag zonder de muis. Op het
                   omhullende veld, zodat elk invoertype het erft. -->
              <nldd-form-field ref="questionField" :label="labelFor(question)" @keydown.enter="canContinue && submitAnswer()">
                <nldd-dropdown v-if="kindOf(question) === 'enum'" width="full">
                  <select :value="answers[question.name] ?? ''" @change="setAnswer(question, $event)">
                    <option value="" disabled>Maak een keuze</option>
                    <option v-for="o in enumOptions(question)" :key="o.value" :value="o.value">{{ o.label }}</option>
                  </select>
                </nldd-dropdown>
                <nldd-dropdown v-else-if="kindOf(question) === 'boolean'" width="full">
                  <select :value="answers[question.name] ?? ''" @change="setAnswer(question, $event)">
                    <option value="" disabled>Maak een keuze</option>
                    <option value="true">Ja</option>
                    <option value="false">Nee</option>
                  </select>
                </nldd-dropdown>
                <nldd-date-field v-else-if="kindOf(question) === 'date'" :value="answers[question.name] ?? ''" width="full" @change="setAnswer(question, $event)"></nldd-date-field>
                <!-- An unanswered amount is empty, not 0: nldd-number-field has no empty state (it starts at 0 and
                     an emptied field falls back to the last value), so the question is a text field with a numeric
                     keyboard; parseAnswer reads the Dutch notation. -->
                <nldd-text-field v-else-if="kindOf(question) === 'amount' || kindOf(question) === 'number'" :value="answers[question.name] ?? ''" width="full" :keyboard="kindOf(question) === 'amount' ? 'decimal' : 'numeric'" :placeholder="placeholderFor(question)" @input="setAnswer(question, $event)"></nldd-text-field>
                <nldd-text-field v-else :value="answers[question.name] ?? ''" width="full" :placeholder="placeholderFor(question)" @input="setAnswer(question, $event)"></nldd-text-field>
                <nldd-form-field-help-text v-if="question.spec?.description">{{ question.spec.description }}</nldd-form-field-help-text>
              </nldd-form-field>
              <nldd-banner v-if="error" variant="critical" :text="error"></nldd-banner>
              <nldd-form-actions>
                <nldd-button variant="primary" text="Verder" :disabled="!canContinue || undefined" @click="submitAnswer"></nldd-button>
              </nldd-form-actions>
            </template>
            <template v-else>
              <nldd-activity-indicator timing="instant" size="24"></nldd-activity-indicator>
              <nldd-rich-text spacing="tight"><p>De wet wordt opnieuw doorgerekend…</p></nldd-rich-text>
            </template>
            <nldd-list v-if="asked.some((a) => a.claim)" variant="simple" accessible-label="Door u opgegeven">
              <nldd-list-item v-for="a in asked.filter((x) => x.claim)" :key="a.name" size="sm">
                <nldd-icon-cell icon="checked" size="16" color="success"></nldd-icon-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell size="sm" :text="humanize(a.name)" supporting-text="door u opgegeven"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(a.claim.newValue, a.spec)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <!-- Step 2: check the outcome and the data, declare, submit -->
          <template v-else-if="step === 'controleren'">
            <template v-if="!evaluation?.ok">
              <nldd-activity-indicator timing="instant" size="24"></nldd-activity-indicator>
              <nldd-rich-text spacing="tight"><p>De regeling wordt met uw gegevens berekend…</p></nldd-rich-text>
            </template>
            <template v-else>
              <nldd-banner v-if="verdict === 'unknown'" variant="accent" text="De wet kan nog geen uitkomst geven" :supporting-text="`Er ${verdictMissing}. Zonder deze gegevens kan de aanvraag niet worden beoordeeld.`"></nldd-banner>
              <nldd-list v-else variant="box-tinted" accessible-label="Uitkomst">
                <nldd-list-item size="md">
                  <nldd-icon-cell :icon="requirementsMet ? 'check-mark-circle' : 'dismiss-circle'" :color="requirementsMet ? 'success' : 'critical'"></nldd-icon-cell>
                  <nldd-spacer-cell size="12"></nldd-spacer-cell>
                  <nldd-title-cell size="4" :overline="requirementsMet ? 'U voldoet aan de voorwaarden' : 'U voldoet niet aan de voorwaarden'" :text="requirementsMet && outcomeRows[0] ? formatValue(outcomeRows[0][1], fieldSpec(doc, outcomeRows[0][0])) : requirementsMet ? 'Ja' : 'Aanvragen heeft geen zin'" :supporting-text="requirementsMet && outcomeRows[0] ? humanize(outcomeRows[0][0]) : ''"></nldd-title-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-list v-if="outcomeRows.length > 1" variant="simple" accessible-label="Overige uitkomsten">
                <nldd-list-item v-for="[name, value] in outcomeRows.slice(1)" :key="name" size="sm">
                  <nldd-text-cell size="sm" color="secondary" :text="humanize(name)"></nldd-text-cell>
                  <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(value, fieldSpec(doc, name))"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>

              <nldd-title size="5">
                <h3>Gegevens waarop de berekening rust</h3>
                <span slot="subtitle">{{ usedCount }} gegevens uit registers en uw eigen opgave</span>
              </nldd-title>
              <nldd-list type="tree" variant="box-tinted" accessible-label="Gebruikte gegevens">
                <DataLineage :nodes="lineage" @edit="emit('edit-value', { node: $event, law })" />
              </nldd-list>
              <nldd-rich-text spacing="tight"><p><small>Klik op een gegeven om het te corrigeren; een behandelaar beoordeelt de correctie.</small></p></nldd-rich-text>

              <template v-if="requirementsMet">
                <nldd-checkbox-field label="Ik verklaar dat de door mij opgegeven gegevens juist en volledig zijn." :checked="declared || undefined" @change="declared = !!($event.detail?.checked ?? $event.target?.checked)"></nldd-checkbox-field>
                <nldd-form-actions>
                  <nldd-button variant="primary" start-icon="paper-plane" text="Aanvraag indienen" :disabled="!canSubmit || undefined" @click="submitApplication"></nldd-button>
                </nldd-form-actions>
              </template>
              <nldd-banner v-else-if="verdict === false" variant="warning" text="U voldoet niet aan de voorwaarden" supporting-text="U kunt wel aanvragen, maar de wet wijst de aanvraag af. Controleer eerst of alle gegevens kloppen."></nldd-banner>
            </template>
          </template>

          <!-- Status of the application -->
          <template v-else-if="currentCase">
            <nldd-banner :variant="statusView.variant" :icon="statusView.icon" :text="statusView.text" :supporting-text="statusView.supporting"></nldd-banner>
            <nldd-rich-text v-if="justSubmitted" spacing="tight"><p>Uw aanvraag is ingediend bij {{ service }}. U kunt de voortgang hier volgen.</p></nldd-rich-text>

            <!-- Wat er ligt klopt niet meer met wat de wet nu zegt. De demo
                 rekent niets opnieuw af achter de rug van de burger om: het
                 besluit blijft staan en hij krijgt te zien wat er veranderd is,
                 met de weg terug ernaast. Aanvraag per aanvraag, want elke
                 aanvraag is een eigen besluit. -->
            <template v-if="drift && !justSubmitted">
              <nldd-banner variant="warning" text="Uw aanvraag klopt niet meer" :supporting-text="driftText"></nldd-banner>
              <nldd-list variant="box-tinted" accessible-label="Verschil met uw eerdere aanvraag">
                <nldd-list-item v-for="row in rows" :key="row.name" size="sm">
                  <nldd-text-cell size="sm" color="secondary" min-width="50%" :text="humanize(row.name)"></nldd-text-cell>
                  <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="`${row.was} → ${row.now}`"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-form-actions>
                <nldd-button variant="primary" start-icon="paper-plane" text="Aanvraag wijzigen" @click="resubmit"></nldd-button>
              </nldd-form-actions>
            </template>
            <nldd-list v-if="claimedPrimary" variant="box-tinted" accessible-label="Aangevraagd">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" :text="`Aangevraagd · ${humanize(claimedPrimary.name)}`"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(claimedPrimary.value, fieldSpec(doc, claimedPrimary.name))"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" text="Ingediend op"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatDateTime(currentCase.submittedAt)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <template v-if="caseClaims.length">
              <nldd-title size="5"><h3>Correcties op uw gegevens</h3></nldd-title>
              <nldd-list variant="box-tinted" accessible-label="Correcties op uw gegevens">
                <nldd-list-item v-for="cl in caseClaims" :key="cl.id" size="sm">
                  <nldd-text-cell size="sm" :text="`${humanize(cl.input)}: ${formatValue(cl.oldValue, claimSpec(cl))} → **${formatValue(cl.newValue, claimSpec(cl))}**`">
                    <span slot="supporting-text">
                      {{ cl.claimant === 'BEHANDELAAR' ? 'door behandelaar' : 'door u' }} · {{ cl.reason }}
                      <template v-if="cl.hardship?.clause"><br />Beroep op hardheidsclausule: {{ cl.hardship.clause }}</template>
                      <template v-if="cl.evidence"><br />Bewijsstuk: {{ cl.evidence.name }}</template>
                    </span>
                  </nldd-text-cell>
                  <nldd-cell v-if="cl.hardship?.clause"><nldd-tag size="sm" color="warning" text="Hardheidsclausule"></nldd-tag></nldd-cell>
                  <!-- Twee losse labels naast elkaar: zonder tussenruimte lezen ze als een. -->
                  <nldd-spacer-cell v-if="cl.hardship?.clause" size="8"></nldd-spacer-cell>
                  <nldd-cell><nldd-tag size="sm" :color="claimStatus(cl).color" :text="claimStatus(cl).text"></nldd-tag></nldd-cell>
                </nldd-list-item>
              </nldd-list>
            </template>
            <nldd-title size="5"><h3>Verloop</h3></nldd-title>
            <nldd-list variant="simple" accessible-label="Verloop van de aanvraag">
              <nldd-list-item v-for="(e, i) in citizenEvents" :key="i" size="sm">
                <nldd-text-cell size="sm" :text="e.text" :supporting-text="formatDateTime(e.at)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <!-- De termijn komt uit de wet en niet uit dit scherm: artikel 6:7
                 Awb geeft de zes weken, artikel 6:8 rekent de einddatum uit
                 vanaf de bekendmaking, en een bijzondere wet die daarvan
                 afwijkt is er al in verwerkt. Daarom staat hier een datum en
                 geen vaste tekst. -->
            <nldd-list v-if="awb.bezwaartermijnEinde" variant="box-tinted" accessible-label="Bezwaartermijn">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" text="U kunt bezwaar maken tot en met"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(awb.bezwaartermijnEinde, null)"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item v-if="awb.bezwaartermijnWeken" size="sm">
                <nldd-text-cell size="sm" color="secondary" text="Termijn volgens de wet"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="`${awb.bezwaartermijnWeken} weken`"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <template v-if="canObject">
              <nldd-form-field label="Niet mee eens? Maak bezwaar" optional>
                <nldd-multi-line-text-field :value="objectionReason" rows="3" placeholder="Waarom bent u het niet eens met het besluit? (Awb art. 6:5)" @input="objectionReason = $event.detail?.value ?? $event.target.value"></nldd-multi-line-text-field>
              </nldd-form-field>
              <nldd-form-actions>
                <nldd-button variant="secondary" start-icon="flag" text="Bezwaar indienen" @click="fileObjection"></nldd-button>
              </nldd-form-actions>
            </template>
            <!-- Besloten, maar nog niet de deur uit. Eerlijk benoemen dat de
                 termijn nog niet loopt is beter dan een knop die er wel staat
                 maar niets betekent. -->
            <nldd-banner
              v-else-if="currentCase && !currentCase.publishedAt && currentCase.decidedAt"
              variant="accent"
              text="Het besluit is nog niet bekendgemaakt"
              supporting-text="Zodra u het besluit ontvangt, begint de bezwaartermijn te lopen (Awb art. 6:8)."
            ></nldd-banner>
          </template>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<script setup>
import { computed, nextTick, reactive, ref, watch } from 'vue';
import OrgLogo from './OrgLogo.vue';
import { fieldSpec, formatDateTime, formatValue, humanize } from '../data/format.js';
import { lineageFromTrace, leafValues } from '../data/lineage.js';
import { askedInputsFor, claimKeyFor, evaluationParamsFor, inputKind, nextQuestions, parseAnswer } from '../data/askedInputs.js';
import { useDemo } from '../store/demoStore.js';

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
const emit = defineEmits(['close']);
const demo = useDemo();
const { corpus, profile, personaParams, claimFor, findCase, dataVersion } = demo;

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
const missing = computed(() => (props.law ? nextQuestions(asked.value, props.evaluation, props.law.id, evaluationParamsFor(personaParams(), asked.value)) : []));
const question = computed(() => missing.value[0] ?? null);
const answered = ref(0);
const currentCase = computed(() => {
  void dataVersion.value;
  return props.law ? findCase(props.law) : null;
});
const doc = computed(() => props.law?.doc);

const requirementsMet = computed(() => {
  if (!props.evaluation?.ok) return null;
  const v = props.evaluation.outputs?.voldoet_aan_voorwaarden;
  return v === undefined ? true : !!v;
});
const primaryName = computed(() => corpus.value?.config?.dashboard_outputs?.[`${props.law?.service}/${props.law?.law_path}`] ?? null);
const outcomeRows = computed(() => {
  const o = props.evaluation?.ok ? props.evaluation.outputs : {};
  const rows = Object.entries(o).filter(([k]) => k !== 'voldoet_aan_voorwaarden');
  rows.sort(([a], [b]) => (a === primaryName.value ? -1 : b === primaryName.value ? 1 : 0));
  return rows.slice(0, 6);
});
const usedValues = computed(() => {
  if (!props.evaluation?.trace || !props.law) return [];
  return leafValues(lineageFromTrace(props.evaluation.trace, props.law.id, evaluationParamsFor(personaParams(), asked.value)));
});
function lawName(id) {
  return corpus.value?.lawById(id)?.name ?? id;
}
function valueSpec(node) {
  return fieldSpec(corpus.value?.lawById(node.law)?.doc, node.name);
}

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

// ---- step 2/3: check and submit --------------------------------------------------
const canSubmit = computed(() => props.evaluation?.ok && requirementsMet.value && declared.value && missing.value.length === 0);
function submitApplication() {
  const c = demo.submitCase(props.law, props.evaluation, evaluationParamsFor(personaParams(), asked.value));
  if (c) {
    justSubmitted.value = true;
    step.value = 'status';
  }
}

// ---- status and objection --------------------------------------------------------
const statusView = computed(() => {
  const c = currentCase.value;
  if (!c) return null;
  if (c.objection?.status === 'PENDING') return { variant: 'warning', text: 'Bezwaar ingediend', supporting: 'De gemeente of dienst beoordeelt uw bezwaar.' };
  if (c.status === 'DECIDED') {
    if (c.objection) return c.approved ? { variant: 'success', text: 'Toegekend na bezwaar', supporting: c.reason } : { variant: 'critical', text: 'Afgewezen, bezwaar ongegrond', supporting: c.reason };
    return c.approved ? { variant: 'success', text: 'Toegekend', supporting: c.reason } : { variant: 'critical', text: 'Afgewezen', supporting: c.reason };
  }
  if (c.status === 'IN_REVIEW') return { variant: 'info', text: 'In behandeling', supporting: 'Een behandelaar beoordeelt uw aanvraag. U ontvangt bericht.' };
  return { variant: 'info', text: 'Ingediend', supporting: 'Uw aanvraag is ontvangen.' };
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
const canObject = computed(() => currentCase.value?.status === 'DECIDED' && !currentCase.value?.objection);
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
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheet" placement="right" width="560px" accessible-label="Aanvraag" @close="emit('close')">
      <nldd-page v-if="law">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="law.name" :supporting-text="service" dismiss-text="Sluiten" @dismiss="emit('close')"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="16">
          <nldd-container v-if="step !== 'status'" layout="row" gap="8" vertical-alignment="center">
            <nldd-tag size="sm" :color="step === 'gegevens' ? 'accent' : 'neutral'" text="1 Gegevens"></nldd-tag>
            <nldd-tag size="sm" :color="step === 'controleren' ? 'accent' : 'neutral'" text="2 Controleren en indienen"></nldd-tag>
          </nldd-container>

          <!-- Step 1: the questions only the citizen can answer -->
          <template v-if="step === 'gegevens'">
            <nldd-rich-text spacing="tight">
              <p v-if="answered === 0">De wet is voor u doorgerekend met wat de overheid al weet. Eén gegeven staat in geen register; dat kunt alleen u opgeven.</p>
              <p v-else>Dank u. De wet is opnieuw doorgerekend en loopt tegen nog een gegeven aan dat alleen u weet.</p>
            </nldd-rich-text>
            <template v-if="question">
              <nldd-form-field :label="labelFor(question)">
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
                <nldd-number-field v-else-if="kindOf(question) === 'amount' || kindOf(question) === 'number'" :value="answers[question.name] ?? ''" :step="kindOf(question) === 'amount' ? '0.01' : '1'" width="full" hide-spin-buttons :placeholder="placeholderFor(question)" @input="setAnswer(question, $event)" @change="setAnswer(question, $event)" @keydown.enter="submitAnswer"></nldd-number-field>
                <nldd-text-field v-else :value="answers[question.name] ?? ''" width="full" :placeholder="placeholderFor(question)" @input="setAnswer(question, $event)" @keydown.enter="submitAnswer"></nldd-text-field>
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
              <nldd-list variant="box" background="tinted" accessible-label="Uitkomst">
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
                <span slot="subtitle">{{ usedValues.length }} gegevens uit registers en uw eigen opgave</span>
              </nldd-title>
              <nldd-list variant="box" accessible-label="Gebruikte gegevens">
                <nldd-list-item v-for="node in usedValues" :key="`${node.law}|${node.name}`" size="sm">
                  <nldd-cell v-if="node.service"><OrgLogo :service="node.service" size="sm" /></nldd-cell>
                  <nldd-icon-cell v-else icon="edit" size="16" color="accent"></nldd-icon-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-text-cell size="sm" :text="humanize(node.name)" :supporting-text="node.service ? corpus.services[node.service]?.name ?? node.service : 'door u opgegeven'"></nldd-text-cell>
                  <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(node.value, valueSpec(node))"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-rich-text spacing="tight"><p><small>Klopt een gegeven niet? Sluit dit venster en corrigeer het onder "Gebruikte gegevens" op de tegel; een behandelaar beoordeelt de correctie.</small></p></nldd-rich-text>

              <template v-if="requirementsMet">
                <nldd-checkbox-field label="Ik verklaar dat de door mij opgegeven gegevens juist en volledig zijn." :checked="declared || undefined" @change="declared = !!($event.detail?.checked ?? $event.target?.checked)"></nldd-checkbox-field>
                <nldd-form-actions>
                  <nldd-button variant="primary" start-icon="paper-plane" text="Aanvraag indienen" :disabled="!canSubmit || undefined" @click="submitApplication"></nldd-button>
                </nldd-form-actions>
              </template>
              <nldd-banner v-else variant="warning" text="U voldoet niet aan de voorwaarden" supporting-text="U kunt wel aanvragen, maar de wet wijst de aanvraag af. Controleer eerst of alle gegevens kloppen."></nldd-banner>
            </template>
          </template>

          <!-- Status of the application -->
          <template v-else-if="currentCase">
            <nldd-banner :variant="statusView.variant" :text="statusView.text" :supporting-text="statusView.supporting"></nldd-banner>
            <nldd-rich-text v-if="justSubmitted" spacing="tight"><p>Uw aanvraag is ingediend bij {{ service }}. U kunt de voortgang hier volgen.</p></nldd-rich-text>
            <nldd-list v-if="claimedPrimary" variant="box" accessible-label="Aangevraagd">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" :text="`Aangevraagd · ${humanize(claimedPrimary.name)}`"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(claimedPrimary.value, fieldSpec(doc, claimedPrimary.name))"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" text="Ingediend op"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatDateTime(currentCase.submittedAt)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-title size="5"><h3>Verloop</h3></nldd-title>
            <nldd-list variant="simple" accessible-label="Verloop van de aanvraag">
              <nldd-list-item v-for="(e, i) in citizenEvents" :key="i" size="sm">
                <nldd-text-cell size="sm" :text="e.text" :supporting-text="formatDateTime(e.at)"></nldd-text-cell>
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
          </template>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

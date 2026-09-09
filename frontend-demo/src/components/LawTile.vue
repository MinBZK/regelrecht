<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import OrgLogo from './OrgLogo.vue';
import DataLineage from './DataLineage.vue';
import { fieldSpec, formatMissing, formatValue, humanize, isUnknown, verdictOf } from '../data/format.js';
import { lineageFromTrace, leafValues } from '../data/lineage.js';
import { askedInputsFor, claimKeyFor, evaluationParamsFor, nextQuestions } from '../data/askedInputs.js';
import { useDemo } from '../store/demoStore.js';

// One regeling on the portal: the outcome of the law for this persona, the
// values it used (expandable, each correctable), the application button and
// the state of a submitted case. Re-evaluates whenever registered data changes.

const props = defineProps({
  law: { type: Object, required: true },
});
const emit = defineEmits(['edit-value', 'evaluated', 'apply']);
const router = useRouter();
const demo = useDemo();
const { corpus, dataVersion, profile, personaParams, findCase } = demo;

const evaluation = ref(null);
const showData = ref(false);
const traceSheet = ref(null);
const showTrace = ref(false);

function run() {
  evaluation.value = demo.evaluate(props.law, evaluationParams());
  emit('evaluated', { law: props.law, evaluation: evaluation.value });
}

const doc = computed(() => props.law.doc);
const primaryOutputName = computed(() => {
  const cfg = corpus.value?.config?.dashboard_outputs ?? {};
  return cfg[`${props.law.service}/${props.law.law_path}`] ?? null;
});
const outputs = computed(() => {
  const o = evaluation.value?.ok ? evaluation.value.outputs : {};
  return Object.entries(o).filter(([k]) => k !== 'voldoet_aan_voorwaarden');
});
// true/false when the law decided; 'unknown' when it could not for lack of
// facts (RFC-036), which is never shown as a yes; a law without a verdict
// output (a tax, a registration) is computed for everyone.
const verdict = computed(() => {
  if (!evaluation.value?.ok) return null;
  const v = verdictOf(evaluation.value.outputs);
  return v === null ? true : v;
});
const requirementsMet = computed(() => verdict.value === true);
const lawName = (id) => corpus.value?.lawById(id)?.name ?? id;
// What the verdict lacks, when it is unknown.
const verdictMissing = computed(() => (verdict.value === 'unknown' ? formatMissing(evaluation.value.outputs.voldoet_aan_voorwaarden, { ownLaw: props.law.id, lawName }) : ''));
const primary = computed(() => {
  if (!evaluation.value?.ok) return null;
  const name = primaryOutputName.value && evaluation.value.outputs[primaryOutputName.value] !== undefined
    ? primaryOutputName.value
    : outputs.value.find(([, v]) => typeof v === 'number')?.[0] ?? outputs.value[0]?.[0];
  if (!name) return null;
  return { name, value: evaluation.value.outputs[name], spec: fieldSpec(doc.value, name) };
});
const secondary = computed(() => outputs.value.filter(([k]) => k !== primary.value?.name).slice(0, 6));

const lineage = computed(() => {
  if (!evaluation.value?.trace) return [];
  return lineageFromTrace(evaluation.value.trace, props.law.id, evaluationParams());
});
const valueCount = computed(() => leafValues(lineage.value).length);

const currentCase = computed(() => findCase(props.law));

// The questions this regeling has for the citizen (see askedInputs.js): the
// inputs no register holds and the application-form parameters. Until
// answered they are unknown; the tile asks for them instead of showing an
// error, and the answers are passed as parameters where the law wants them.
const askedInputs = computed(() => {
  void dataVersion.value;
  return corpus.value ? askedInputsFor(corpus.value, props.law, demo.claimFor) : [];
});
// What the engine ran into that only the citizen can answer (just in time).
const missingInputs = computed(() => nextQuestions(askedInputs.value, evaluation.value, props.law.id));
function evaluationParams() {
  return evaluationParamsFor(personaParams(), askedInputs.value);
}
// After the computeds it reads: the immediate run needs `askedInputs`.
watch([dataVersion, () => profile.value?.bsn], run, { immediate: true });

function supply(input) {
  const params = personaParams();
  emit('edit-value', {
    law: props.law,
    selfDeclared: true,
    node: { kind: 'value', law: props.law.id, name: input.name, value: input.claim?.newValue ?? null, service: null, ...claimKeyFor(props.law, params) },
  });
}
const produces = computed(() => {
  for (const a of doc.value.articles ?? []) {
    const p = a.machine_readable?.execution?.produces;
    if (p) return p;
  }
  return null;
});
const canApply = computed(() => evaluation.value?.ok && verdict.value === true && !currentCase.value && produces.value?.legal_character === 'BESCHIKKING');

// The application stays in the portal (a sheet); the case system is the
// caseworker's world.
function apply() {
  emit('apply', { law: props.law, evaluation: evaluation.value });
}

watch(showTrace, async (open) => {
  if (!open) return traceSheet.value?.hide?.();
  await nextTick();
  traceSheet.value?.show?.();
});

const statusTag = computed(() => {
  const c = currentCase.value;
  if (!c) return null;
  if (c.status === 'DECIDED') return c.approved ? { color: 'success', text: 'Toegekend', icon: 'checked' } : { color: 'critical', text: 'Afgewezen', icon: 'dismiss-circle' };
  if (c.status === 'IN_REVIEW') return { color: 'warning', text: 'In behandeling', icon: 'clock' };
  return { color: 'neutral', text: 'Ingediend', icon: 'paper-plane' };
});
</script>

<template>
  <nldd-card :accessible-label="law.name">
    <nldd-container slot="header" padding="16" layout="row" gap="12" vertical-alignment="top">
      <OrgLogo :service="law.service" />
      <nldd-title-cell size="5" :text="law.name" :supporting-text="corpus.services[law.service]?.name ?? law.service"></nldd-title-cell>
      <nldd-tag v-if="statusTag" :color="statusTag.color" :text="statusTag.text" :icon="statusTag.icon" size="sm"></nldd-tag>
    </nldd-container>

    <nldd-container padding-inline="16" padding-bottom="16" gap="12">
      <template v-if="!evaluation">
        <nldd-activity-indicator timing="instant" size="24"></nldd-activity-indicator>
      </template>
      <template v-else-if="missingInputs.length">
        <nldd-inline-dialog icon="edit" text="Nog een gegeven nodig" :supporting-text="`De wet is doorgerekend met wat de overheid weet en loopt vast op ${humanize(missingInputs[0].name).toLowerCase()}: dat staat in geen register, alleen u kunt het opgeven.`"></nldd-inline-dialog>
      </template>
      <template v-else-if="!evaluation.ok">
        <nldd-inline-dialog variant="alert" text="Kon deze regeling niet berekenen" :supporting-text="evaluation.error"></nldd-inline-dialog>
      </template>
      <template v-else>
        <nldd-inline-dialog v-if="verdict === 'unknown'" icon="info" text="Nog niet te bepalen" :supporting-text="`De wet kan met de bekende gegevens geen uitkomst geven; ${verdictMissing}.`"></nldd-inline-dialog>
        <nldd-list v-else variant="box-tinted" accessible-label="Uitkomst">
          <nldd-list-item size="md">
            <nldd-icon-cell :icon="requirementsMet ? 'check-mark-circle' : 'dismiss-circle'" :color="requirementsMet ? 'success' : 'critical'"></nldd-icon-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-title-cell
              size="4"
              :overline="requirementsMet ? 'U voldoet aan de voorwaarden' : 'U voldoet niet aan de voorwaarden'"
              :text="requirementsMet ? (primary ? formatValue(primary.value, primary.spec) : 'Ja') : 'Niet van toepassing'"
              :supporting-text="requirementsMet && primary ? (isUnknown(primary.value) ? `${humanize(primary.name)} · ${formatMissing(primary.value, { ownLaw: law.id, lawName })}` : humanize(primary.name)) : ''"
            ></nldd-title-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-list v-if="secondary.length" variant="simple" accessible-label="Overige uitkomsten">
          <nldd-list-item v-for="[name, value] in secondary" :key="name" size="sm">
            <nldd-text-cell size="sm" color="secondary" min-width="55%" :text="humanize(name)"></nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" max-width="45%" horizontal-alignment="right" :color="isUnknown(value) ? 'secondary' : 'default'" :text="formatValue(value, fieldSpec(doc, name))"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-list v-if="askedInputs.length" variant="simple" accessible-label="Door u opgegeven">
          <nldd-list-item v-for="input in askedInputs" :key="input.name" size="sm" button @click="supply(input)">
            <nldd-icon-cell icon="edit" size="16" color="accent"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell size="sm" :text="humanize(input.name)" supporting-text="door u opgegeven"></nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="input.claim ? formatValue(input.claim.newValue, input.spec) : 'Opgeven'"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-list type="tree" variant="box-tinted" accessible-label="Gebruikte gegevens">
          <nldd-list-item size="sm" button :expanded="showData" @click="showData = !showData">
            <nldd-icon-cell icon="rectangle-stack" size="16" color="secondary"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell size="sm" :text="`Gebruikte gegevens (${valueCount})`" :supporting-text="showData ? 'Klik op een gegeven om het te corrigeren' : undefined"></nldd-text-cell>
            <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            <DataLineage v-if="showData" :nodes="lineage" nested @edit="emit('edit-value', { node: $event, law })" />
          </nldd-list-item>
        </nldd-list>
      </template>
    </nldd-container>

    <!-- wrap, not row: in a narrow tile (three-column grid) a whole button moves to a
         second line, right-aligned with the other secondary actions, instead of a
         button breaking its label. The buttons stay direct children: a nested
         container has size containment and so no intrinsic width in a flex line. -->
    <nldd-container slot="footer" padding="16" layout="wrap" gap="8" vertical-alignment="center" horizontal-alignment="right">
      <nldd-button v-if="currentCase" variant="secondary" size="sm" start-icon="file-text" text="Mijn aanvraag" @click="apply"></nldd-button>
      <nldd-button v-else-if="evaluation && missingInputs.length && produces?.legal_character === 'BESCHIKKING'" variant="primary" size="sm" start-icon="edit" text="Gegevens aanvullen" @click="apply"></nldd-button>
      <nldd-button v-else-if="canApply" variant="primary" size="sm" start-icon="paper-plane" text="Aanvragen" @click="apply"></nldd-button>
      <nldd-spacer size="flexible" direction="horizontal"></nldd-spacer>
      <nldd-button v-if="evaluation?.ok" variant="neutral-transparent" size="sm" start-icon="list" text="Berekening" @click="showTrace = true"></nldd-button>
      <nldd-button variant="neutral-transparent" size="sm" start-icon="book" text="Wettekst" @click="router.push(`/wetten/${encodeURIComponent(law.id)}`)"></nldd-button>
    </nldd-container>

    <Teleport to="body">
      <nldd-sheet ref="traceSheet" placement="right" width="720px" accessible-label="Berekening" @close="showTrace = false">
        <nldd-page>
          <nldd-container slot="header" padding="12">
            <nldd-top-title-bar text="Berekening" :supporting-text="law.name" dismiss-text="Sluiten" @dismiss="showTrace = false"></nldd-top-title-bar>
          </nldd-container>
          <nldd-container padding="16">
            <nldd-rich-text spacing="tight"><p>Dit is de volledige uitvoering van de wet door de RegelRecht-engine voor deze persoon: elke stap, elk opgehaald gegeven en elke tussenuitkomst.</p></nldd-rich-text>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-code-viewer v-if="showTrace" variant="box-tinted" no-copy>{{ evaluation?.traceText }}</nldd-code-viewer>
          </nldd-container>
        </nldd-page>
      </nldd-sheet>
    </Teleport>
  </nldd-card>
</template>

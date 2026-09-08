<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import OrgLogo from './OrgLogo.vue';
import DataLineage from './DataLineage.vue';
import { fieldSpec, formatValue, humanize } from '../data/format.js';
import { lineageFromTrace, leafValues } from '../data/lineage.js';
import { useDemo } from '../store/demoStore.js';

// One regeling on the portal: the outcome of the law for this persona, the
// values it used (expandable, each correctable), the application button and
// the state of a submitted case. Re-evaluates whenever registered data changes.

const props = defineProps({
  law: { type: Object, required: true },
});
const emit = defineEmits(['edit-value', 'evaluated']);
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
const requirementsMet = computed(() => {
  if (!evaluation.value?.ok) return null;
  const v = evaluation.value.outputs?.voldoet_aan_voorwaarden;
  return v === undefined ? true : !!v;
});
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

// Inputs only the citizen can supply (`kind: claim` in the bindings): rent,
// childcare hours. Without them the law cannot be calculated; the tile asks
// for them instead of showing an error.
const claimInputs = computed(() => {
  const bindings = corpus.value?.bindings?.[props.law.id] ?? {};
  return Object.entries(bindings)
    .filter(([, b]) => b.kind === 'claim')
    .map(([name]) => ({ name, claim: demo.claimFor(props.law.id, name), spec: fieldSpec(doc.value, name) }));
});
// Parameters the law declares beyond the persona's identity (an application
// form: terrace size, whether food is served). They are asked for the same
// way; until supplied they are passed as null so the law computes with unknowns.
const formParameters = computed(() => {
  const identity = new Set(Object.keys(personaParams()));
  const out = [];
  for (const article of doc.value.articles ?? []) {
    for (const p of article.machine_readable?.execution?.parameters ?? []) {
      if (identity.has(p.name) || out.some((o) => o.name === p.name)) continue;
      out.push({ name: p.name, claim: demo.claimFor(props.law.id, p.name), spec: p, isParameter: true });
    }
  }
  return out;
});
const askedInputs = computed(() => [...claimInputs.value, ...formParameters.value]);
const missingInputs = computed(() => askedInputs.value.filter((i) => !i.claim));
function evaluationParams() {
  const params = personaParams();
  for (const p of formParameters.value) params[p.name] = p.claim ? p.claim.newValue : null;
  return params;
}
// After the computeds it reads: the immediate run needs `formParameters`.
watch([dataVersion, () => profile.value?.bsn], run, { immediate: true });

function supply(input) {
  const params = personaParams();
  emit('edit-value', {
    law: props.law,
    selfDeclared: true,
    node: { kind: 'value', law: props.law.id, name: input.name, value: input.claim?.newValue ?? null, service: null, keyField: params.kvk_nummer && !params.bsn ? 'kvk_nummer' : 'bsn', keyValue: params.bsn ?? params.kvk_nummer },
  });
}
const produces = computed(() => {
  for (const a of doc.value.articles ?? []) {
    const p = a.machine_readable?.execution?.produces;
    if (p) return p;
  }
  return null;
});
const canApply = computed(() => evaluation.value?.ok && requirementsMet.value && !currentCase.value && produces.value?.legal_character === 'BESCHIKKING');

function apply() {
  const c = demo.submitCase(props.law, evaluation.value, evaluationParams());
  if (c) router.push(`/zaaksysteem/${c.id}`);
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
    <nldd-container slot="header" padding="16" layout="row" gap="12" vertical-alignment="center">
      <OrgLogo :service="law.service" />
      <nldd-title-cell size="5" :text="law.name" :supporting-text="corpus.services[law.service]?.name ?? law.service"></nldd-title-cell>
      <nldd-tag v-if="statusTag" :color="statusTag.color" :text="statusTag.text" :icon="statusTag.icon" size="sm"></nldd-tag>
    </nldd-container>

    <nldd-container padding-inline="16" padding-bottom="16" class="tile-body">
      <template v-if="!evaluation">
        <nldd-activity-indicator timing="instant" size="24"></nldd-activity-indicator>
      </template>
      <template v-else-if="missingInputs.length">
        <nldd-inline-dialog icon="edit" text="Aanvullende gegevens nodig" supporting-text="Deze gegevens staan in geen register; alleen u kunt ze opgeven."></nldd-inline-dialog>
        <nldd-list variant="box" accessible-label="Benodigde gegevens">
          <nldd-list-item v-for="input in askedInputs" :key="input.name" size="sm" button @click="supply(input)">
            <nldd-icon-cell :icon="input.claim ? 'checked' : 'exclamation-circle'" size="16" :color="input.claim ? 'success' : 'warning'"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell size="sm" :text="humanize(input.name)" :supporting-text="input.spec?.description"></nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="input.claim ? 'default' : 'warning'" :text="input.claim ? formatValue(input.claim.newValue, input.spec) : 'Opgeven'"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
      </template>
      <template v-else-if="!evaluation.ok">
        <nldd-inline-dialog variant="alert" text="Kon deze regeling niet berekenen" :supporting-text="evaluation.error"></nldd-inline-dialog>
      </template>
      <template v-else>
        <nldd-list variant="box" background="tinted" accessible-label="Uitkomst">
          <nldd-list-item size="md">
            <nldd-icon-cell :icon="requirementsMet ? 'check-mark-circle' : 'dismiss-circle'" :color="requirementsMet ? 'success' : 'critical'"></nldd-icon-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-title-cell
              size="4"
              :overline="requirementsMet ? 'U voldoet aan de voorwaarden' : 'U voldoet niet aan de voorwaarden'"
              :text="requirementsMet ? (primary ? formatValue(primary.value, primary.spec) : 'Ja') : 'Niet van toepassing'"
              :supporting-text="requirementsMet && primary ? humanize(primary.name) : ''"
            ></nldd-title-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-list v-if="secondary.length" variant="simple" accessible-label="Overige uitkomsten">
          <nldd-list-item v-for="[name, value] in secondary" :key="name" size="sm">
            <nldd-text-cell size="sm" color="secondary" min-width="55%" :text="humanize(name)"></nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" max-width="45%" horizontal-alignment="right" :text="formatValue(value, fieldSpec(doc, name))"></nldd-text-cell>
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
        <nldd-button
          variant="neutral-tinted"
          size="sm"
          width="full"
          horizontal-alignment="left"
          :start-icon="showData ? 'chevron-up' : 'chevron-down'"
          :text="`Gebruikte gegevens (${valueCount})`"
          :expanded="showData || undefined"
          @click="showData = !showData"
        ></nldd-button>
        <template v-if="showData">
          <DataLineage :nodes="lineage" @edit="emit('edit-value', { node: $event, law })" />
          <nldd-rich-text spacing="tight"><p><small>Klik op een gegeven om het te corrigeren.</small></p></nldd-rich-text>
        </template>
      </template>
    </nldd-container>

    <nldd-container slot="footer" padding="16" layout="row" gap="8" vertical-alignment="center">
      <nldd-button v-if="canApply" variant="primary" size="sm" start-icon="paper-plane" text="Aanvragen" @click="apply"></nldd-button>
      <nldd-button v-else-if="currentCase" variant="secondary" size="sm" start-icon="inbox" text="Bekijk zaak" @click="router.push(`/zaaksysteem/${currentCase.id}`)"></nldd-button>
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
            <nldd-code-viewer v-if="showTrace" variant="box" no-copy>{{ evaluation?.traceText }}</nldd-code-viewer>
          </nldd-container>
        </nldd-page>
      </nldd-sheet>
    </Teleport>
  </nldd-card>
</template>

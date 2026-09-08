<script setup>
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import OrgLogo from '../components/OrgLogo.vue';
import { fieldSpec, formatDateTime, formatValue, humanize } from '../data/format.js';
import { useDemo } from '../store/demoStore.js';

// The caseworker's side: applications the citizen submitted, in three lanes,
// with the engine's fresh verdict next to what the citizen claimed, the
// corrections that wait for a decision, and the decision itself.

const route = useRoute();
const router = useRouter();
const demo = useDemo();
const { corpus, profile, state, dataVersion } = demo;

const service = ref(null);
watch(() => profile.value?.zaaksysteem_service, (s) => { if (s && !service.value) service.value = s; }, { immediate: true });

const services = computed(() => {
  const set = new Set([profile.value?.zaaksysteem_service, ...state.cases.map((c) => c.service)].filter(Boolean));
  return [...set];
});

const cases = computed(() => state.cases.filter((c) => c.service === service.value));
const lanes = computed(() => [
  { key: 'SUBMITTED', title: 'Ingediend', items: cases.value.filter((c) => c.status === 'SUBMITTED') },
  { key: 'IN_REVIEW', title: 'In behandeling', items: cases.value.filter((c) => c.status === 'IN_REVIEW') },
  { key: 'DECIDED', title: 'Besloten', items: cases.value.filter((c) => c.status === 'DECIDED') },
]);

const selected = computed(() => state.cases.find((c) => c.id === route.params.caseId) ?? null);
watch(selected, (c) => { if (c && c.service !== service.value) service.value = c.service; }, { immediate: true });

function open(c) {
  router.push(`/zaaksysteem/${c.id}`);
}
function close() {
  router.push('/zaaksysteem');
}

function personaName(bsn) {
  return corpus.value?.profiles?.profiles?.[bsn]?.name ?? bsn;
}
function lawOf(c) {
  return corpus.value?.lawById(c.lawId) ?? null;
}

// The engine's current verdict for the selected case (with approved
// corrections applied), next to the citizen's claimed result.
const verified = computed(() => {
  const c = selected.value;
  const law = c ? lawOf(c) : null;
  if (!c || !law) return null;
  void dataVersion.value;
  return demo.evaluate(law, c.parameters);
});

function outputRows(c) {
  const law = lawOf(c);
  const claimed = c.claimedResult ?? {};
  const now = verified.value?.ok ? verified.value.outputs : {};
  const names = [...new Set([...Object.keys(claimed), ...Object.keys(now)])];
  return names.map((name) => ({
    name,
    spec: fieldSpec(law?.doc, name),
    claimed: claimed[name],
    now: now[name],
    differs: JSON.stringify(claimed[name]) !== JSON.stringify(now[name]),
  }));
}

const caseClaims = computed(() => (selected.value ? state.claims.filter((cl) => cl.caseId === selected.value.id || (cl.bsn === selected.value.bsn && cl.tileLawId === selected.value.lawId)) : []));
const serviceClaims = computed(() => state.claims.filter((cl) => cl.status === 'PENDING' && (corpus.value?.lawById(cl.tileLawId)?.service === service.value || corpus.value?.lawById(cl.lawId)?.service === service.value)));

const reason = ref('');
function decide(approved) {
  if (!selected.value) return;
  const text = reason.value.trim() || (approved ? 'Voldoet aan de voorwaarden.' : 'Voldoet niet aan de voorwaarden.');
  demo.decideCase(selected.value.id, approved, text, verified.value?.ok ? verified.value.outputs : null);
  reason.value = '';
}
function decideObjection(upheld) {
  if (!selected.value) return;
  demo.decideObjection(selected.value.id, upheld, reason.value.trim() || (upheld ? 'Bezwaar gegrond.' : 'Bezwaar ongegrond.'));
  reason.value = '';
}
const objectionReason = ref('');
function fileObjection() {
  if (!selected.value) return;
  demo.objectToCase(selected.value.id, objectionReason.value.trim() || 'Ik ben het niet eens met het besluit.');
  objectionReason.value = '';
}

function laneTag(c) {
  if (c.status === 'DECIDED') return c.approved ? { color: 'success', text: 'Toegekend' } : { color: 'critical', text: 'Afgewezen' };
  if (c.objection?.status === 'PENDING') return { color: 'warning', text: 'Bezwaar' };
  return { color: 'neutral', text: c.status === 'IN_REVIEW' ? 'Te beoordelen' : 'Nieuw' };
}
function claimLawName(cl) {
  return corpus.value?.lawById(cl.lawId)?.name ?? cl.lawId;
}
function claimSpec(cl) {
  return fieldSpec(corpus.value?.lawById(cl.lawId)?.doc, cl.input);
}
</script>

<template>
  <nldd-navigation-split-view inspector-accessible-label="Zaakdetails">
    <nldd-split-view-pane slot="main" has-content>
      <nldd-page background="tinted">
        <nldd-container slot="header" padding="12" background="base">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start" v-if="service"><OrgLogo :service="service" /></nldd-toolbar-item>
            <nldd-toolbar-title slot="start" text="Zaaksysteem" :supporting-text="corpus.services[service]?.name ?? service ?? ''"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end" v-if="services.length > 1">
              <nldd-segmented-control size="sm" width="fit-content" :value="service" @change="service = $event.detail?.value">
                <nldd-segmented-control-item v-for="s in services" :key="s" :value="s" :text="s"></nldd-segmented-control-item>
              </nldd-segmented-control>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>

        <nldd-simple-section width="full">
          <nldd-inline-dialog v-if="cases.length === 0 && serviceClaims.length === 0" icon="inbox" text="Geen zaken" supporting-text="Zodra een burger op het portaal een aanvraag indient, verschijnt die hier."></nldd-inline-dialog>
          <div v-else class="case-board">
            <div v-for="lane in lanes" :key="lane.key">
              <nldd-container padding-inline="12" padding-block="6">
                <nldd-text-cell size="sm" color="secondary" :text="`**${lane.title}** (${lane.items.length})`"></nldd-text-cell>
              </nldd-container>
              <nldd-list variant="box" background="base" :accessible-label="lane.title">
                <nldd-list-item v-if="lane.items.length === 0" size="sm">
                  <nldd-text-cell size="sm" color="secondary" text="Leeg"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item v-for="c in lane.items" :key="c.id" size="md" button :selected="selected?.id === c.id || undefined" @click="open(c)">
                  <nldd-text-cell :text="c.lawName" :supporting-text="`${personaName(c.bsn)} · ${formatDateTime(c.submittedAt)}`"></nldd-text-cell>
                  <nldd-cell><nldd-tag size="sm" :color="laneTag(c).color" :text="laneTag(c).text"></nldd-tag></nldd-cell>
                </nldd-list-item>
              </nldd-list>
            </div>
          </div>
        </nldd-simple-section>

        <nldd-simple-section v-if="serviceClaims.length" width="full" padding-top="0">
          <nldd-container padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Correcties van burgers ter beoordeling"></nldd-text-cell></nldd-container>
<nldd-list variant="box" background="base" accessible-label="Correcties ter beoordeling">
            <nldd-list-item v-for="cl in serviceClaims" :key="cl.id" size="md">
              <nldd-text-cell :text="`${humanize(cl.input)}: ${formatValue(cl.oldValue, claimSpec(cl))} → **${formatValue(cl.newValue, claimSpec(cl))}**`" :supporting-text="`${personaName(cl.bsn)} · ${claimLawName(cl)} · ${cl.reason}`"></nldd-text-cell>
              <nldd-cell>
                <nldd-button-group orientation="horizontal" size="sm">
                  <nldd-button size="sm" variant="primary" text="Goedkeuren" @click="demo.decideClaim(cl.id, true)"></nldd-button>
                  <nldd-button size="sm" variant="secondary" text="Afwijzen" @click="demo.decideClaim(cl.id, false)"></nldd-button>
                </nldd-button-group>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="inspector" :has-content="!!selected || undefined">
      <nldd-page v-if="selected">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="selected.lawName" :supporting-text="`Zaak ${selected.id.slice(-5)} · ${personaName(selected.bsn)}`" dismiss-text="Sluiten" @dismiss="close"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="12" gap="16">
          <nldd-banner
            :variant="selected.status === 'DECIDED' ? (selected.approved ? 'success' : 'critical') : 'accent'"
            :text="selected.status === 'DECIDED' ? (selected.approved ? 'Toegekend' : 'Afgewezen') : selected.objection?.status === 'PENDING' ? 'Bezwaar ingediend' : 'Wacht op beoordeling'"
            :supporting-text="selected.reason ?? selected.events.at(-1)?.text"
          ></nldd-banner>

          <nldd-container padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Uitkomst" supporting-text="aangevraagd → nu berekend door de engine"></nldd-text-cell></nldd-container>
<nldd-list variant="box" accessible-label="Uitkomst">
            <nldd-list-item v-for="row in outputRows(selected)" :key="row.name" size="sm">
              <nldd-text-cell size="sm" :text="humanize(row.name)"></nldd-text-cell>
              <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="row.differs ? 'warning' : 'default'">
                <template v-if="row.differs"><s>{{ formatValue(row.claimed, row.spec) }}</s> → {{ formatValue(row.now, row.spec) }}</template>
                <template v-else>{{ formatValue(row.now, row.spec) }}</template>
              </nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-banner v-if="verified && !verified.ok" variant="warning" text="Herberekening mislukt" :supporting-text="verified.error"></nldd-banner>

          <nldd-container v-if="caseClaims.length" padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Correcties van de burger"></nldd-text-cell></nldd-container>
<nldd-list v-if="caseClaims.length" variant="box" accessible-label="Correcties">
            <nldd-list-item v-for="cl in caseClaims" :key="cl.id" size="sm">
              <nldd-text-cell size="sm" :text="`${humanize(cl.input)}: ${formatValue(cl.oldValue, claimSpec(cl))} → **${formatValue(cl.newValue, claimSpec(cl))}**`" :supporting-text="cl.reason"></nldd-text-cell>
              <nldd-cell>
                <nldd-tag v-if="cl.status !== 'PENDING'" size="sm" :color="cl.status === 'APPROVED' ? 'success' : 'critical'" :text="cl.status === 'APPROVED' ? 'Goedgekeurd' : 'Afgewezen'"></nldd-tag>
                <nldd-button-group v-else orientation="horizontal" size="sm">
                  <nldd-button size="sm" variant="primary" text="Goedkeuren" @click="demo.decideClaim(cl.id, true)"></nldd-button>
                  <nldd-button size="sm" variant="secondary" text="Afwijzen" @click="demo.decideClaim(cl.id, false)"></nldd-button>
                </nldd-button-group>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>

          <template v-if="selected.status !== 'DECIDED' && !selected.objection">
            <nldd-form-field label="Motivering">
              <nldd-multi-line-text-field :value="reason" rows="2" placeholder="Toelichting bij het besluit" @input="reason = $event.detail?.value ?? $event.target.value"></nldd-multi-line-text-field>
            </nldd-form-field>
            <nldd-button-group orientation="horizontal">
              <nldd-button variant="primary" start-icon="checked" text="Toekennen" @click="decide(true)"></nldd-button>
              <nldd-button variant="destructive" start-icon="dismiss" text="Afwijzen" @click="decide(false)"></nldd-button>
            </nldd-button-group>
          </template>
          <template v-else-if="selected.objection?.status === 'PENDING'">
            <nldd-rich-text spacing="tight"><p><strong>Bezwaar:</strong> {{ selected.objection.reason }}</p></nldd-rich-text>
            <nldd-form-field label="Motivering">
              <nldd-multi-line-text-field :value="reason" rows="2" @input="reason = $event.detail?.value ?? $event.target.value"></nldd-multi-line-text-field>
            </nldd-form-field>
            <nldd-button-group orientation="horizontal">
              <nldd-button variant="primary" text="Bezwaar gegrond" @click="decideObjection(true)"></nldd-button>
              <nldd-button variant="secondary" text="Bezwaar ongegrond" @click="decideObjection(false)"></nldd-button>
            </nldd-button-group>
          </template>
          <template v-else-if="selected.status === 'DECIDED' && !selected.objection">
            <nldd-form-field label="Namens de burger: bezwaar maken" optional>
              <nldd-multi-line-text-field :value="objectionReason" rows="2" placeholder="Reden van het bezwaar (Awb art. 6:5)" @input="objectionReason = $event.detail?.value ?? $event.target.value"></nldd-multi-line-text-field>
            </nldd-form-field>
            <nldd-button variant="secondary" start-icon="flag" text="Bezwaar indienen" @click="fileObjection"></nldd-button>
          </template>

          <nldd-container padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Gebeurtenissen"></nldd-text-cell></nldd-container>
<nldd-list variant="box" accessible-label="Gebeurtenissen">
            <nldd-list-item v-for="(ev, i) in selected.events" :key="i" size="sm">
              <nldd-timeline-track-cell :step="i === selected.events.length - 1 ? 'future' : 'past'" :child="i === 0 ? 'first' : i === selected.events.length - 1 ? 'last' : 'between'"></nldd-timeline-track-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell size="sm" :text="ev.text" :supporting-text="formatDateTime(ev.at)"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>

<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import CorrectionRows from '../components/CorrectionRows.vue';
import DataLineage from '../components/DataLineage.vue';
import EditValueSheet from '../components/EditValueSheet.vue';
import { fieldSpec, formatDateTime, formatValue, humanize } from '../data/format.js';
import { lineageFromTrace } from '../data/lineage.js';
import { useDemo } from '../store/demoStore.js';
import { awbOutcomes, statusOf } from '../data/lifecycle.js';

// The caseworker's side: applications the citizen submitted, in three lanes,
// with the engine's fresh verdict next to what the citizen claimed, the
// corrections that wait for a decision, and the decision itself. The caseworker
// sees exactly what the citizen sees (the same data tree under the outcome) and
// can make every change the citizen can; such a correction applies at once.

const route = useRoute();
const router = useRouter();
const demo = useDemo();
const { corpus, profile, state, dataVersion } = demo;

const service = ref(null);
watch(() => profile.value?.zaaksysteem_service, (s) => { if (s && !service.value) service.value = s; }, { immediate: true });

// Every organisation that executes a law in the demo has a case system; the
// presenter can step into any of them. The profile picks the one to start in.
const services = computed(() => {
  if (!corpus.value) return [];
  const set = new Set([...corpus.value.latestById.values()].map((l) => l.service).filter(Boolean));
  return [...set].sort((a, b) => (corpus.value.services[a]?.name ?? a).localeCompare(corpus.value.services[b]?.name ?? b));
});
/** The regelingen this organisation executes, with the number of cases per regeling. */
const orgLaws = computed(() => {
  if (!corpus.value || !service.value) return [];
  return [...corpus.value.latestById.values()]
    .filter((l) => l.service === service.value)
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((law) => ({ law, count: state.cases.filter((c) => c.lawId === law.id).length }));
});

const cases = computed(() => state.cases.filter((c) => c.service === service.value));
// De banen volgen de fasen die de Awb aan een beschikking geeft (RFC-008), en
// niet een eigen indeling: te beoordelen, besloten maar nog niet verstuurd, en
// bekendgemaakt. Die middelste baan is de reden van deze verandering — een
// besluit dat genomen is maar nog niet is meegedeeld, is een echt moment in de
// wet en was hier eerder onzichtbaar.
const lanes = computed(() => [
  { key: 'IN_REVIEW', title: 'Te beoordelen', items: cases.value.filter((c) => statusOf(c) !== 'DECIDED') },
  { key: 'BESLUIT', title: 'Bekend te maken', items: cases.value.filter((c) => statusOf(c) === 'DECIDED' && !c.publishedAt) },
  { key: 'BEKENDMAKING', title: 'Bekendgemaakt', items: cases.value.filter((c) => statusOf(c) === 'DECIDED' && c.publishedAt) },
]);

const selected = computed(() => state.cases.find((c) => c.id === route.params.caseId) ?? null);
watch(selected, (c) => { if (c && c.service !== service.value) service.value = c.service; }, { immediate: true });

// The case opens in a sheet over the board, not in an inspector column beside
// it. A case carries the banner, both outcomes, the whole data tree and the
// corrections; the inspector is a narrow fixed rail and squeezed all of that
// into a column too thin to read. It also matches the citizen's side, where an
// application opens the same way (ApplicationSheet), so the same case looks the
// same from both ends.
const caseSheet = ref(null);
watch(
  selected,
  async (c) => {
    if (!c) return caseSheet.value?.hide?.();
    await nextTick();
    caseSheet.value?.show?.();
  },
  { immediate: true },
);

function open(c) {
  router.push(`/zaaksysteem/${c.id}`);
}
function close() {
  router.push('/zaaksysteem');
}

function personaName(bsn) {
  // A BSN without a persona (e.g. a case persisted before profiles.yaml
  // changed) is stated as a fact, not passed off as a name.
  return corpus.value?.profiles?.profiles?.[bsn]?.name ?? `onbekende persoon (BSN ${bsn})`;
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

// The same tree the citizen sees in the application: the values the law used,
// keyed on the case's own parameters (not the active profile's).
const lineage = computed(() => {
  const c = selected.value;
  if (!c || !verified.value?.ok || !verified.value.trace) return [];
  return lineageFromTrace(verified.value.trace, c.lawId, c.parameters ?? {});
});
/** The lineage value node the caseworker is correcting; null = sheet closed. */
const editing = ref(null);

function outputRows(c) {
  const law = lawOf(c);
  const claimed = c.claimedResult ?? {};
  // A failed recomputation has no "now" value to compare against: show the
  // claimed values as they are and let the warning banner explain the failure.
  const recomputed = !!verified.value?.ok;
  const now = recomputed ? verified.value.outputs : {};
  const names = [...new Set([...Object.keys(claimed), ...Object.keys(now)])];
  return names.map((name) => ({
    name,
    spec: fieldSpec(law?.doc, name),
    claimed: claimed[name],
    now: recomputed ? now[name] : claimed[name],
    differs: recomputed && JSON.stringify(claimed[name]) !== JSON.stringify(now[name]),
  }));
}

const caseClaims = computed(() => (selected.value ? state.claims.filter((cl) => cl.caseId === selected.value.id || (cl.bsn === selected.value.bsn && cl.tileLawId === selected.value.lawId)) : []));
const serviceClaims = computed(() => state.claims.filter((cl) => cl.status === 'PENDING' && (corpus.value?.lawById(cl.tileLawId)?.service === service.value || corpus.value?.lawById(cl.lawId)?.service === service.value)));

const reason = ref('');
function decide(approved) {
  if (!selected.value) return;
  // Without a typed reason, say who decided — not what the law says. A
  // caseworker can approve or reject against the engine's own outcome, and
  // "Voldoet niet aan de voorwaarden" then claimed the law had rejected the
  // citizen while the tile beside it showed the amount the law had granted.
  // Awb art. 3:46 asks a besluit to rest on a deugdelijke motivering; the
  // honest fallback is that none was given.
  const text = reason.value.trim() || (approved ? 'Toegekend door de behandelaar. Geen toelichting gegeven.' : 'Afgewezen door de behandelaar. Geen toelichting gegeven.');
  demo.decideCase(selected.value.id, approved, text, verified.value?.ok ? verified.value.outputs : null);
  reason.value = '';
}
/** Het besluit gaat de deur uit (Awb 3:41); daarna pas loopt de termijn. */
function publish() {
  if (!selected.value) return;
  demo.publishCase(selected.value.id);
}
/** Wat de Awb aan dit besluit heeft toegevoegd. */
const awb = computed(() => awbOutcomes(selected.value));

function decideObjection(upheld) {
  if (!selected.value) return;
  demo.decideObjection(selected.value.id, upheld, reason.value.trim() || (upheld ? 'Bezwaar gegrond.' : 'Bezwaar ongegrond.'));
  reason.value = '';
}

function laneTag(c) {
  if (c.objection?.status === 'PENDING') return { color: 'warning', text: 'Bezwaar' };
  if (statusOf(c) === 'DECIDED') return c.approved ? { color: 'success', text: 'Toegekend' } : { color: 'critical', text: 'Afgewezen' };
  return { color: 'neutral', text: 'Te beoordelen' };
}
function claimLawName(cl) {
  return corpus.value?.lawById(cl.lawId)?.name ?? cl.lawId;
}
</script>

<template>
  <nldd-navigation-split-view>
    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar text="Zaaksysteem" :supporting-text="corpus.services[service]?.name ?? service ?? ''">
            <nldd-dropdown slot="toolbar" size="sm" accessible-label="Organisatie">
              <select :value="service" @change="service = $event.target.value">
                <option v-for="s in services" :key="s" :value="s">{{ corpus.services[s]?.name ?? s }}</option>
              </select>
            </nldd-dropdown>
          </nldd-top-title-bar>
        </nldd-container>

        <nldd-simple-section width="full">
          <nldd-container layout="grid" column-count="3" sm-column-count="1" gap="16">
            <nldd-box v-for="lane in lanes" :key="lane.key" background="tinted">
              <nldd-container padding="12" gap="8">
                <nldd-container layout="row" gap="8" vertical-alignment="center" padding-inline="4">
                  <nldd-title-cell size="6" :text="lane.title" heading-level="2"></nldd-title-cell>
                  <nldd-badge color="neutral" :number="lane.items.length" :accessible-label="`${lane.items.length} zaken`"></nldd-badge>
                </nldd-container>
                <nldd-container v-if="lane.items.length === 0" padding-inline="4" padding-block="8">
                  <nldd-text-cell size="sm" color="secondary" text="Geen zaken"></nldd-text-cell>
                </nldd-container>
                <nldd-list v-for="c in lane.items" :key="c.id" variant="box-base" :accessible-label="c.lawName">
                  <nldd-list-item size="md" button :selected="selected?.id === c.id || undefined" @click="open(c)">
                    <!-- The status tag sits in the overline, not in an end cell: a lane is narrow
                         and an end cell never shrinks, so beside the tag the title would break
                         per letter. In the overline the title keeps the whole card width. -->
                    <nldd-text-cell :text="c.lawName" :supporting-text="`${personaName(c.bsn)} · ${formatDateTime(c.submittedAt)}`">
                      <nldd-tag slot="overline" size="sm" :color="laneTag(c).color" :text="laneTag(c).text"></nldd-tag>
                    </nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
              </nldd-container>
            </nldd-box>
          </nldd-container>
        </nldd-simple-section>

        <nldd-simple-section width="full" padding-top="0">
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ `Regelingen die ${corpus.services[service]?.name ?? service} uitvoert` }}</nldd-text></nldd-container>
            <nldd-list variant="box-tinted" accessible-label="Regelingen van deze organisatie">
              <nldd-list-item v-if="orgLaws.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" text="Geen regelingen in het demo-corpus"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="{ law, count } in orgLaws" :key="law.id" size="sm" button @click="router.push(`/wetten/${encodeURIComponent(law.id)}`)">
                <nldd-text-cell size="sm" :text="law.name" :supporting-text="law.discoverable === 'BUSINESS' ? 'voor ondernemers' : law.discoverable === 'CITIZEN' ? 'voor burgers' : 'levert gegevens aan andere wetten'"></nldd-text-cell>
                <nldd-cell><nldd-tag size="sm" :color="count ? 'accent' : 'neutral'" :text="count === 1 ? '1 zaak' : `${count} zaken`"></nldd-tag></nldd-cell>
                <nldd-icon-cell icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </nldd-simple-section>

        <nldd-simple-section v-if="serviceClaims.length" width="full" padding-top="0">
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">Correcties van burgers ter beoordeling</nldd-text></nldd-container>
            <nldd-list variant="box-tinted" accessible-label="Correcties ter beoordeling">
              <CorrectionRows :claims="serviceClaims" :origin="(cl) => `${personaName(cl.bsn)} · ${claimLawName(cl)}`" />
            </nldd-list>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

  </nldd-navigation-split-view>

  <Teleport to="body">
    <nldd-sheet ref="caseSheet" placement="right" width="720px" accessible-label="Zaakdetails" @close="close">
      <nldd-page v-if="selected">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="selected.lawName" :supporting-text="`Zaak ${selected.id.slice(-5)} · ${personaName(selected.bsn)}`" dismiss-text="Sluiten" @dismiss="close"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="16">
          <nldd-banner
            :variant="selected.status === 'DECIDED' ? (selected.approved ? 'success' : 'critical') : 'accent'"
            :text="selected.status === 'DECIDED' ? (selected.approved ? 'Toegekend' : 'Afgewezen') : selected.objection?.status === 'PENDING' ? 'Bezwaar ingediend' : 'Wacht op beoordeling'"
            :supporting-text="selected.reason ?? selected.events.at(-1)?.text"
          ></nldd-banner>

          <nldd-container gap="4">

            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">Uitkomst</nldd-text><nldd-text size="xs" color="secondary">{{ verified?.ok ? 'aangevraagd → nu berekend door de engine' : 'zoals aangevraagd' }}</nldd-text></nldd-container>

            <nldd-list variant="box-tinted" accessible-label="Uitkomst">
              <nldd-list-item v-for="row in outputRows(selected)" :key="row.name" size="sm">
                <nldd-text-cell size="sm" :text="humanize(row.name)"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="row.differs ? 'warning' : 'default'">
                  <template v-if="row.differs"><s>{{ formatValue(row.claimed, row.spec) }}</s> → {{ formatValue(row.now, row.spec) }}</template>
                  <template v-else>{{ formatValue(row.now, row.spec) }}</template>
                </nldd-text-cell>
              </nldd-list-item>
            </nldd-list>

          </nldd-container>
          <nldd-banner v-if="verified && !verified.ok" variant="warning" text="Herberekening mislukt" :supporting-text="verified.error"></nldd-banner>

          <nldd-container v-if="lineage.length" gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">Gebruikte gegevens</nldd-text><nldd-text size="xs" color="secondary">dezelfde gegevens als de burger ziet; klik op een gegeven om het te corrigeren</nldd-text></nldd-container>
            <nldd-list type="tree" variant="box-tinted" accessible-label="Gebruikte gegevens">
              <DataLineage :nodes="lineage" @edit="editing = $event" />
            </nldd-list>
          </nldd-container>

          <nldd-container v-if="caseClaims.length" padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Correcties"></nldd-text-cell></nldd-container>
          <nldd-list v-if="caseClaims.length" variant="box-tinted" accessible-label="Correcties">
            <CorrectionRows :claims="caseClaims" />
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
          <!-- Besloten, maar nog niet verstuurd. De Awb maakt van het besluit
               (art. 1:3) en de bekendmaking ervan (art. 3:41) twee momenten, en
               pas het tweede laat de bezwaartermijn beginnen (art. 6:8). Dat is
               hier dus ook een eigen handeling, en geen bijzaak van toekennen. -->
          <template v-else-if="!selected.publishedAt">
            <nldd-banner
              variant="accent"
              text="Het besluit is genomen en nog niet bekendgemaakt"
              supporting-text="Zolang het besluit niet is verstuurd, loopt er geen bezwaartermijn: de belanghebbende weet nog van niets (Awb art. 3:41 en 6:8)."
            ></nldd-banner>
            <nldd-button-group orientation="horizontal">
              <nldd-button variant="primary" start-icon="paper-plane" text="Bekendmaken" @click="publish"></nldd-button>
            </nldd-button-group>
          </template>
          <template v-else-if="!selected.objection">
            <!-- De termijn komt uit de wet: 6:7 geeft het aantal weken, 6:8 de
                 einddatum vanaf de bekendmaking. -->
            <nldd-list v-if="awb.bezwaartermijnEinde" variant="box-tinted" accessible-label="Bezwaartermijn">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" text="Bezwaar mogelijk tot en met"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(awb.bezwaartermijnEinde, null)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-rich-text spacing="tight"><p><small>Bekendgemaakt. De burger kan op het portaal bezwaar maken; dat verschijnt dan hier.</small></p></nldd-rich-text>
          </template>

          <nldd-container gap="4">

            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">Gebeurtenissen</nldd-text></nldd-container>

            <nldd-list variant="box-tinted" accessible-label="Gebeurtenissen">
              <nldd-list-item v-for="(ev, i) in selected.events" :key="i" size="sm">
                <nldd-timeline-track-cell :step="i === selected.events.length - 1 ? 'future' : 'past'" :child="i === 0 ? 'first' : i === selected.events.length - 1 ? 'last' : 'between'"></nldd-timeline-track-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell size="sm" :text="ev.text" :supporting-text="formatDateTime(ev.at)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>

          </nldd-container>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
  <!-- The same correction sheet as on the portal, filled in by the caseworker for this case. -->
  <EditValueSheet
    v-if="selected"
    :open="!!editing"
    :node="editing"
    :tile-law-id="selected.lawId"
    claimant="BEHANDELAAR"
    :bsn="selected.bsn"
    :case-id="selected.id"
    @close="editing = null"
  />
</template>

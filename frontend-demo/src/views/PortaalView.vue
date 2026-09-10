<script setup>
import { computed, reactive, ref, shallowReactive } from 'vue';
import LawTile from '../components/LawTile.vue';
import EditValueSheet from '../components/EditValueSheet.vue';
import ApplicationSheet from '../components/ApplicationSheet.vue';
import { fieldSpec, numericImpact } from '../data/format.js';
import { loadFailures } from '../engine/useDemoEngine.js';
import { useDemo } from '../store/demoStore.js';

// The citizen's (or entrepreneur's) portal: every regeling the persona can
// discover, evaluated live, ordered by financial impact. This is where the
// audience sees the law do its work for one person, corrects a value and
// submits an application.

const demo = useDemo();
const { profile, persona, portalLaws, corpus, state } = demo;

// Impact per law (from the tiles' evaluations) drives the ordering.
const impact = reactive({});
/** Latest evaluation per law, so the application sheet follows the tile. */
const evaluations = shallowReactive({});
function onEvaluated({ law, evaluation }) {
  evaluations[law.id] = evaluation;
  if (!evaluation?.ok) {
    impact[law.id] = -1;
    return;
  }
  const cfg = corpus.value?.config?.dashboard_outputs ?? {};
  const name = cfg[`${law.service}/${law.law_path}`];
  const outputs = evaluation.outputs ?? {};
  let value = 0;
  if (name && typeof outputs[name] === 'number') value = Math.abs(numericImpact(outputs[name], fieldSpec(law.doc, name)));
  else {
    for (const [k, v] of Object.entries(outputs)) {
      if (typeof v === 'number') value = Math.max(value, Math.abs(numericImpact(v, fieldSpec(law.doc, k))));
    }
  }
  if (outputs.voldoet_aan_voorwaarden === false) value = -0.5;
  impact[law.id] = value;
}

const sortedLaws = computed(() =>
  [...portalLaws.value].sort((a, b) => (impact[b.id] ?? 0) - (impact[a.id] ?? 0) || a.name.localeCompare(b.name)),
);

const editing = ref(null); // { node, law }
function onEditValue(payload) {
  editing.value = payload;
}
const applying = ref(null); // the law being applied for
function onApply({ law }) {
  applying.value = law;
}

const pendingClaims = computed(() => state.claims.filter((c) => c.bsn === profile.value?.bsn && c.status === 'PENDING'));
const properties = computed(() => persona.value?.properties ?? []);

// A law the engine refused to load (a type-check finding, RFC-037) is missing
// from every tile that depends on it. The refusal is shown here, not buried in
// the console, with the engine's own message per law.
const loadFailureText = computed(() => loadFailures.value.map((f) => `${f.id} (${f.path}): ${f.message}`).join(' — '));
</script>

<template>
  <nldd-page>
    <!-- No title bar above the heading: it repeated the portal's name right on
         top of the page heading that follows, and the tab in the app bar already
         names it. What the bar carried that nothing else did is the disclaimer,
         so that moves into the overline, where it stays next to the persona the
         visitor is logged in as. -->
    <nldd-simple-section width="1200px">
      <nldd-title slot="header" size="2">
        <span slot="overline">Ingelogd als {{ persona?.name ?? profile?.name }} · demo, geen echte overheidsdienst</span>
        <h1>{{ profile?.portal_heading }}</h1>
        <span slot="subtitle">{{ profile?.portal_subtitle }}</span>
        <!-- Tags naast elkaar: gap 8, dezelfde scheiding als de spacer-cells in de lijstrijen. -->
        <nldd-container slot="actions" layout="wrap" gap="8">
          <nldd-tag v-for="p in properties" :key="p" size="sm" :text="p"></nldd-tag>
          <nldd-tag v-if="profile?.kvk" size="sm" icon="building" :text="`KVK ${profile.kvk}`"></nldd-tag>
        </nldd-container>
      </nldd-title>
      <nldd-rich-text v-if="persona?.description" spacing="tight"><p><em>{{ persona.description }}</em></p></nldd-rich-text>
      <!-- The persona line above sets `spacing="tight"`, which strips the space
           under it, so a banner placed straight after touched it (measured: 0px
           between them). The banners get their own container with a gap. -->
      <nldd-container v-if="loadFailures.length || pendingClaims.length" padding-top="16" gap="12">
      <nldd-banner
        v-if="loadFailures.length"
        variant="critical"
        :text="`${loadFailures.length === 1 ? 'Eén wet is' : `${loadFailures.length} wetten zijn`} niet geladen`"
        :supporting-text="`De engine weigerde: ${loadFailureText}. Regelingen die hiervan afhangen kunnen geen uitkomst geven.`"
      ></nldd-banner>
      <nldd-banner
        v-if="pendingClaims.length"
        variant="accent"
        :text="`${pendingClaims.length} ${pendingClaims.length === 1 ? 'correctie wacht' : 'correcties wachten'} op beoordeling`"
        supporting-text="De regelingen hieronder rekenen al met wat u heeft opgegeven. Een behandelaar beoordeelt de correctie; pas daarna staat de uitkomst vast."
      ></nldd-banner>
      </nldd-container>
    </nldd-simple-section>

    <nldd-simple-section width="1200px" padding-top="0">
      <!-- `item-width` is een minimum, geen breedte: de collectie verdeelt haar
           1200px over zoveel kolommen als er passen en rekt de rest uit. Op 400px
           bleven er dus twee over van 588px elk. 360px geeft er drie, en drie
           tegels naast elkaar laat het oog rustig scannen. Ruimte hoort binnen de
           tegel (padding 20, gap 16), niet in de doos. -->
      <nldd-collection layout="grid" item-width="360px" max-items="60">
        <LawTile v-for="law in sortedLaws" :key="law.id" :law="law" @edit-value="onEditValue" @evaluated="onEvaluated" @apply="onApply" />
      </nldd-collection>
      <nldd-inline-dialog v-if="sortedLaws.length === 0" icon="inbox" text="Geen regelingen" supporting-text="Voor dit profiel zijn geen regelingen zichtbaar."></nldd-inline-dialog>
    </nldd-simple-section>

    <EditValueSheet :open="!!editing" :node="editing?.node ?? null" :tile-law-id="editing?.law?.id ?? null" :self-declared="!!editing?.selfDeclared" @close="editing = null" />
    <ApplicationSheet :open="!!applying" :law="applying" :evaluation="applying ? evaluations[applying.id] ?? null : null" @close="applying = null" @edit-value="onEditValue" />
  </nldd-page>
</template>

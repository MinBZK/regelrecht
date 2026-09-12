<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import OrgLogo from './OrgLogo.vue';
import DataLineage from './DataLineage.vue';
import { fieldSpec, formatMissing, formatValue, humanize, isUnknown, verdictOf } from '../data/format.js';
import { lineageFromTrace, leafValues } from '../data/lineage.js';
import { askedInputsFor, claimKeyFor, evaluationParamsFor, nextQuestions } from '../data/askedInputs.js';
import { dateInputFor, phraseOutcome, phrasingFor } from '../data/outcomePhrasing.js';
import { driftSentence } from '../data/caseDrift.js';
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
const { corpus, dataVersion, profile, personaParams, findCase, caseDrift, canSubmitClaims, activeDelegation } = demo;

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
/**
 * What the tile shows under the outcome.
 *
 * A citizen asks three things: do I get it, how much, and do I have to do
 * something. Everything else is justification, and that belongs one click away
 * (Berekening, Gebruikte gegevens, Wettekst) where it can also be corrected.
 * Showing every remaining output put the reasoning on the front page —
 * "Uitzondering caribisch nederland van toepassing: Nee" for someone living in
 * the Netherlands.
 *
 * `tile_details` in demo-config.yaml names the values that do matter, per law;
 * an empty list means the outcome speaks for itself. A law with no entry keeps
 * the old behaviour, so this grows law by law.
 */
const secondary = computed(() => {
  const rest = outputs.value.filter(([k]) => k !== primary.value?.name);
  const wanted = corpus.value?.config?.tile_details?.[`${props.law.service}/${props.law.law_path}`];
  if (!wanted) return rest.slice(0, 6);
  return wanted.map((name) => rest.find(([k]) => k === name)).filter(Boolean);
});

// The tile's sentence (see outcomePhrasing.js): a lead, the outcome big, and
// the unit after it — "Uw huurtoeslag is waarschijnlijk € 302,96 per jaar".
// Null for a law nobody wrote wording for, and then the general rendering
// below stands unchanged.
const phrasing = computed(() => phrasingFor(corpus.value?.config, props.law.service, props.law.law_path));
// The date a yes/no law is about ("de verkiezingen van 29 oktober 2025"). It
// is an input the law resolved, so it is in the lineage the tile already
// builds; absent, the lead degrades to a sentence without a date.
const outcomeDate = computed(() => {
  const name = dateInputFor(phrasing.value);
  if (!name) return null;
  const hit = leafValues(lineage.value).find((n) => n.name === name);
  return hit && !isUnknown(hit.value) && hit.value !== null ? formatValue(hit.value, null) : null;
});
const phrased = computed(() => {
  if (!primary.value) return null;
  return phraseOutcome(phrasing.value, {
    met: requirementsMet.value,
    // An unknown amount is not a number to put in a sentence; the general
    // rendering names what is missing, so leave it to that.
    value: isUnknown(primary.value.value) ? null : formatValue(primary.value.value, primary.value.spec),
    isYesNo: typeof primary.value.value === 'boolean',
    date: outcomeDate.value,
  });
});

const lineage = computed(() => {
  if (!evaluation.value?.trace) return [];
  return lineageFromTrace(evaluation.value.trace, props.law.id, evaluationParams());
});
const valueCount = computed(() => leafValues(lineage.value).length);

const currentCase = computed(() => findCase(props.law));
/**
 * Een lopende aanvraag die niet meer klopt met wat de wet nu zegt.
 *
 * De burger heeft ergens een gegeven gewijzigd — misschien bij een hele
 * andere regeling — en deze wet rekent daar al mee, terwijl de aanvraag nog
 * op het oude bedrag staat. Dat hoort hij te zien op de plek waar dat besluit
 * staat, niet pas als het geld anders binnenkomt.
 */
const drift = computed(() => caseDrift(props.law, evaluation.value));
const driftText = computed(() => driftSentence(drift.value, doc.value));

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
/**
 * The self-supplied values this tile shows: the ones already answered, plus the
 * ones the law actually ran into.
 *
 * Not every question a law could ask is a question for this person. The kieswet
 * declares "ingezetenschapsduur jaren" and "werkzaam in nederlandse openbare
 * dienst" for the exception that keeps Dutch nationals abroad enfranchised; for
 * someone living in the Netherlands the law never reaches them, and listing
 * them anyway asks for paperwork nobody needs and suggests the outcome is
 * incomplete when it is settled. `nextQuestions` already knows which ones the
 * engine hit (RFC-036 reports the facts it missed), so the tile follows that.
 */
const ownInputs = computed(() => {
  const answered = askedInputs.value.filter((a) => a.claim);
  const asked = missingInputs.value.filter((a) => !a.claim);
  return [...answered, ...asked];
});

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
/**
 * Correct an outcome of this law: the aggregates on the tile (the primary
 * amount and the secondary rows) are claims like any other value, so citizen
 * and caseworker can both dispute them, as they could in the POC.
 */
function correctOutcome(name, value) {
  const params = personaParams();
  emit('edit-value', {
    law: props.law,
    node: { kind: 'law', law: props.law.id, name, value, service: props.law.service, ...claimKeyFor(props.law, params) },
  });
}
const produces = computed(() => {
  for (const a of doc.value.articles ?? []) {
    const p = a.machine_readable?.execution?.produces;
    if (p) return p;
  }
  return null;
});
// Een machtiging zonder het recht om aanvragen in te dienen mag alleen kijken:
// dan verdwijnen de knoppen, niet alleen hun werking.
const canApply = computed(() => canSubmitClaims.value && evaluation.value?.ok && verdict.value === true && !currentCase.value && produces.value?.legal_character === 'BESCHIKKING');
/** A decided-and-rejected case the citizen has not objected to yet (Awb art. 6:5). */
const canObject = computed(() => canSubmitClaims.value && currentCase.value?.status === 'DECIDED' && currentCase.value?.approved === false && !currentCase.value?.objection);

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
    <nldd-container slot="header" padding="20" layout="row" gap="12" vertical-alignment="top">
      <OrgLogo :service="law.service" />
      <nldd-title-cell size="5" :text="law.name" :supporting-text="corpus.services[law.service]?.name ?? law.service"></nldd-title-cell>
      <nldd-tag v-if="statusTag" :color="statusTag.color" :text="statusTag.text" :icon="statusTag.icon" size="sm"></nldd-tag>
    </nldd-container>

    <nldd-container padding-inline="20" padding-bottom="20" gap="16">
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
        <!-- Wat er al lag klopt niet meer met wat de wet nu zegt. Geen
             herberekening die iets besluit: alleen de constatering, met de
             weg terug ernaast (de knop "Wijzigen" in de voettekst). Het
             besluit zelf blijft staan tot de burger zijn aanvraag wijzigt of
             een behandelaar ernaar kijkt. Boven de uitkomst en niet in plaats
             daarvan: het nieuwe bedrag is juist wat hij moet zien.

             Een banner en geen inline-dialog: die laatste is de lege-staat, met
             het icoon bóven de tekst en alles gecentreerd. Dat leest als een
             andere soort mededeling dan de rest van de tegel, terwijl dit er
             juist naast hoort te staan. De aanvraag gebruikt dezelfde banner,
             dus beide kanten zien hetzelfde. -->
        <nldd-banner
          v-if="drift"
          variant="warning"
          text="Uw aanvraag klopt niet meer"
          :supporting-text="driftText"
        ></nldd-banner>
        <nldd-inline-dialog v-if="verdict === 'unknown'" icon="info" text="Nog niet te bepalen" :supporting-text="`De wet kan met de bekende gegevens geen uitkomst geven; ${verdictMissing}.`"></nldd-inline-dialog>
        <nldd-list v-else variant="box-tinted" accessible-label="Uitkomst">
          <nldd-list-item size="md" :button="primary ? true : undefined" @click="primary && correctOutcome(primary.name, primary.value)">
            <nldd-icon-cell :icon="requirementsMet ? 'check-mark-circle' : 'dismiss-circle'" :color="requirementsMet ? 'success' : 'critical'"></nldd-icon-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <!-- The law's own sentence, when it has one: the lead as overline,
                 the outcome as the title, the unit under it. Same cell as the
                 general rendering below, so the pencil and the row keep working. -->
            <!-- Size follows what the headline is. With a lead it is a short
                 figure ("€ 406,95", "STEMRECHT") and carries the tile, so it is
                 large. Without one the headline IS the whole sentence ("U krijgt
                 waarschijnlijk geen kindgebonden budget."), and set that large it
                 shouts a non-result across three lines. -->
            <nldd-title-cell
              v-if="phrased"
              :size="phrased.lead ? '3' : '5'"
              :overline="phrased.lead || undefined"
              :text="phrased.headline"
              :supporting-text="phrased.unit || undefined"
            ></nldd-title-cell>
            <nldd-title-cell
              v-else
              size="4"
              :overline="requirementsMet ? 'U voldoet aan de voorwaarden' : 'U voldoet niet aan de voorwaarden'"
              :text="requirementsMet ? (primary ? formatValue(primary.value, primary.spec) : 'Ja') : 'Niet van toepassing'"
              :supporting-text="requirementsMet && primary ? (isUnknown(primary.value) ? `${humanize(primary.name)} · ${formatMissing(primary.value, { ownLaw: law.id, lawName })}` : humanize(primary.name)) : ''"
            ></nldd-title-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-list v-if="secondary.length" variant="simple" accessible-label="Overige uitkomsten">
          <nldd-list-item v-for="[name, value] in secondary" :key="name" size="sm" button @click="correctOutcome(name, value)">
            <nldd-text-cell size="sm" color="secondary" min-width="55%" :text="humanize(name)"></nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" max-width="45%" horizontal-alignment="right" :color="isUnknown(value) ? 'secondary' : 'default'" :text="formatValue(value, fieldSpec(doc, name))"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell icon="edit" size="16" color="secondary"></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>

        <!-- These rows are questions the registers cannot answer, so only the
             citizen can. Until one is answered it is not "door u opgegeven" —
             saying that of an empty row claims the person supplied something
             they never did. Unanswered reads as a question, answered says who
             gave the answer. -->
        <nldd-list v-if="ownInputs.length" variant="simple" accessible-label="Gegevens die u zelf opgeeft">
          <nldd-list-item v-for="input in ownInputs" :key="input.name" size="sm" button @click="supply(input)">
            <nldd-icon-cell :icon="input.claim ? 'edit' : 'question-mark-circle'" size="16" :color="input.claim ? 'accent' : 'secondary'"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell size="sm" :text="humanize(input.name)" :supporting-text="input.claim ? 'door u opgegeven' : 'alleen u kunt dit opgeven'"></nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="input.claim ? 'default' : 'secondary'" :text="input.claim ? formatValue(input.claim.newValue, input.spec) : 'nog niet opgegeven'"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <!-- Outlined, not tinted. The tinted box is the answer; giving the same
             fill to a link into the reasoning made the two read as equals, and
             the eye had nowhere to land. This one is a door, not a statement. -->
        <nldd-list type="tree" variant="box-base" accessible-label="Gebruikte gegevens">
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
         second line instead of a button breaking its label. The buttons stay direct
         children: a nested container has size containment and so no intrinsic width
         in a flex line.
         No flexible spacer between the primary and the secondary actions. It ate
         every leftover pixel, so a long primary label ("Gegevens aanvullen") pushed
         the row over its width and dropped "Wettekst" alone onto a second line,
         while a short one ("Aanvragen") kept all three together. The footer then
         looked different from tile to tile for no reason the reader can see. Left
         alignment with a plain gap wraps the same way for every label length.
         The primary label is kept short for the same reason: "Gegevens aanvullen"
         made the three buttons 374px wide in a 384px footer, so padding and gaps
         pushed "Wettekst" onto a second line while every neighbouring tile kept
         its buttons on one. The heading above the button already says which data
         is missing. -->
    <nldd-container slot="footer" padding="20" layout="wrap" gap="12" vertical-alignment="center">
      <!-- A rejected decision leads with the objection, not with the file. Awb
           art. 6:5 gives the citizen six weeks to disagree, and hiding that
           route one click deep behind the file made the tile a dead end: the POC
           put it in view. Once an objection is running the button goes back to
           the file, which is where its status is.
           Labels are one word for a measured reason: the three buttons have
           about 313px of a 384px footer before padding and gaps push the last
           one onto a second line. "Mijn aanvraag" needed 336px and "Bezwaar
           maken" 347px, so both wrapped; "Aanvraag" and "Bezwaar" fit. -->
      <nldd-button v-if="canObject" variant="primary" size="sm" start-icon="flag" text="Bezwaar" @click="apply"></nldd-button>
      <nldd-button v-else-if="drift" variant="primary" size="sm" start-icon="edit" text="Wijzigen" @click="apply"></nldd-button>
      <nldd-button v-else-if="currentCase" variant="secondary" size="sm" start-icon="file-text" text="Aanvraag" @click="apply"></nldd-button>
      <nldd-button v-else-if="canSubmitClaims && evaluation && missingInputs.length && produces?.legal_character === 'BESCHIKKING'" variant="primary" size="sm" start-icon="edit" text="Aanvullen" @click="apply"></nldd-button>
      <nldd-button v-else-if="canApply" variant="primary" size="sm" start-icon="paper-plane" text="Aanvragen" @click="apply"></nldd-button>
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

<script setup>
import { computed } from 'vue';
import { formatValue } from '../world/format.js';
import { resolveSource, TRACE_STEPS } from '../world/receipt.js';

// Eén stap uit de uitvoeringstrace, met de stappen eronder.
//
// Het component roept zichzelf aan, omdat een trace een boom is en het
// ontwerpsysteem een boom ook echt als boom neerzet: een tak legt zijn kinderen
// in zijn eigen `slot="children"`, en daaruit leidt hulptechnologie niveau en
// positie af (zie de documentatie van `nldd-list type="tree"`). Een platte lijst
// met inspringing zou er hetzelfde uitzien en niets van die structuur dragen.
//
// De inspringing is wél aan ons — dat zegt het ontwerpsysteem er met zoveel
// woorden bij — en die komt hier als één spacer per niveau.

const props = defineProps({
  /** De stap, zoals `receiptTrace` haar oplevert. */
  node: { type: Object, required: true },
  /** Hoe diep deze stap in de boom zit; bepaalt alleen de inspringing. */
  depth: { type: Number, default: 0 },
  /** De paden van de takken die uitgeklapt staan. */
  open: { type: Set, required: true },
});

const emit = defineEmits(['toggle']);

const expandable = computed(() => props.node.children.length > 0);
const expanded = computed(() => expandable.value && props.open.has(props.node.path));

/** Hoe deze soort stap heet en eruitziet. */
const step = computed(() => TRACE_STEPS[props.node.nodeType] ?? TRACE_STEPS.unknown);

/**
 * De regel onder de naam: waar deze stap vandaan komt.
 *
 * De regeling en het artikel staan voorop, want dat is waar een lezer een
 * besluit op naslaat. Ze staan er alleen als de engine ze vastlegde — een losse
 * rekenstap binnen een actie hoort bij het artikel erboven en krijgt hier geen
 * artikelnummer aangemeten dat de trace niet draagt.
 */
const source = computed(() => {
  const parts = [];
  if (props.node.regulation && props.node.article) {
    parts.push(`${props.node.regulation}, artikel ${props.node.article}`);
  } else if (props.node.regulation) {
    parts.push(props.node.regulation);
  }
  const origin = resolveSource(props.node.resolveType);
  if (origin) parts.push(`uit ${origin}`);
  if (props.node.message) parts.push(props.node.message);
  return parts.join(' · ');
});

/**
 * De uitkomst van deze stap.
 *
 * Draagt de stap er geen, dan staat dát er: een stap zonder uitkomst is iets
 * anders dan een stap die niets opleverde, en een lege cel zou die twee niet uit
 * elkaar houden.
 */
const result = computed(() => (props.node.hasResult ? formatValue(props.node.result) : '—'));

/**
 * Een tak open- of dichtdoen.
 *
 * Een klik in een tak eronder telt niet mee. Die bubbelt over deze rij heen, en
 * de rij is zelf de knop, dus zonder deze toets zou het openklappen van een
 * kindstap elke stap erboven dichtdoen. De toets kijkt naar de rij waarin de
 * klik begon: is dat niet deze rij, dan is hij niet voor ons.
 */
function toggle(event) {
  if (!expandable.value) return;
  if (event?.target?.closest?.('nldd-list-item') !== event?.currentTarget) return;
  emit('toggle', props.node.path);
}
</script>

<template>
  <nldd-list-item
    size="sm"
    :button="expandable || undefined"
    :expanded="expanded || undefined"
    @click="toggle"
  >
    <!-- Eén spacer per niveau: de boom zorgt voor de structuur, de inspringing
         is aan de gebruiker ervan. -->
    <nldd-spacer-cell v-for="level in depth" :key="level" size="16"></nldd-spacer-cell>
    <nldd-icon-cell :icon="step.icon" size="16" color="secondary"></nldd-icon-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-cell width="132px">
      <nldd-tag size="sm" :color="step.color" :text="step.label"></nldd-tag>
    </nldd-cell>
    <nldd-text-cell
      size="sm"
      min-width="200px"
      :text="node.name"
      :supporting-text="source"
    ></nldd-text-cell>
    <nldd-text-cell
      size="sm"
      width="fit-content"
      max-width="40%"
      horizontal-alignment="right"
      color="secondary"
      :text="result"
    ></nldd-text-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-icon-cell
      v-if="expandable"
      disclosure
      icon="chevron-right"
      size="16"
      color="secondary"
    ></nldd-icon-cell>

    <!-- Alleen een open tak bouwt haar kinderen op: een trace van honderden
         stappen zou anders in één keer in de DOM staan voor iets wat niemand
         ziet. -->
    <TraceNode
      v-for="child in expanded ? node.children : []"
      :key="child.path"
      slot="children"
      :node="child"
      :depth="depth + 1"
      :open="open"
      @toggle="emit('toggle', $event)"
    />
  </nldd-list-item>
</template>

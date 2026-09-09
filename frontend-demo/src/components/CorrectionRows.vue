<script setup>
import { fieldSpec, formatValue, humanize } from '../data/format.js';
import { useDemo } from '../store/demoStore.js';

// The rows of a corrections list in the case system: what the citizen (or the
// caseworker) changed, why, with what evidence, and the decision or the buttons
// to take it. Renders `nldd-list-item` rows only; the caller owns the list.
//
// Cells never shrink below their content, so a tag and two buttons beside the
// text would leave the text cell no width at all in a narrow list (one
// character per line). Each correction therefore takes up to three rows: the
// text (with a width floor) and at most one status tag; the clause tag and the
// evidence; and the decision buttons. The clause itself and the file size are
// in the supporting text, so the tags stay short enough for a 320px inspector.

const props = defineProps({
  claims: { type: Array, required: true },
  /** Optional: where a correction comes from (persona, law), for a list that spans cases. */
  origin: { type: Function, default: null },
});

const demo = useDemo();
const { corpus } = demo;

function spec(cl) {
  return fieldSpec(corpus.value?.lawById(cl.lawId)?.doc, cl.input);
}
function text(cl) {
  return `${humanize(cl.input)}: ${formatValue(cl.oldValue, spec(cl))} → **${formatValue(cl.newValue, spec(cl))}**`;
}
/** A citizen's correction the caseworker still has to approve or reject. */
function awaitsDecision(cl) {
  return cl.claimant !== 'BEHANDELAAR' && cl.status === 'PENDING';
}
function formatSize(bytes) {
  if (!Number.isFinite(bytes)) return '';
  return bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} kB`;
}
</script>

<template>
  <template v-for="cl in props.claims" :key="cl.id">
    <nldd-list-item size="sm">
      <nldd-text-cell size="sm" min-width="120px" :text="text(cl)">
        <span slot="supporting-text">
          <template v-if="props.origin">{{ props.origin(cl) }} · </template>{{ cl.reason }}
          <template v-if="cl.hardship?.clause"><br />Beroep op hardheidsclausule: {{ cl.hardship.clause }}</template>
          <template v-if="cl.evidence"><br />Bewijsstuk: {{ cl.evidence.name }} ({{ formatSize(cl.evidence.size) }})</template>
        </span>
      </nldd-text-cell>
      <!-- A caseworker's correction is approved by definition: one tag says both. -->
      <nldd-cell v-if="cl.claimant === 'BEHANDELAAR'"><nldd-tag size="sm" color="success" text="Door behandelaar"></nldd-tag></nldd-cell>
      <nldd-cell v-else-if="cl.status !== 'PENDING'"><nldd-tag size="sm" :color="cl.status === 'APPROVED' ? 'success' : 'critical'" :text="cl.status === 'APPROVED' ? 'Goedgekeurd' : 'Afgewezen'"></nldd-tag></nldd-cell>
    </nldd-list-item>
    <nldd-list-item v-if="cl.hardship?.clause || cl.evidence" size="sm">
      <nldd-cell v-if="cl.hardship?.clause"><nldd-tag size="sm" color="warning" text="Hardheidsclausule"></nldd-tag></nldd-cell>
      <nldd-cell v-if="cl.evidence">
        <nldd-button v-if="cl.evidence.dataUrl" size="sm" variant="neutral-transparent" start-icon="file" :text="cl.evidence.name" :href="cl.evidence.dataUrl" target="_blank" rel="noopener"></nldd-button>
        <nldd-tag v-else size="sm" color="neutral" icon="file" :text="cl.evidence.name"></nldd-tag>
      </nldd-cell>
    </nldd-list-item>
    <nldd-list-item v-if="awaitsDecision(cl)" size="sm">
      <nldd-cell width="full">
        <nldd-button-group orientation="horizontal" size="sm">
          <nldd-button size="sm" variant="primary" text="Goedkeuren" @click="demo.decideClaim(cl.id, true)"></nldd-button>
          <nldd-button size="sm" variant="secondary" text="Afwijzen" @click="demo.decideClaim(cl.id, false)"></nldd-button>
        </nldd-button-group>
      </nldd-cell>
    </nldd-list-item>
  </template>
</template>

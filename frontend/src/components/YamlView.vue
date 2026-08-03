<script setup>
import { computed, inject } from 'vue';
import * as yaml from 'js-yaml';
import MachineEmptyState from './MachineEmptyState.vue';

const props = defineProps({
  article: { type: Object, default: null },
  /** Read-only context: offer a button that opens the editor so the missing
   *  machine-readable version can be created there. */
  canCreate: { type: Boolean, default: false },
  /** Whether an enrichment can be requested here. The run covers the whole
   *  law, not just this article, so the empty state says so. */
  canEnrich: { type: Boolean, default: false },
  /** An enrichment for this law is already pending or processing. The empty
   *  state then reports that instead of offering the buttons again. */
  enriching: { type: Boolean, default: false },
  /** Message from the most recent failed enrich request, shown in place of
   *  the supporting text. */
  enrichError: { type: String, default: '' },
  /** Er staat een openstaande review-taak klaar voor deze wet. */
  reviewReady: { type: Boolean, default: false },
  /** Anchor target for that button. Leave unset when the user isn't logged
   *  in, so the click gates on the login popover instead of the href. */
  createHref: { type: String, default: undefined },
});

const emit = defineEmits([
  /** Create-button click. Payload is the button element, so the parent can
   *  anchor the login popover to it. */
  'create-mr',
  /** Ask for a generated proposal instead of writing one by hand. */
  'enrich',
  /** Open the task list filtered to this law, while a run is pending. */
  'view-tasks',
  /** Open review-modus voor de klaarstaande taak. */
  'review',
]);

// Provided by AppShell; see LibraryView's "Bewerken" button.
const onLoginTriggerPointerdown = inject('onLoginTriggerPointerdown', () => {});

const yamlText = computed(() => {
  const mr = props.article?.machine_readable;
  if (!mr) return null;
  return yaml.dump(mr, { lineWidth: 80, noRefs: true });
});
</script>

<template>
  <!-- Zelfde ingang als in de Machine-pane: die twee staan niet altijd allebei
       open. De supporting-text benoemt de scope, want de knop staat in een
       weergave die over één artikel gaat. -->
  <MachineEmptyState
    v-if="!yamlText"
    :review-ready="reviewReady"
    :enriching="enriching"
    :can-enrich="canEnrich"
    :can-create="canCreate"
    :create-href="createHref"
    :enrich-error="enrichError"
    @enrich="emit('enrich', $event)"
    @create="emit('create-mr', $event)"
    @view-tasks="emit('view-tasks')"
    @review="emit('review')"
  ></MachineEmptyState>
  <nldd-code-viewer v-else language="yaml">{{ yamlText }}</nldd-code-viewer>
</template>

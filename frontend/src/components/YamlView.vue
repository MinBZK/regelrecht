<script setup>
import { computed, inject } from 'vue';
import * as yaml from 'js-yaml';

const props = defineProps({
  article: { type: Object, default: null },
  /** Read-only context: offer a button that opens the editor so the missing
   *  machine-readable version can be created there. */
  canCreate: { type: Boolean, default: false },
  /** Anchor target for that button. Leave unset when the user isn't logged
   *  in, so the click gates on the login popover instead of the href. */
  createHref: { type: String, default: undefined },
});

const emit = defineEmits([
  /** Create-button click. Payload is the button element, so the parent can
   *  anchor the login popover to it. */
  'create-mr',
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
  <nldd-inline-dialog v-if="!yamlText" text="Geen machine-leesbare gegevens voor dit artikel">
    <nldd-button
      v-if="canCreate"
      slot="actions"
      variant="secondary"
      size="md"
      data-testid="create-mr-btn"
      text="Machine versie aanmaken"
      :href="createHref"
      @click.prevent="emit('create-mr', $event.currentTarget)"
      @pointerdown.capture="onLoginTriggerPointerdown"
    ></nldd-button>
  </nldd-inline-dialog>
  <nldd-code-viewer v-else language="yaml">{{ yamlText }}</nldd-code-viewer>
</template>

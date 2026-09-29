<script setup>
// The grams of the cell. The cell knows no login and no roles, but its
// chronicle is not open: the frontend reads it through the inspection of a
// process, as a handler (see api.js, inspectionApi).
import { inject, onMounted, ref } from 'vue';
import Grams from '../components/Grams.vue';

// The chronicle and the lexostatuses of the cell, through the inspection of a process.
const api = inject('cellApi');

const props = defineProps({
  // The gram just recorded, to point it out in the table.
  highlighted: { type: Object, default: null },
  portal: { type: Boolean, default: false },
});

const items = ref([]);
const error = ref('');
const loaded = ref(false);

onMounted(async () => {
  try {
    items.value = (await api.chronicle()).reverse();
  } catch (e) {
    error.value = e.message;
  } finally {
    loaded.value = true;
  }
});
</script>

<template>
  <nldd-title size="2"><h1>Kroniek</h1></nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="error">
    <nldd-inline-dialog variant="alert" text="De kroniek is niet te laden" :supporting-text="error"></nldd-inline-dialog>
  </template>
  <Grams
    v-else-if="loaded"
    :items="items"
    :highlighted="props.highlighted"
    :empty-text="props.portal ? 'Een ingediende aanvraag verschijnt hier als gram.' : undefined"
  />
</template>

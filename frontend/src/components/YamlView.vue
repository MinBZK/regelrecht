<script setup>
import { computed } from 'vue';
import * as yaml from 'js-yaml';

const props = defineProps({
  article: { type: Object, default: null },
});

const yamlText = computed(() => {
  const mr = props.article?.machine_readable;
  if (!mr) return null;
  return yaml.dump(mr, { lineWidth: 80, noRefs: true });
});
</script>

<template>
  <nldd-inline-dialog v-if="!yamlText" text="Geen machine-leesbare gegevens voor dit artikel"></nldd-inline-dialog>
  <nldd-code-viewer v-else language="yaml">{{ yamlText }}</nldd-code-viewer>
</template>

<script setup>
import { computed } from 'vue';
import yaml from 'js-yaml';

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
  <pre v-else class="yaml-source"><code>{{ yamlText }}</code></pre>
</template>

<style>
.yaml-source {
  background: var(--semantics-surfaces-tinted-background-color, #F4F6F9);
  border-radius: 12px;
  padding: 16px;
  font-size: 13px;
  line-height: 1.5;
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
}
</style>

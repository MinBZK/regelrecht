<script setup>
// The YAML fragment the cell loaded for a source (a step of the "waarom?",
// a node of the map): file, lines and the YAML. Fetched through the
// `fragment` cache (see why.js) that the page provides, once per source.
import { inject, ref, watch } from 'vue';

const props = defineProps({
  // A SourceRef: {law: "<regulation>#<article>"} or {config, anchor}.
  source: { type: Object, required: true },
});

// fragment(source) -> Promise<{file, line, end_line, yaml} | null>.
const fragment = inject('fragment');
const shown = ref(null);
const loading = ref(false);
const error = ref('');

watch(
  () => props.source,
  async (source) => {
    shown.value = null;
    error.value = '';
    loading.value = true;
    try {
      const f = await fragment(source);
      if (source === props.source) shown.value = f;
    } catch (e) {
      if (source === props.source) error.value = e.message;
    } finally {
      if (source === props.source) loading.value = false;
    }
  },
  { immediate: true },
);
</script>

<template>
  <nldd-activity-indicator v-if="loading" size="24" text="Fragment laden"></nldd-activity-indicator>
  <nldd-inline-dialog v-else-if="error" variant="alert" text="Fragment niet te laden" :supporting-text="error"></nldd-inline-dialog>
  <template v-else-if="shown">
    <nldd-rich-text><p>{{ shown.file }}, regel {{ shown.line }}–{{ shown.end_line }}</p></nldd-rich-text>
    <nldd-code-viewer language="yaml" wrap>{{ shown.yaml }}</nldd-code-viewer>
  </template>
</template>

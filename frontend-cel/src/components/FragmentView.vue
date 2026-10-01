<script setup>
// The YAML fragment the cell loaded for a source (a step of the "waarom?",
// a node of the map): file, lines and the YAML. Fetched through the
// `fragment` cache (see why.js) that the page provides, once per source.
// The file name opens the whole file the fragment is in, and closes it again.
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
// The fragment as a whole file, or null while only the block is shown.
const whole = ref(null);

async function load(source, into) {
  error.value = '';
  loading.value = true;
  try {
    const f = await fragment(source);
    if (source.of === props.source) into.value = f;
  } catch (e) {
    if (source.of === props.source) error.value = e.message;
  } finally {
    if (source.of === props.source) loading.value = false;
  }
}

watch(
  () => props.source,
  (source) => {
    shown.value = null;
    whole.value = null;
    load({ ...source, of: source }, shown);
  },
  { immediate: true },
);

function toggleWhole() {
  if (whole.value) whole.value = null;
  else load({ ...props.source, whole: true, of: props.source }, whole);
}
</script>

<template>
  <nldd-activity-indicator v-if="loading" size="24" text="Fragment laden"></nldd-activity-indicator>
  <nldd-inline-dialog v-if="error" variant="alert" text="Fragment niet te laden" :supporting-text="error"></nldd-inline-dialog>
  <template v-if="shown">
    <nldd-container layout="row" gap="8" vertical-alignment="center">
      <nldd-button
        type="button"
        variant="neutral-tinted"
        size="sm"
        :end-icon="whole ? 'chevron-up' : 'chevron-down'"
        :text="shown.file"
        :expanded="Boolean(whole) || undefined"
        :accessible-label="`${whole ? 'Verberg' : 'Toon'} het hele bestand ${shown.file}`"
        @click="toggleWhole"
      ></nldd-button>
      <nldd-rich-text>
        <p>{{ whole ? `hele bestand; het fragment staat op regel ${shown.line}–${shown.end_line}` : `regel ${shown.line}–${shown.end_line}` }}</p>
      </nldd-rich-text>
    </nldd-container>
    <nldd-code-viewer language="yaml" wrap>{{ (whole ?? shown).yaml }}</nldd-code-viewer>
  </template>
  <nldd-rich-text v-else-if="!loading && !error"><p>Geen fragment voor deze knoop.</p></nldd-rich-text>
</template>

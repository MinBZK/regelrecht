<script setup>
// A chain of steps of the "waarom?", numbered, as the cell gives them: per
// step where it is written and the reason. "Toon YAML" fetches the fragment
// the cell loaded (once, see why.js) and shows it with file and lines.
import { inject, ref } from 'vue';
import { fragmentPath, stepLabel } from '../why.js';

defineProps({
  steps: { type: Array, default: () => [] },
});

// fragment(source) -> Promise<{file, line, end_line, yaml}>, provided by
// ApplicationView.
const fragment = inject('fragment');
const open = ref({});
const error = ref({});

async function toggle(i, step) {
  if (open.value[i]) {
    open.value = { ...open.value, [i]: null };
    return;
  }
  try {
    open.value = { ...open.value, [i]: await fragment(step.source) };
    error.value = { ...error.value, [i]: '' };
  } catch (e) {
    error.value = { ...error.value, [i]: e.message };
  }
}
</script>

<template>
  <nldd-list>
    <nldd-list-item v-for="(s, i) in steps" :key="i">
      <nldd-container gap="8" padding-block="8">
        <nldd-rich-text>
          <p><strong>{{ i + 1 }}. {{ stepLabel(s) }}</strong>: {{ s.reason }}</p>
        </nldd-rich-text>
        <nldd-container v-if="fragmentPath(s.source)" layout="row">
          <nldd-button
            type="button"
            variant="neutral-tinted"
            size="sm"
            :text="open[i] ? 'Verberg YAML' : 'Toon YAML'"
            @click="toggle(i, s)"
          ></nldd-button>
        </nldd-container>
        <template v-if="open[i]">
          <nldd-rich-text><p>{{ open[i].file }}, regel {{ open[i].line }}–{{ open[i].end_line }}</p></nldd-rich-text>
          <nldd-code-viewer language="yaml" wrap>{{ open[i].yaml }}</nldd-code-viewer>
        </template>
        <nldd-inline-dialog
          v-if="error[i]"
          variant="alert"
          text="Fragment niet te laden"
          :supporting-text="error[i]"
        ></nldd-inline-dialog>
      </nldd-container>
    </nldd-list-item>
  </nldd-list>
</template>

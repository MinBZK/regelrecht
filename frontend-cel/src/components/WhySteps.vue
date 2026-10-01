<script setup>
// A chain of steps of the "waarom?", numbered, as the cell gives them: per
// step where it is written and the reason. "Toon YAML" shows the fragment
// the cell loaded (FragmentView) with file and lines.
import { inject, ref } from 'vue';
import { fragmentPath, stepLabel } from '../why.js';
import FragmentView from './FragmentView.vue';

defineProps({
  steps: { type: Array, default: () => [] },
});

// Provided by ProcessView; without it there is no YAML to show.
const fragment = inject('fragment', null);
const open = ref({});

function toggle(i) {
  open.value = { ...open.value, [i]: !open.value[i] };
}
</script>

<template>
  <nldd-list>
    <nldd-list-item v-for="(s, i) in steps" :key="i">
      <nldd-container gap="8" padding-block="8">
        <nldd-rich-text>
          <p><strong>{{ i + 1 }}. {{ stepLabel(s) }}</strong>: {{ s.reason }}</p>
        </nldd-rich-text>
        <nldd-container v-if="fragment && fragmentPath(s.source)" layout="row">
          <nldd-button
            type="button"
            variant="neutral-tinted"
            size="sm"
            :text="open[i] ? 'Verberg YAML' : 'Toon YAML'"
            :accessible-label="`${open[i] ? 'Verberg' : 'Toon'} YAML van ${stepLabel(s)}`"
            :expanded="Boolean(open[i])"
            @click="toggle(i)"
          ></nldd-button>
        </nldd-container>
        <FragmentView v-if="open[i]" :source="s.source" />
      </nldd-container>
    </nldd-list-item>
  </nldd-list>
</template>

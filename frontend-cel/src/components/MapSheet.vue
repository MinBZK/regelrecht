<script setup>
// The fragment of a node of the map: the YAML the cell loaded, with file and
// lines (FragmentView). A sheet and not a modal: the fragment is secondary
// content next to the map. The open state is mirrored to the imperative API
// of the sheet, so it animates; @close sets it back through `close`.
import { computed, nextTick, ref, watch } from 'vue';
import { KIND_TEXT, nodeText } from '../map.js';
import FragmentView from './FragmentView.vue';

const props = defineProps({ node: { type: Object, default: null } });
const emit = defineEmits(['close']);
const sheetEl = ref(null);
// The last node shown, so the sheet keeps its content while it closes.
const shown = ref(null);
const label = computed(() => (shown.value ? `Fragment van ${nodeText(shown.value)}` : 'Fragment'));

watch(
  () => props.node,
  async (n) => {
    if (!n) {
      sheetEl.value?.hide();
      return;
    }
    shown.value = n;
    await nextTick();
    sheetEl.value?.show();
  },
);
</script>

<template>
  <nldd-sheet ref="sheetEl" placement="right" width="760px" :accessible-label="label" @close="emit('close')">
    <nldd-page v-if="shown">
      <nldd-container padding="24" gap="16">
        <nldd-title :size="3">
          <span slot="overline">{{ KIND_TEXT[shown.kind] ?? shown.kind }}</span>
          <h2>{{ shown.kind === 'article' ? `${shown.regulation} art. ${shown.label}` : shown.label }}</h2>
        </nldd-title>
        <FragmentView :source="shown.source" />
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>

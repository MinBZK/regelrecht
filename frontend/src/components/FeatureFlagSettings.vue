<script setup>
import { ref, watch } from 'vue';
import { useFeatureFlags } from '../composables/useFeatureFlags.js';

const props = defineProps({
  open: { type: Boolean, default: false },
});
const emit = defineEmits(['close']);

const { flags, toggle } = useFeatureFlags();

const panelFlags = [
  ['panel.article_text', 'Wettekst'],
  ['panel.scenario_form', 'Scenario formulier'],
  ['panel.yaml_editor', 'YAML editor'],
  ['panel.execution_trace', 'Resultaat'],
  ['panel.machine_readable', 'Machine readable'],
];

const sheetEl = ref(null);

watch(() => props.open, (val) => {
  if (!sheetEl.value) return;
  if (val) {
    sheetEl.value.showSheet();
  } else {
    sheetEl.value.close();
  }
});
</script>

<template>
  <rr-sheet ref="sheetEl" placement="right" @close="emit('close')">
    <div class="settings-content">
      <rr-toolbar size="md">
        <rr-toolbar-start-area>
          <rr-toolbar-item>
            <rr-title-bar size="4">Instellingen</rr-title-bar>
          </rr-toolbar-item>
        </rr-toolbar-start-area>
        <rr-toolbar-end-area>
          <rr-toolbar-item>
            <rr-icon-button variant="neutral-plain" size="m" icon="dismiss" @click="emit('close')"></rr-icon-button>
          </rr-toolbar-item>
        </rr-toolbar-end-area>
      </rr-toolbar>

      <rr-simple-section>
        <rr-title-bar size="5" style="margin-bottom: 8px;">Panelen</rr-title-bar>
        <rr-list variant="box">
          <rr-list-item v-for="[key, label] in panelFlags" :key="key" size="md">
            <rr-text-cell>{{ label }}</rr-text-cell>
            <rr-cell>
              <rr-switch
                :checked="flags[key] ? true : undefined"
                @change="toggle(key)"
              ></rr-switch>
            </rr-cell>
          </rr-list-item>
        </rr-list>
      </rr-simple-section>
    </div>
  </rr-sheet>
</template>

<style scoped>
.settings-content {
  width: 320px;
  min-height: 100%;
}
</style>

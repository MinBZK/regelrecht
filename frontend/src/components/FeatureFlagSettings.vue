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
];

const sheetEl = ref(null);

watch(() => props.open, (val) => {
  if (!sheetEl.value) return;
  if (val) {
    sheetEl.value.show();
  } else {
    sheetEl.value.close();
  }
});
</script>

<template>
  <ndd-sheet ref="sheetEl" placement="right" @close="emit('close')">
    <div class="settings-content">
      <ndd-toolbar size="md">
        <ndd-toolbar-item slot="start">
          <ndd-title-bar size="4" text="Instellingen"></ndd-title-bar>
        </ndd-toolbar-item>
        <ndd-toolbar-item slot="end">
          <ndd-icon-button variant="neutral-plain" size="md" icon="dismiss" @click="emit('close')"></ndd-icon-button>
        </ndd-toolbar-item>
      </ndd-toolbar>

      <ndd-simple-section>
        <ndd-title-bar size="5" text="Panelen" style="margin-bottom: 8px;"></ndd-title-bar>
        <ndd-list variant="box">
          <ndd-list-item v-for="[key, label] in panelFlags" :key="key" size="md">
            <ndd-text-cell :text="label"></ndd-text-cell>
            <ndd-cell>
              <ndd-switch
                :checked="flags[key] ? true : undefined"
                @change="toggle(key)"
              ></ndd-switch>
            </ndd-cell>
          </ndd-list-item>
        </ndd-list>
      </ndd-simple-section>
    </div>
  </ndd-sheet>
</template>

<style scoped>
.settings-content {
  width: 320px;
  min-height: 100%;
}
</style>

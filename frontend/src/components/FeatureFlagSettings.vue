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
    sheetEl.value.show();
  } else {
    sheetEl.value.close();
  }
});
</script>

<template>
  <nldd-sheet ref="sheetEl" placement="right" @close="emit('close')">
    <div class="settings-content">
      <nldd-toolbar size="md">
        <nldd-toolbar-item slot="start">
          <nldd-title-bar size="4" text="Instellingen"></nldd-title-bar>
        </nldd-toolbar-item>
        <nldd-toolbar-item slot="end">
          <nldd-icon-button variant="neutral-plain" size="md" icon="dismiss" @click="emit('close')"></nldd-icon-button>
        </nldd-toolbar-item>
      </nldd-toolbar>

      <nldd-simple-section>
        <nldd-title-bar size="5" text="Panelen" style="margin-bottom: 8px;"></nldd-title-bar>
        <nldd-list variant="box">
          <nldd-list-item v-for="[key, label] in panelFlags" :key="key" size="md">
            <nldd-text-cell :text="label"></nldd-text-cell>
            <nldd-cell>
              <nldd-switch
                :checked="flags[key] ? true : undefined"
                @change="toggle(key)"
              ></nldd-switch>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
      </nldd-simple-section>
    </div>
  </nldd-sheet>
</template>

<style scoped>
.settings-content {
  width: 320px;
  min-height: 100%;
}
</style>

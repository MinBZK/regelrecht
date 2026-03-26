<script setup>
import { ref, watch, onMounted, onUnmounted } from 'vue';

const props = defineProps({
  item: { type: Object, default: null },
});

const emit = defineEmits(['save', 'close']);

const values = ref({});

const typeOptions = ['string', 'number', 'boolean', 'amount'];

// Type inference for controls
function inferControlType(value, unit) {
  if (typeof value === 'boolean') return 'boolean';
  if (unit === 'eurocent') return 'currency';
  if (typeof value === 'number' && value > 0 && value < 1 && !unit) return 'percentage';
  if (typeof value === 'number') return 'number';
  return 'text';
}

function toDisplay(value, controlType) {
  if (controlType === 'currency') return +(value / 100).toFixed(2);
  if (controlType === 'percentage') return +(value * 100).toFixed(6);
  return value;
}

function fromDisplay(value, controlType) {
  if (controlType === 'currency') return Math.round(value * 100);
  if (controlType === 'percentage') return parseFloat((value / 100).toPrecision(15));
  return value;
}

watch(() => props.item, (item) => {
  if (!item) return;
  const s = item.section;
  if (s === 'definition' || s === 'add-definition') {
    const val = item.rawDef != null ? (typeof item.rawDef === 'object' ? item.rawDef.value : item.rawDef) : 0;
    const unit = item.rawDef != null && typeof item.rawDef === 'object' ? item.rawDef.type_spec?.unit : undefined;
    const ct = item.isNew ? 'number' : inferControlType(val, unit);
    values.value = {
      name: item.key ?? '',
      displayValue: item.isNew ? 0 : toDisplay(val, ct),
      controlType: ct,
      unit,
      rawDef: item.rawDef != null ? JSON.parse(JSON.stringify(item.rawDef)) : { value: 0 },
    };
  } else if (s === 'parameter' || s === 'add-parameter') {
    values.value = {
      name: item.data?.name ?? '',
      type: item.data?.type ?? 'string',
      required: item.data?.required ?? false,
    };
  } else if (s === 'input' || s === 'add-input') {
    values.value = {
      name: item.data?.name ?? '',
      type: item.data?.type ?? 'string',
      sourceRegulation: item.data?.source?.regulation ?? '',
      sourceOutput: item.data?.source?.output ?? '',
    };
  } else if (s === 'output' || s === 'add-output') {
    values.value = {
      name: item.data?.name ?? '',
      type: item.data?.type ?? 'string',
    };
  }
}, { immediate: true });

function save() {
  const item = props.item;
  if (!item) return;
  const s = item.section;

  if (s === 'definition' || s === 'add-definition') {
    const { name, displayValue, controlType, rawDef } = values.value;
    if (!name.trim()) return;
    const stored = controlType === 'boolean' ? displayValue : fromDisplay(Number(displayValue), controlType);
    const data = typeof rawDef === 'object' ? { ...rawDef, value: stored } : stored;
    if (s === 'definition') {
      emit('save', { section: 'definition', key: item.key, newKey: name.trim(), data });
    } else {
      emit('save', { section: 'add-definition', key: name.trim(), data });
    }
  } else if (s === 'parameter' || s === 'add-parameter') {
    const { name, type, required } = values.value;
    if (!name.trim()) return;
    if (s === 'parameter') {
      emit('save', { section: 'parameter', index: item.index, data: { name: name.trim(), type, required } });
    } else {
      emit('save', { section: 'add-parameter', data: { name: name.trim(), type, required } });
    }
  } else if (s === 'input' || s === 'add-input') {
    const { name, type, sourceRegulation, sourceOutput } = values.value;
    if (!name.trim()) return;
    const data = { name: name.trim(), type };
    if (sourceRegulation || sourceOutput) {
      data.source = {};
      if (sourceRegulation) data.source.regulation = sourceRegulation;
      if (sourceOutput) data.source.output = sourceOutput;
      if (item.data?.source?.parameters) data.source.parameters = item.data.source.parameters;
    }
    if (type === 'amount' && item.data?.type_spec) data.type_spec = item.data.type_spec;
    if (s === 'input') {
      emit('save', { section: 'input', index: item.index, data });
    } else {
      emit('save', { section: 'add-input', data });
    }
  } else if (s === 'output' || s === 'add-output') {
    const { name, type } = values.value;
    if (!name.trim()) return;
    const data = { name: name.trim(), type };
    if (type === 'amount' && item.data?.type_spec) data.type_spec = item.data.type_spec;
    if (s === 'output') {
      emit('save', { section: 'output', index: item.index, data });
    } else {
      emit('save', { section: 'add-output', data });
    }
  }

  emit('close');
}

const sectionLabels = {
  'definition': 'Definitie',
  'add-definition': 'Nieuwe definitie',
  'parameter': 'Parameter',
  'add-parameter': 'Nieuwe parameter',
  'input': 'Input',
  'add-input': 'Nieuwe input',
  'output': 'Output',
  'add-output': 'Nieuwe output',
};

function handleKeydown(e) {
  if (e.key === 'Escape' && props.item) emit('close');
}

onMounted(() => document.addEventListener('keydown', handleKeydown));
onUnmounted(() => document.removeEventListener('keydown', handleKeydown));
</script>

<template>
  <div v-if="item" class="edit-sheet-overlay">
    <div class="edit-sheet-backdrop" @click="emit('close')"></div>
    <div class="edit-sheet-panel">
      <!-- Header -->
      <rr-toolbar size="md">
        <rr-toolbar-start-area>
          <rr-toolbar-item>
            <span class="edit-sheet-title">{{ sectionLabels[item.section] || 'Bewerk' }}</span>
          </rr-toolbar-item>
        </rr-toolbar-start-area>
        <rr-toolbar-end-area>
          <rr-toolbar-item>
            <rr-button variant="accent-transparent" size="md" @click="emit('close')">Annuleer</rr-button>
          </rr-toolbar-item>
        </rr-toolbar-end-area>
      </rr-toolbar>

      <!-- Body -->
      <div class="edit-sheet-body">
        <rr-simple-section>
          <!-- Definition -->
          <template v-if="item.section === 'definition' || item.section === 'add-definition'">
            <rr-list variant="box" class="settings-list">
              <rr-list-item size="md">
                <rr-label-cell>Naam</rr-label-cell>
                <rr-button-cell slot="end">
                  <rr-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></rr-text-field>
                </rr-button-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-label-cell>Waarde</rr-label-cell>
                <rr-button-cell slot="end">
                  <div v-if="values.controlType === 'currency'" class="edit-sheet-value-group">
                    <span class="edit-sheet-unit">&euro;</span>
                    <input type="number" step="0.01" v-model.number="values.displayValue" class="edit-sheet-input">
                  </div>
                  <div v-else-if="values.controlType === 'percentage'" class="edit-sheet-value-group">
                    <input type="number" step="0.001" v-model.number="values.displayValue" class="edit-sheet-input">
                    <span class="edit-sheet-unit">%</span>
                  </div>
                  <input v-else-if="values.controlType === 'boolean'" type="checkbox" v-model="values.displayValue" class="edit-sheet-checkbox">
                  <input v-else-if="values.controlType === 'number'" type="number" v-model.number="values.displayValue" class="edit-sheet-input edit-sheet-input--full">
                  <input v-else type="text" v-model="values.displayValue" class="edit-sheet-input edit-sheet-input--full">
                </rr-button-cell>
              </rr-list-item>
            </rr-list>
          </template>

          <!-- Parameter -->
          <template v-if="item.section === 'parameter' || item.section === 'add-parameter'">
            <rr-list variant="box" class="settings-list">
              <rr-list-item size="md">
                <rr-label-cell>Naam</rr-label-cell>
                <rr-button-cell slot="end">
                  <rr-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></rr-text-field>
                </rr-button-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-label-cell>Type</rr-label-cell>
                <rr-button-cell slot="end">
                  <select v-model="values.type" class="edit-sheet-select">
                    <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                  </select>
                </rr-button-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-label-cell>Verplicht</rr-label-cell>
                <rr-button-cell slot="end">
                  <rr-switch :checked="values.required ? true : undefined" @change="values.required = Boolean($event.detail?.checked)"></rr-switch>
                </rr-button-cell>
              </rr-list-item>
            </rr-list>
          </template>

          <!-- Input -->
          <template v-if="item.section === 'input' || item.section === 'add-input'">
            <rr-list variant="box" class="settings-list">
              <rr-list-item size="md">
                <rr-label-cell>Naam</rr-label-cell>
                <rr-button-cell slot="end">
                  <rr-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></rr-text-field>
                </rr-button-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-label-cell>Type</rr-label-cell>
                <rr-button-cell slot="end">
                  <select v-model="values.type" class="edit-sheet-select">
                    <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                  </select>
                </rr-button-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-label-cell>Bron regelgeving</rr-label-cell>
                <rr-button-cell slot="end">
                  <rr-text-field size="md" :value="values.sourceRegulation" @input="values.sourceRegulation = $event.target?.value ?? $event.detail?.value ?? values.sourceRegulation"></rr-text-field>
                </rr-button-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-label-cell>Bron output</rr-label-cell>
                <rr-button-cell slot="end">
                  <rr-text-field size="md" :value="values.sourceOutput" @input="values.sourceOutput = $event.target?.value ?? $event.detail?.value ?? values.sourceOutput"></rr-text-field>
                </rr-button-cell>
              </rr-list-item>
            </rr-list>
          </template>

          <!-- Output -->
          <template v-if="item.section === 'output' || item.section === 'add-output'">
            <rr-list variant="box" class="settings-list">
              <rr-list-item size="md">
                <rr-label-cell>Naam</rr-label-cell>
                <rr-button-cell slot="end">
                  <rr-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></rr-text-field>
                </rr-button-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-label-cell>Type</rr-label-cell>
                <rr-button-cell slot="end">
                  <select v-model="values.type" class="edit-sheet-select">
                    <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                  </select>
                </rr-button-cell>
              </rr-list-item>
            </rr-list>
          </template>
        </rr-simple-section>
      </div>

      <!-- Footer -->
      <div class="edit-sheet-footer">
        <rr-button variant="accent-filled" size="md" style="width: 100%;" @click="save">
          Opslaan
        </rr-button>
      </div>
    </div>
  </div>
</template>

<style>
.edit-sheet-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  justify-content: flex-end;
}
.edit-sheet-backdrop {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.1);
}
.edit-sheet-panel {
  position: relative;
  width: 480px;
  background: #fff;
  display: flex;
  flex-direction: column;
  height: 100%;
  box-shadow: 0px 16px 64px 0px rgba(0, 0, 0, 0.11),
              0px 8px 32px 0px rgba(0, 0, 0, 0.09),
              0px 4px 16px 0px rgba(0, 0, 0, 0.06),
              0px 2px 8px 0px rgba(0, 0, 0, 0.04),
              0px 1px 4px 0px rgba(0, 0, 0, 0.03),
              0px 0px 2px 0px rgba(0, 0, 0, 0.02);
}
.edit-sheet-title {
  font-family: var(--rr-font-family-title, 'RijksSansVF', sans-serif);
  font-weight: 550;
  font-size: 20px;
  line-height: 1.4;
  color: var(--semantics-text-primary-color, #333B44);
}
.edit-sheet-body {
  flex: 1;
  overflow-y: auto;
}
.edit-sheet-footer {
  padding: 0 16px 16px;
}

/* Form fields */
.edit-sheet-panel .settings-list rr-list-item {
  display: grid;
  grid-template-columns: 120px 1fr;
  gap: 0 12px;
  align-items: center;
}
.edit-sheet-panel .settings-list rr-button-cell {
  width: 100%;
}
.edit-sheet-panel .settings-list rr-text-field {
  width: 100%;
}
.edit-sheet-input {
  padding: 8px 12px;
  border: 1px solid var(--semantics-dividers-color, #D1D5DB);
  border-radius: 8px;
  font-size: 14px;
  font-family: inherit;
  background: white;
  color: var(--semantics-text-primary-color, #333B44);
  outline: none;
  min-width: 0;
}
.edit-sheet-input:focus {
  border-color: #154273;
  box-shadow: 0 0 0 2px rgba(21, 66, 115, 0.15);
}
.edit-sheet-input--full {
  width: 100%;
}
.edit-sheet-value-group {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}
.edit-sheet-value-group .edit-sheet-input {
  flex: 1;
  min-width: 0;
}
.edit-sheet-unit {
  font-size: 14px;
  font-weight: 500;
  color: var(--semantics-text-secondary-color, #6B7280);
  flex-shrink: 0;
}
.edit-sheet-select {
  width: 100%;
  padding: 8px 12px;
  border: 1px solid var(--semantics-dividers-color, #D1D5DB);
  border-radius: 8px;
  font-size: 14px;
  font-family: inherit;
  background: white;
  color: var(--semantics-text-primary-color, #333B44);
  outline: none;
  appearance: auto;
}
.edit-sheet-select:focus {
  border-color: #154273;
  box-shadow: 0 0 0 2px rgba(21, 66, 115, 0.15);
}
.edit-sheet-checkbox {
  width: 20px;
  height: 20px;
  accent-color: #154273;
}
</style>

<script setup>
import { ref, watch, nextTick } from 'vue';

const props = defineProps({
  item: { type: Object, default: null },
});

const emit = defineEmits(['save', 'close']);

const sheetEl = ref(null);
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

watch(() => props.item, async (item) => {
  if (!item) {
    sheetEl.value?.hide();
    return;
  }

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

  await nextTick();
  sheetEl.value?.show();
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
</script>

<template>
  <rr-sheet ref="sheetEl" placement="right" @close="emit('close')">
    <div class="edit-sheet-content">
      <!-- Header -->
      <rr-toolbar size="md">
        <rr-toolbar-start-area>
          <rr-toolbar-item>
            <rr-title-bar v-if="item" size="4">{{ sectionLabels[item.section] || 'Bewerk' }}</rr-title-bar>
          </rr-toolbar-item>
        </rr-toolbar-start-area>
        <rr-toolbar-end-area>
          <rr-toolbar-item>
            <rr-button variant="accent-transparent" size="md" @click="emit('close')">Annuleer</rr-button>
          </rr-toolbar-item>
        </rr-toolbar-end-area>
      </rr-toolbar>

      <!-- Body -->
      <div class="edit-sheet-body" v-if="item">
        <rr-simple-section>
          <!-- Definition -->
          <template v-if="item.section === 'definition' || item.section === 'add-definition'">
            <rr-list variant="box" class="settings-list">
              <rr-list-item size="md">
                <rr-text-cell>Naam</rr-text-cell>
                <rr-cell>
                  <rr-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></rr-text-field>
                </rr-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-text-cell>Waarde</rr-text-cell>
                <rr-cell>
                  <div v-if="values.controlType === 'currency'" class="edit-sheet-value-group">
                    <span class="edit-sheet-unit">&euro;</span>
                    <rr-number-field :value="values.displayValue" step="0.01" full-width @change="values.displayValue = $event.detail?.value ?? values.displayValue"></rr-number-field>
                  </div>
                  <div v-else-if="values.controlType === 'percentage'" class="edit-sheet-value-group">
                    <rr-number-field :value="values.displayValue" step="0.001" full-width @change="values.displayValue = $event.detail?.value ?? values.displayValue"></rr-number-field>
                    <span class="edit-sheet-unit">%</span>
                  </div>
                  <rr-switch-field v-else-if="values.controlType === 'boolean'" :checked="values.displayValue ? true : undefined" @change="values.displayValue = Boolean($event.detail?.checked)">Waarde</rr-switch-field>
                  <rr-number-field v-else-if="values.controlType === 'number'" :value="values.displayValue" full-width hide-spin-buttons @change="values.displayValue = $event.detail?.value ?? values.displayValue"></rr-number-field>
                  <rr-text-field v-else size="md" :value="String(values.displayValue)" @input="values.displayValue = $event.target?.value ?? $event.detail?.value ?? values.displayValue"></rr-text-field>
                </rr-cell>
              </rr-list-item>
            </rr-list>
          </template>

          <!-- Parameter -->
          <template v-if="item.section === 'parameter' || item.section === 'add-parameter'">
            <rr-list variant="box" class="settings-list">
              <rr-list-item size="md">
                <rr-text-cell>Naam</rr-text-cell>
                <rr-cell>
                  <rr-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></rr-text-field>
                </rr-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-text-cell>Type</rr-text-cell>
                <rr-cell>
                  <rr-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </rr-dropdown>
                </rr-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-text-cell>Verplicht</rr-text-cell>
                <rr-cell>
                  <rr-switch :checked="values.required ? true : undefined" @change="values.required = Boolean($event.detail?.checked)"></rr-switch>
                </rr-cell>
              </rr-list-item>
            </rr-list>
          </template>

          <!-- Input -->
          <template v-if="item.section === 'input' || item.section === 'add-input'">
            <rr-list variant="box" class="settings-list">
              <rr-list-item size="md">
                <rr-text-cell>Naam</rr-text-cell>
                <rr-cell>
                  <rr-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></rr-text-field>
                </rr-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-text-cell>Type</rr-text-cell>
                <rr-cell>
                  <rr-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </rr-dropdown>
                </rr-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-text-cell>Bron regelgeving</rr-text-cell>
                <rr-cell>
                  <rr-text-field size="md" :value="values.sourceRegulation" @input="values.sourceRegulation = $event.target?.value ?? $event.detail?.value ?? values.sourceRegulation"></rr-text-field>
                </rr-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-text-cell>Bron output</rr-text-cell>
                <rr-cell>
                  <rr-text-field size="md" :value="values.sourceOutput" @input="values.sourceOutput = $event.target?.value ?? $event.detail?.value ?? values.sourceOutput"></rr-text-field>
                </rr-cell>
              </rr-list-item>
            </rr-list>
          </template>

          <!-- Output -->
          <template v-if="item.section === 'output' || item.section === 'add-output'">
            <rr-list variant="box" class="settings-list">
              <rr-list-item size="md">
                <rr-text-cell>Naam</rr-text-cell>
                <rr-cell>
                  <rr-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></rr-text-field>
                </rr-cell>
              </rr-list-item>
              <rr-list-item size="md">
                <rr-text-cell>Type</rr-text-cell>
                <rr-cell>
                  <rr-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </rr-dropdown>
                </rr-cell>
              </rr-list-item>
            </rr-list>
          </template>
        </rr-simple-section>
      </div>

      <!-- Footer -->
      <div class="edit-sheet-footer">
        <rr-button variant="accent-filled" size="md" full-width @click="save">
          Opslaan
        </rr-button>
      </div>
    </div>
  </rr-sheet>
</template>

<style>
.edit-sheet-content {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 480px;
  max-width: 100vw;
}
.edit-sheet-body {
  flex: 1;
  overflow-y: auto;
}
.edit-sheet-footer {
  padding: 0 16px 16px;
}

/* Form field layout in settings list */
.edit-sheet-content .settings-list rr-list-item {
  display: grid;
  grid-template-columns: 120px 1fr;
  gap: 0 12px;
  align-items: center;
}
.edit-sheet-content .settings-list rr-cell {
  width: 100%;
}
.edit-sheet-content .settings-list rr-text-field {
  width: 100%;
}
.edit-sheet-value-group {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}
.edit-sheet-value-group rr-number-field {
  flex: 1;
  min-width: 0;
}
.edit-sheet-unit {
  font-size: 14px;
  font-weight: 500;
  color: var(--semantics-text-secondary-color, #6B7280);
  flex-shrink: 0;
}
</style>

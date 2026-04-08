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
  <ndd-sheet ref="sheetEl" placement="right" @close="emit('close')">
    <ndd-page sticky-header>
      <ndd-top-title-bar slot="header" :text="item ? (sectionLabels[item.section] || 'Bewerk') : ''" dismiss-text="Annuleer" @dismiss="emit('close')"></ndd-top-title-bar>

      <ndd-simple-section v-if="item">
          <!-- Definition -->
          <template v-if="item.section === 'definition' || item.section === 'add-definition'">
            <ndd-list variant="box" class="edit-settings-list">
              <ndd-list-item size="md">
                <ndd-text-cell text="Naam"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Waarde"></ndd-text-cell>
                <ndd-cell>
                  <div v-if="values.controlType === 'currency'" class="edit-sheet-value-group">
                    <span class="edit-sheet-unit">&euro;</span>
                    <ndd-number-field :value="values.displayValue" step="0.01" full-width @change="values.displayValue = $event.detail?.value ?? values.displayValue"></ndd-number-field>
                  </div>
                  <div v-else-if="values.controlType === 'percentage'" class="edit-sheet-value-group">
                    <ndd-number-field :value="values.displayValue" step="0.001" full-width @change="values.displayValue = $event.detail?.value ?? values.displayValue"></ndd-number-field>
                    <span class="edit-sheet-unit">%</span>
                  </div>
                  <ndd-switch-field v-else-if="values.controlType === 'boolean'" :checked="values.displayValue ? true : undefined" @change="values.displayValue = Boolean($event.detail?.checked)">Waarde</ndd-switch-field>
                  <ndd-number-field v-else-if="values.controlType === 'number'" :value="values.displayValue" full-width hide-spin-buttons @change="values.displayValue = $event.detail?.value ?? values.displayValue"></ndd-number-field>
                  <ndd-text-field v-else size="md" :value="String(values.displayValue)" @input="values.displayValue = $event.target?.value ?? $event.detail?.value ?? values.displayValue"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
            </ndd-list>
          </template>

          <!-- Parameter -->
          <template v-if="item.section === 'parameter' || item.section === 'add-parameter'">
            <ndd-list variant="box" class="edit-settings-list">
              <ndd-list-item size="md">
                <ndd-text-cell text="Naam"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Type"></ndd-text-cell>
                <ndd-cell>
                  <ndd-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </ndd-dropdown>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Verplicht"></ndd-text-cell>
                <ndd-cell>
                  <ndd-switch :checked="values.required ? true : undefined" @change="values.required = Boolean($event.detail?.checked)"></ndd-switch>
                </ndd-cell>
              </ndd-list-item>
            </ndd-list>
          </template>

          <!-- Input -->
          <template v-if="item.section === 'input' || item.section === 'add-input'">
            <ndd-list variant="box" class="edit-settings-list">
              <ndd-list-item size="md">
                <ndd-text-cell text="Naam"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Type"></ndd-text-cell>
                <ndd-cell>
                  <ndd-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </ndd-dropdown>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Bron regelgeving"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.sourceRegulation" @input="values.sourceRegulation = $event.target?.value ?? $event.detail?.value ?? values.sourceRegulation"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Bron output"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.sourceOutput" @input="values.sourceOutput = $event.target?.value ?? $event.detail?.value ?? values.sourceOutput"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
            </ndd-list>
          </template>

          <!-- Output -->
          <template v-if="item.section === 'output' || item.section === 'add-output'">
            <ndd-list variant="box" class="edit-settings-list">
              <ndd-list-item size="md">
                <ndd-text-cell text="Naam"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Type"></ndd-text-cell>
                <ndd-cell>
                  <ndd-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </ndd-dropdown>
                </ndd-cell>
              </ndd-list-item>
            </ndd-list>
          </template>
      </ndd-simple-section>

      <ndd-container slot="footer" padding="16">
        <ndd-button variant="primary" size="md" full-width @click="save" text="Opslaan"></ndd-button>
      </ndd-container>
    </ndd-page>
  </ndd-sheet>
</template>

<style>
/* Form field layout in settings list */
.edit-settings-list ndd-list-item {
  display: grid;
  grid-template-columns: 120px 1fr;
  gap: 0 12px;
  align-items: center;
}
.edit-settings-list ndd-cell {
  width: 100%;
}
.edit-settings-list ndd-text-field {
  width: 100%;
}
.edit-sheet-value-group {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}
.edit-sheet-value-group ndd-number-field {
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

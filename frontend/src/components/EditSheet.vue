<script setup>
import { ref, watch, nextTick } from 'vue';

const props = defineProps({
  item: { type: Object, default: null },
});

const emit = defineEmits(['save', 'close']);

const sheetEl = ref(null);
const values = ref({});

const typeOptions = ['string', 'number', 'boolean', 'amount'];

// Monotonic counter for stable v-for keys on the source.parameters rows.
// We can't use the row index as a key because that would let Vue reuse the
// DOM (and the per-row data-testid attributes) across deletions, which
// confuses focus, the testid contract, and the data-testid-bound e2e
// helpers. Each row gets a fresh _rowId when pushed onto the array.
let nextRowId = 1;
function makeParamRow(key = '', value = '') {
  return { _rowId: nextRowId++, key, value };
}

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
    // Flatten source.parameters into an ordered key/value pair list so the
    // user can edit existing entries and add new ones via the form. We use
    // an array (not an object) because two rows can briefly share an empty
    // key while typing, and Object.entries would silently collapse them.
    //
    // Each row carries a stable `_rowId` so the v-for key survives row
    // deletions (otherwise Vue reuses DOM by index and the data-testids
    // bound to row positions point at stale rows).
    //
    // Hydration is value-type aware:
    //   - string / number / boolean → represent as a string the user can
    //     edit. Numbers and booleans round-trip via JSON-ish parsing on
    //     save (see `save()` below).
    //   - object / array → unsupported in the form editor today (no UI
    //     for nested literal values). Skip with a warning rather than
    //     stringify them to "[object Object]" and silently corrupt the
    //     law on the next save. The user can still edit such inputs via
    //     the YAML pane.
    const params = item.data?.source?.parameters;
    const paramList = [];
    if (params && typeof params === 'object') {
      for (const [k, v] of Object.entries(params)) {
        if (v == null || typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') {
          paramList.push(makeParamRow(k, v == null ? '' : String(v)));
        } else {
          // eslint-disable-next-line no-console
          console.warn(
            `EditSheet: skipping non-scalar source.parameter '${k}' for input '${item.data?.name}'. Edit via the YAML pane.`,
          );
        }
      }
    }
    values.value = {
      name: item.data?.name ?? '',
      type: item.data?.type ?? 'string',
      sourceRegulation: item.data?.source?.regulation ?? '',
      sourceOutput: item.data?.source?.output ?? '',
      sourceParameters: paramList,
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
    const { name, type, sourceRegulation, sourceOutput, sourceParameters } = values.value;
    if (!name.trim()) return;
    const data = { name: name.trim(), type };
    // Reduce the form's parameter rows back into a plain object. Skip rows
    // with an empty key — they're either still being typed or were added
    // and abandoned. Duplicate keys: last write wins (matches what
    // serializing an object would do anyway).
    const paramObj = {};
    for (const row of sourceParameters || []) {
      const k = (row.key || '').trim();
      if (!k) continue;
      paramObj[k] = row.value ?? '';
    }
    const hasParams = Object.keys(paramObj).length > 0;
    if (sourceRegulation || sourceOutput || hasParams) {
      data.source = {};
      if (sourceRegulation) data.source.regulation = sourceRegulation;
      if (sourceOutput) data.source.output = sourceOutput;
      if (hasParams) data.source.parameters = paramObj;
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
  <ndd-sheet ref="sheetEl" placement="right" class="edit-sheet" @close="emit('close')">
    <ndd-page sticky-header>
      <ndd-top-title-bar slot="header" :text="item ? (sectionLabels[item.section] || 'Bewerk') : ''" dismiss-text="Annuleer" @dismiss="emit('close')"></ndd-top-title-bar>

      <ndd-simple-section v-if="item">
          <!-- Definition -->
          <template v-if="item.section === 'definition' || item.section === 'add-definition'">
            <ndd-list variant="box" class="edit-settings-list">
              <ndd-list-item size="md">
                <ndd-text-cell text="Naam" max-width="140"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Waarde" max-width="140"></ndd-text-cell>
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
                <ndd-text-cell text="Naam" max-width="140"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Type" max-width="140"></ndd-text-cell>
                <ndd-cell>
                  <ndd-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </ndd-dropdown>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Verplicht" max-width="140"></ndd-text-cell>
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
                <ndd-text-cell text="Naam" max-width="140"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Type" max-width="140"></ndd-text-cell>
                <ndd-cell>
                  <ndd-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </ndd-dropdown>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Bron regelgeving" max-width="140"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.sourceRegulation" @input="values.sourceRegulation = $event.target?.value ?? $event.detail?.value ?? values.sourceRegulation"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Bron output" max-width="140"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.sourceOutput" @input="values.sourceOutput = $event.target?.value ?? $event.detail?.value ?? values.sourceOutput"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
            </ndd-list>

            <ndd-spacer size="12"></ndd-spacer>
            <ndd-title size="6"><h6>Bron parameters</h6></ndd-title>
            <ndd-spacer size="8"></ndd-spacer>
            <ndd-list variant="box" class="edit-settings-list" data-testid="source-parameters-list">
              <ndd-list-item
                v-for="(param, idx) in values.sourceParameters"
                :key="param._rowId"
                size="md"
              >
                <ndd-cell>
                  <ndd-text-field
                    size="md"
                    placeholder="naam"
                    :value="param.key"
                    :data-testid="`source-param-key-${idx}`"
                    @input="param.key = $event.target?.value ?? $event.detail?.value ?? param.key"
                  ></ndd-text-field>
                </ndd-cell>
                <ndd-cell>
                  <ndd-text-field
                    size="md"
                    placeholder="waarde (bijv. $bsn)"
                    :value="param.value"
                    :data-testid="`source-param-value-${idx}`"
                    @input="param.value = $event.target?.value ?? $event.detail?.value ?? param.value"
                  ></ndd-text-field>
                </ndd-cell>
                <ndd-cell>
                  <ndd-icon-button
                    icon="minus"
                    title="Verwijder parameter"
                    :data-testid="`source-param-remove-${idx}`"
                    @click="values.sourceParameters.splice(idx, 1)"
                  ></ndd-icon-button>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-button
                  start-icon="plus-small"
                  data-testid="source-param-add-btn"
                  text="Voeg parameter toe"
                  @click="values.sourceParameters.push(makeParamRow())"
                ></ndd-button>
              </ndd-list-item>
            </ndd-list>
          </template>

          <!-- Output -->
          <template v-if="item.section === 'output' || item.section === 'add-output'">
            <ndd-list variant="box" class="edit-settings-list">
              <ndd-list-item size="md">
                <ndd-text-cell text="Naam" max-width="140"></ndd-text-cell>
                <ndd-cell>
                  <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                </ndd-cell>
              </ndd-list-item>
              <ndd-list-item size="md">
                <ndd-text-cell text="Type" max-width="140"></ndd-text-cell>
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
        <ndd-button variant="primary" size="md" full-width data-testid="edit-sheet-save-btn" @click="save" text="Opslaan"></ndd-button>
      </ndd-container>
    </ndd-page>
  </ndd-sheet>
</template>

<style>
/* Make our EditSheet wider than the NDD default. ndd-sheet hard-codes its
 * width via these tokens (360px md / 480px lg, see node_modules/@minbzk/
 * storybook). Overriding the custom properties on the host scopes the
 * change to this sheet only — other ndd-sheets in the app keep the design
 * system defaults. */
ndd-sheet.edit-sheet {
  --components-sheet-side-md-width: 480px;
  --components-sheet-side-lg-width: 640px;
}

/* Form field layout in the edit sheet's settings list.
 *
 * The previous attempt used `display: grid` on the host element, but
 * `ndd-list-item` is a Lit web component whose internal shadow DOM lays
 * out slotted children with its own flexbox — the user-side grid rule
 * is silently ignored, so the labels collapsed and the value fields
 * shrank to ~80px wide.
 *
 * The pattern that DOES work is the one OperationSettings.vue uses:
 * pin the label cell width via the ndd-text-cell `max-width` attribute
 * (handled inside the component's shadow DOM), and let the value cell
 * grow with `flex: 1; min-width: 0`. The slotted children participate
 * as flex items in ndd-list-item's shadow DOM flex container. */
.edit-settings-list ndd-cell {
  flex: 1;
  min-width: 0;
}
.edit-settings-list ndd-text-field,
.edit-settings-list ndd-dropdown,
.edit-settings-list ndd-number-field {
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

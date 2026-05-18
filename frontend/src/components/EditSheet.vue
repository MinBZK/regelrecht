<script setup>
import { ref, computed, watch, watchEffect, nextTick } from 'vue';
import { collectAvailableVariables } from '../utils/operationTree.js';

const props = defineProps({
  item: { type: Object, default: null },
  article: { type: Object, default: null },
});

const emit = defineEmits(['save', 'close']);

const sheetEl = ref(null);
const lawComboBoxEl = ref(null);
const outputComboBoxEl = ref(null);
const values = ref({});

// Snapshot of the form taken when an item opens. For a NEW item the Save
// button is always shown (you opened the sheet to add it); for an existing
// item it only appears once the user actually changes something.
const baseline = ref('');
const isDirty = computed(() => {
  if (props.item?.isNew) return true;
  try {
    return JSON.stringify(values.value) !== baseline.value;
  } catch {
    return true;
  }
});

const typeOptions = ['string', 'number', 'boolean', 'amount'];

// Available variables from the article's machine_readable for parameter
// value dropdowns, grouped by category with alphabetical sorting within
// each group. Reuses the same collectAvailableVariables utility that
// OperationSettings uses for subject dropdowns.
const paramValueGroups = computed(() => {
  const vars = collectAvailableVariables(props.article);
  const groups = new Map();
  for (const v of vars) {
    if (!groups.has(v.category)) groups.set(v.category, []);
    groups.get(v.category).push({
      value: v.ref,
      label: v.name.replace(/_/g, ' '),
    });
  }
  for (const opts of groups.values()) {
    opts.sort((a, b) => a.label.localeCompare(b.label, 'nl'));
  }
  return groups;
});

// --- Law search / output selection ---
let lawsCache = null;
async function fetchLawsList() {
  if (lawsCache) return lawsCache;
  try {
    const res = await fetch('/api/corpus/laws?limit=1000');
    if (!res.ok) return [];
    lawsCache = await res.json();
  } catch {
    return [];
  }
  return lawsCache;
}

const allLaws = ref([]);
const availableOutputs = ref([]);
const outputsLoading = ref(false);

function displayName(law) {
  if (law.display_name) return law.display_name;
  if (law.name) return law.name;
  return law.law_id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

function outputDisplayName(out) {
  return `${out.name} (${out.output_type})`;
}

async function fetchOutputsForLaw(lawId) {
  if (!lawId) {
    availableOutputs.value = [];
    return;
  }
  outputsLoading.value = true;
  try {
    const res = await fetch(`/api/corpus/laws/${encodeURIComponent(lawId)}/outputs`);
    if (res.ok) {
      availableOutputs.value = await res.json();
    } else {
      availableOutputs.value = [];
    }
  } catch {
    availableOutputs.value = [];
  } finally {
    outputsLoading.value = false;
  }
}

// Combo-box has no value→displayValue sync (display is internal @state, only
// updated by user actions). Keep them aligned manually whenever the form's
// sourceRegulation changes OR allLaws finishes loading. Without this, an
// existing input opened in the sheet would show an empty input until the
// user typed.
watchEffect(() => {
  const lawId = values.value?.sourceRegulation;
  if (!lawComboBoxEl.value) return;
  const match = allLaws.value.find(l => l.law_id === lawId);
  lawComboBoxEl.value._displayValue = match ? displayName(match) : (lawId || '');
});

watchEffect(() => {
  const outName = values.value?.sourceOutput;
  if (!outputComboBoxEl.value) return;
  const match = availableOutputs.value.find(o => o.name === outName);
  outputComboBoxEl.value._displayValue = match ? outputDisplayName(match) : (outName || '');
});

async function onLawComboChange(event) {
  const next = event.detail?.value ?? '';
  if (next === values.value.sourceRegulation) return;
  values.value.sourceRegulation = next;
  values.value.sourceOutput = '';
  await fetchOutputsForLaw(next);
}

function onOutputComboChange(event) {
  const outputName = event.detail?.value ?? '';
  values.value.sourceOutput = outputName;
  if (!outputName) return;
  const match = availableOutputs.value.find(o => o.name === outputName);
  if (match) {
    if (!values.value.name) {
      values.value.name = outputName;
    }
    // Always set type from the source output — it's determined by the external law
    values.value.type = match.output_type || 'string';
    // Always reset and pre-populate source parameters from the selected
    // output's article. Without the unconditional reset, switching to an
    // output with fewer (or no) parameters would leave stale rows that
    // produce invalid source.parameters on save.
    values.value.sourceParameters = (match.parameters ?? []).map(p =>
      makeParamRow(p.name, ''),
    );
  }
}

// Monotonic counter for stable v-for keys on the source.parameters rows.
// We can't use the row index as a key because that would let Vue reuse the
// DOM (and the per-row data-testid attributes) across deletions, which
// confuses focus, the testid contract, and the data-testid-bound e2e
// helpers. Each row gets a fresh _rowId when pushed onto the array.
//
// `_origValue` carries the row's original (un-stringified) value so that
// numeric / boolean parameters round-trip back to their original type on
// save when the user didn't touch the value field. Without it the form
// would silently rewrite `peildatum: 2024` to `peildatum: '2024'`.
let nextRowId = 1;
function makeParamRow(key = '', value = '', origValue = undefined) {
  return { _rowId: nextRowId++, key, value, _origValue: origValue };
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
      // structuredClone throws DataCloneError on Vue reactive proxies; defs are JSON-safe.
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
    // user can edit existing entries and add new ones via the form.
    //
    // Design decisions:
    //   - Each row gets a stable `_rowId` (monotonic counter) so Vue's
    //     v-for keying survives deletions without confusing focus or
    //     data-testid attributes.
    //   - `_origValue` preserves the original (un-stringified) value so
    //     numeric/boolean parameters round-trip correctly when the user
    //     doesn't touch the value field (avoids `peildatum: 2024` becoming
    //     `peildatum: '2024'`). On save, if the stringified form hasn't
    //     changed we emit the original typed value.
    //   - Non-scalar parameter values (objects/arrays) are stashed in
    //     `sourceParametersOverflow` and merged back on save so they
    //     survive untouched — the user can only edit those via the YAML
    //     pane.
    const params = item.data?.source?.parameters;
    const paramList = [];
    const overflowParams = {};
    if (params && typeof params === 'object') {
      for (const [k, v] of Object.entries(params)) {
        if (v == null || typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') {
          paramList.push(makeParamRow(k, v == null ? '' : String(v), v));
        } else {
          overflowParams[k] = v;
          // eslint-disable-next-line no-console
          console.warn(
            `EditSheet: source.parameter '${k}' on input '${item.data?.name}' has a non-scalar value; preserved untouched (edit via the YAML pane).`,
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
      sourceParametersOverflow: overflowParams,
    };

    // Load law list for the combo-box menu and pre-populate outputs if
    // editing an existing input. The watchEffect above handles syncing the
    // combo-box's display once allLaws + sourceRegulation are both set.
    availableOutputs.value = [];
    fetchLawsList().then(laws => {
      allLaws.value = laws;
      const reg = item.data?.source?.regulation;
      if (reg) fetchOutputsForLaw(reg);
    });
  } else if (s === 'output' || s === 'add-output') {
    values.value = {
      name: item.data?.name ?? '',
      type: item.data?.type ?? 'string',
    };
  }

  baseline.value = JSON.stringify(values.value);
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
    const { name, type, sourceRegulation, sourceOutput, sourceParameters, sourceParametersOverflow } = values.value;
    if (!name.trim()) return;
    const data = { name: name.trim(), type };
    // Reduce the form's parameter rows back into a plain object. Skip
    // rows with an empty key — they're either still being typed or were
    // added and abandoned. Duplicate keys: last write wins (matches what
    // serializing an object would do anyway).
    //
    // Two correctness rules from iteration 2 review:
    //   1. If the row's value string still equals String(_origValue), the
    //      user didn't touch the value field; emit the original (which
    //      preserves number/boolean types). Otherwise emit the typed
    //      string verbatim.
    //   2. Overflow parameters (object/array values that the form
    //      doesn't render) get merged back BEFORE the user-edited rows
    //      so the user can override them by adding a row with the same
    //      key, but un-overridden overflow keys survive.
    const paramObj = { ...(sourceParametersOverflow || {}) };
    for (const row of sourceParameters || []) {
      const k = (row.key || '').trim();
      if (!k) continue;
      const origStringForm = row._origValue == null ? '' : String(row._origValue);
      if (row._origValue !== undefined && row.value === origStringForm) {
        paramObj[k] = row._origValue;
      } else {
        paramObj[k] = row.value ?? '';
      }
    }
    // Emit `source` only when at least regulation OR output is set.
    // Parameters alone don't make a valid source block (the schema
    // requires regulation when source is present), so if the user has
    // cleared both regulation and output we drop the entire source —
    // including any overflow params. This matches the user's intent
    // ("disable the source binding") and avoids producing a malformed
    // source: { parameters: {...} } that would fail schema validation.
    if (sourceRegulation || sourceOutput) {
      data.source = {};
      if (sourceRegulation) data.source.regulation = sourceRegulation;
      if (sourceOutput) data.source.output = sourceOutput;
      if (Object.keys(paramObj).length > 0) data.source.parameters = paramObj;
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
  'add-definition': 'Definitie toevoegen',
  'parameter': 'Parameter',
  'add-parameter': 'Parameter toevoegen',
  'input': 'Input',
  'add-input': 'Input toevoegen',
  'output': 'Output',
  'add-output': 'Output toevoegen',
};
</script>

<template>
  <nldd-sheet ref="sheetEl" placement="right" width="640px" @close="emit('close')">
    <!-- `:key` forces nldd-page to remount whenever the section changes.
         nldd-page captures the sticky-header height ONCE per mount via
         requestAnimationFrame; if the header text changes after that
         (which happens here because :text is reactive on item.section),
         the measurement stays at the empty/initial value and the body
         content slides up under the title bar. Remounting on section
         change re-runs the measurement with the now-set title text,
         which is the storybook-conventional way to handle a header
         whose content swaps in. -->
    <nldd-page :key="item?.section ?? 'none'" sticky-header>
      <nldd-top-title-bar slot="header" :text="item ? (sectionLabels[item.section] || 'Bewerk') : ''" dismiss-text="Annuleer" @dismiss="emit('close')"></nldd-top-title-bar>

      <nldd-simple-section v-if="item">
          <!-- Definition -->
          <template v-if="item.section === 'definition' || item.section === 'add-definition'">
            <nldd-list variant="box" class="edit-settings-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Waarde" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <template v-if="values.controlType === 'currency'">
                  <nldd-text-cell text="€" width="fit-content"></nldd-text-cell>
                  <nldd-spacer-cell size="6"></nldd-spacer-cell>
                </template>
                <nldd-cell v-if="values.controlType === 'boolean'">
                  <nldd-switch-field :checked="values.displayValue ? true : undefined" @change="values.displayValue = Boolean($event.detail?.checked)">Waarde</nldd-switch-field>
                </nldd-cell>
                <nldd-cell v-else-if="values.controlType === 'currency' || values.controlType === 'percentage' || values.controlType === 'number'">
                  <!-- number-field with hide-spin-buttons: keeps numeric input
                       validation (rejects non-numeric, handles locale) without
                       the visual clutter that felt out of place for "fixed
                       value from law" semantics. The plain text-field used
                       previously silently produced NaN on Dutch comma input
                       and 0 on cleared field.
                       Both @input and @change are intentional, not a
                       copy-paste: @input gives live-as-you-type updates
                       (dirty marking); @change delivers the value the
                       field normalises on commit/blur (locale/step). The
                       assignment is idempotent so a same-tick double-fire
                       is harmless — do not drop @change. -->
                  <nldd-number-field
                    :value="values.displayValue"
                    :step="values.controlType === 'currency' ? '0.01' : (values.controlType === 'percentage' ? '0.001' : undefined)"
                    width="full"
                    hide-spin-buttons
                    @input="values.displayValue = $event.detail?.value ?? values.displayValue"
                    @change="values.displayValue = $event.detail?.value ?? values.displayValue"
                  ></nldd-number-field>
                </nldd-cell>
                <nldd-cell v-else>
                  <nldd-text-field size="md" :value="String(values.displayValue)" @input="values.displayValue = $event.target?.value ?? $event.detail?.value ?? values.displayValue"></nldd-text-field>
                </nldd-cell>
                <template v-if="values.controlType === 'percentage'">
                  <nldd-spacer-cell size="6"></nldd-spacer-cell>
                  <nldd-text-cell text="%" width="fit-content"></nldd-text-cell>
                </template>
              </nldd-list-item>
            </nldd-list>
          </template>

          <!-- Parameter -->
          <template v-if="item.section === 'parameter' || item.section === 'add-parameter'">
            <nldd-list variant="box" class="edit-settings-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Type" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </nldd-dropdown>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Verplicht" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-switch :checked="values.required ? true : undefined" @change="values.required = Boolean($event.detail?.checked)"></nldd-switch>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <!-- Input -->
          <template v-if="item.section === 'input' || item.section === 'add-input'">
            <nldd-list variant="box" class="edit-settings-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item v-if="!values.sourceOutput" size="md">
                <nldd-text-cell text="Type" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </nldd-dropdown>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Bron regelgeving" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-combo-box
                    ref="lawComboBoxEl"
                    size="md"
                    width="100%"
                    placeholder="Zoek regelgeving..."
                    accessible-label="Bron regelgeving"
                    :value="values.sourceRegulation"
                    data-testid="law-combo-box"
                    @change="onLawComboChange"
                  >
                    <nldd-menu>
                      <nldd-menu-item
                        v-for="law in allLaws"
                        :key="law.law_id"
                        :text="displayName(law)"
                        :aliases="law.law_id"
                        :value="law.law_id"
                        :data-testid="`law-result-${law.law_id}`"
                      ></nldd-menu-item>
                    </nldd-menu>
                  </nldd-combo-box>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Bron output" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-combo-box
                    v-if="availableOutputs.length > 0"
                    ref="outputComboBoxEl"
                    size="md"
                    width="100%"
                    placeholder="Selecteer output..."
                    accessible-label="Bron output"
                    :value="values.sourceOutput"
                    data-testid="output-combo-box"
                    @change="onOutputComboChange"
                  >
                    <nldd-menu>
                      <nldd-menu-item
                        v-for="out in availableOutputs"
                        :key="out.name"
                        :text="outputDisplayName(out)"
                        :value="out.name"
                      ></nldd-menu-item>
                    </nldd-menu>
                  </nldd-combo-box>
                  <nldd-text-field v-else size="md" :value="values.sourceOutput" data-testid="output-text-field" @input="values.sourceOutput = $event.target?.value ?? $event.detail?.value ?? values.sourceOutput"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>

            <nldd-spacer size="12"></nldd-spacer>
            <nldd-title size="6"><h6>Bron parameters</h6></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list variant="box" class="edit-settings-list" data-testid="source-parameters-list">
              <nldd-list-item
                v-for="param in values.sourceParameters"
                :key="param._rowId"
                size="md"
              >
                <nldd-text-cell
                  :text="param.key"
                  max-width="140px"
                  :data-testid="`source-param-key-${param._rowId}`"
                ></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-dropdown size="md" :data-testid="`source-param-value-${param._rowId}`">
                    <select :value="param.value" :aria-label="`Waarde voor ${param.key}`" @change="param.value = $event.target.value">
                      <option value="">Selecteer...</option>
                      <option v-if="param.value && !param.value.startsWith('$')" :value="param.value" :selected="true">{{ param.value }}</option>
                      <optgroup v-for="[category, opts] in paramValueGroups" :key="category" :label="category">
                        <option v-for="opt in opts" :key="opt.value" :value="opt.value" :selected="opt.value === param.value">{{ opt.label }}</option>
                      </optgroup>
                    </select>
                  </nldd-dropdown>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <!-- Output -->
          <template v-if="item.section === 'output' || item.section === 'add-output'">
            <nldd-list variant="box" class="edit-settings-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Type" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-dropdown size="md">
                    <select :value="values.type" @change="values.type = $event.target.value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </nldd-dropdown>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
      </nldd-simple-section>

      <nldd-container v-if="isDirty" slot="footer" padding="16">
        <nldd-button variant="primary" size="md" width="full" data-testid="edit-sheet-save-btn" @click="save" text="Opslaan"></nldd-button>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>

<style>
/* Form field layout in the edit sheet's settings list.
 *
 * The previous attempt used `display: grid` on the host element, but
 * `nldd-list-item` is a Lit web component whose internal shadow DOM lays
 * out slotted children with its own flexbox — the user-side grid rule
 * is silently ignored, so the labels collapsed and the value fields
 * shrank to ~80px wide.
 *
 * The pattern that DOES work is the one OperationSettings.vue uses:
 * pin the label cell width via the nldd-text-cell `max-width` attribute
 * (handled inside the component's shadow DOM), and let the value cell
 * grow with `flex: 1; min-width: 0`. The slotted children participate
 * as flex items in nldd-list-item's shadow DOM flex container. */
.edit-settings-list nldd-cell {
  flex: 1;
  min-width: 0;
}
.edit-settings-list nldd-text-field,
.edit-settings-list nldd-dropdown,
.edit-settings-list nldd-number-field {
  width: 100%;
}
</style>

<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { collectAvailableVariables } from '../utils/operationTree.js';

const props = defineProps({
  item: { type: Object, default: null },
  article: { type: Object, default: null },
});

const emit = defineEmits(['save', 'close']);

const sheetEl = ref(null);
const searchFieldEl = ref(null);
const inputWrapperEl = ref(null);
const values = ref({});

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
const lawSearchQuery = ref('');
const showLawResults = ref(false);
const availableOutputs = ref([]);
const outputsLoading = ref(false);

function displayName(law) {
  if (law.display_name) return law.display_name;
  if (law.name) return law.name;
  return law.law_id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

const filteredLaws = computed(() => {
  const q = lawSearchQuery.value.toLowerCase().trim();
  if (!q) return allLaws.value.slice(0, 20);
  return allLaws.value.filter(law =>
    displayName(law).toLowerCase().includes(q) ||
    law.law_id.toLowerCase().includes(q),
  ).slice(0, 20);
});

function onLawSearchInput(event) {
  lawSearchQuery.value = event.target?.value ?? event.detail?.value ?? '';
  showLawResults.value = true;
  nextTick(() => updateResultsPosition());
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

async function selectLaw(law) {
  values.value.sourceRegulation = law.law_id;
  lawSearchQuery.value = displayName(law);
  showLawResults.value = false;
  values.value.sourceOutput = '';
  await fetchOutputsForLaw(law.law_id);
}

function onOutputSelected(outputName) {
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

function closeLawResults() {
  // Delay to allow click on results to register before closing
  setTimeout(() => { showLawResults.value = false; }, 200);
}

const resultsTopPx = ref(0);

function updateResultsPosition() {
  if (!searchFieldEl.value || !inputWrapperEl.value) return;
  const fieldRect = searchFieldEl.value.getBoundingClientRect();
  const wrapperRect = inputWrapperEl.value.getBoundingClientRect();
  resultsTopPx.value = fieldRect.bottom - wrapperRect.top;
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

    // Load law list for search and pre-populate outputs if editing existing input
    showLawResults.value = false;
    availableOutputs.value = [];
    fetchLawsList().then(laws => {
      allLaws.value = laws;
      const reg = item.data?.source?.regulation;
      if (reg) {
        const match = laws.find(l => l.law_id === reg);
        lawSearchQuery.value = match ? displayName(match) : reg;
        fetchOutputsForLaw(reg);
      } else {
        lawSearchQuery.value = '';
      }
    });
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
    <!-- `:key` forces ndd-page to remount whenever the section changes.
         ndd-page captures the sticky-header height ONCE per mount via
         requestAnimationFrame; if the header text changes after that
         (which happens here because :text is reactive on item.section),
         the measurement stays at the empty/initial value and the body
         content slides up under the title bar. Remounting on section
         change re-runs the measurement with the now-set title text,
         which is the storybook-conventional way to handle a header
         whose content swaps in. -->
    <ndd-page :key="item?.section ?? 'none'" sticky-header>
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
            <div class="input-fields-wrapper" ref="inputWrapperEl">
              <ndd-list variant="box" class="edit-settings-list">
                <ndd-list-item size="md">
                  <ndd-text-cell text="Naam" max-width="140"></ndd-text-cell>
                  <ndd-cell>
                    <ndd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></ndd-text-field>
                  </ndd-cell>
                </ndd-list-item>
                <ndd-list-item v-if="!values.sourceOutput" size="md">
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
                    <ndd-search-field
                      ref="searchFieldEl"
                      size="md"
                      placeholder="Zoek regelgeving..."
                      :value="lawSearchQuery"
                      data-testid="law-search-field"
                      @input="onLawSearchInput($event)"
                      @focus="showLawResults = true; nextTick(() => updateResultsPosition())"
                      @focusout="closeLawResults"
                    ></ndd-search-field>
                  </ndd-cell>
                </ndd-list-item>
                <ndd-list-item size="md">
                  <ndd-text-cell text="Bron output" max-width="140"></ndd-text-cell>
                  <ndd-cell>
                    <ndd-dropdown v-if="availableOutputs.length > 0" size="md" data-testid="output-dropdown">
                      <select :value="values.sourceOutput" @change="onOutputSelected($event.target.value)" aria-label="Bron output">
                        <option value="">Selecteer output...</option>
                        <option v-for="out in availableOutputs" :key="out.name" :value="out.name">{{ out.name }} ({{ out.output_type }})</option>
                      </select>
                    </ndd-dropdown>
                    <ndd-text-field v-else size="md" :value="values.sourceOutput" data-testid="output-text-field" @input="values.sourceOutput = $event.target?.value ?? $event.detail?.value ?? values.sourceOutput"></ndd-text-field>
                  </ndd-cell>
                </ndd-list-item>
              </ndd-list>

              <!-- Absolute overlay: rendered outside the ndd-list to escape
                   shadow DOM overflow clipping, but positioned over the
                   controls below (Bron output, parameters) via z-index. -->
              <div v-if="showLawResults && filteredLaws.length > 0" class="law-search-results" :style="{ top: resultsTopPx + 'px' }" data-testid="law-search-results">
                <ndd-list variant="box">
                  <ndd-list-item
                    v-for="law in filteredLaws"
                    :key="law.law_id"
                    size="sm"
                    class="law-search-result-item"
                    :data-testid="`law-result-${law.law_id}`"
                    @mousedown.prevent="selectLaw(law)"
                  >
                    <ndd-text-cell :text="displayName(law)" :supporting-text="law.law_id"></ndd-text-cell>
                  </ndd-list-item>
                </ndd-list>
              </div>
            </div>

            <ndd-spacer size="12"></ndd-spacer>
            <ndd-title size="6"><h6>Bron parameters</h6></ndd-title>
            <ndd-spacer size="8"></ndd-spacer>
            <ndd-list variant="box" class="edit-settings-list" data-testid="source-parameters-list">
              <ndd-list-item
                v-for="param in values.sourceParameters"
                :key="param._rowId"
                size="md"
              >
                <ndd-cell>
                  <ndd-text-field
                    size="md"
                    placeholder="naam"
                    :value="param.key"
                    readonly
                    :data-testid="`source-param-key-${param._rowId}`"
                  ></ndd-text-field>
                </ndd-cell>
                <ndd-cell>
                  <ndd-dropdown size="md" :data-testid="`source-param-value-${param._rowId}`">
                    <select :value="param.value" :aria-label="`Waarde voor ${param.key}`" @change="param.value = $event.target.value">
                      <option value="">Selecteer...</option>
                      <option v-if="param.value && !param.value.startsWith('$')" :value="param.value" :selected="true">{{ param.value }}</option>
                      <optgroup v-for="[category, opts] in paramValueGroups" :key="category" :label="category">
                        <option v-for="opt in opts" :key="opt.value" :value="opt.value" :selected="opt.value === param.value">{{ opt.label }}</option>
                      </optgroup>
                    </select>
                  </ndd-dropdown>
                </ndd-cell>
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
.input-fields-wrapper {
  position: relative;
}
.law-search-results {
  position: absolute;
  left: 0;
  right: 0;
  z-index: 100;
  max-height: 240px;
  overflow-y: auto;
  background: var(--semantics-surface-primary-color, #fff);
  border: 1px solid var(--semantics-border-primary-color, #d1d5db);
  border-radius: 4px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}
.law-search-result-item {
  cursor: pointer;
}
</style>

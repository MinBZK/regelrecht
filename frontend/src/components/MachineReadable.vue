<script setup>
import { computed, ref, watch } from 'vue';

const props = defineProps({
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
});

const emit = defineEmits(['open-action', 'save']);

const mr = computed(() => props.article?.machine_readable ?? null);
const execution = computed(() => mr.value?.execution ?? null);

const definitions = computed(() => {
  const defs = mr.value?.definitions;
  if (!defs) return [];
  return Object.entries(defs).map(([name, def]) => {
    const val = typeof def === 'object' ? def.value : def;
    const unit = typeof def === 'object' ? def.type_spec?.unit : undefined;
    return { name, value: val, unit };
  });
});

const produces = computed(() => execution.value?.produces ?? null);

const parameters = computed(() =>
  (execution.value?.parameters ?? []).map((p) => ({
    name: p.name,
    type: p.type,
    required: p.required ?? false,
  }))
);

const inputs = computed(() =>
  (execution.value?.input ?? []).map((i) => ({
    name: i.name,
    type: i.type,
    source: i.source?.regulation ?? i.source?.output ?? null,
  }))
);

const outputs = computed(() =>
  (execution.value?.output ?? []).map((o) => ({
    name: o.name,
    type: o.type,
  }))
);

const actions = computed(() => execution.value?.actions ?? []);

function formatValue(val, unit) {
  if (typeof val === 'number') {
    if (unit === 'eurocent') {
      return (val / 100).toLocaleString('nl-NL', { style: 'currency', currency: 'EUR' });
    }
    if (val > 0 && val < 1) {
      return (val * 100).toLocaleString('nl-NL', { maximumFractionDigits: 3 }) + '%';
    }
  }
  return String(val);
}

// ── Editing ──

const editingId = ref(null);
const editValues = ref({});

watch(() => props.article, () => {
  editingId.value = null;
});

function isEditing(id) {
  return editingId.value === id;
}

function cancelEdit() {
  editingId.value = null;
  editValues.value = {};
}

// Type inference for controls
function inferControlType(value, unit, declaredType) {
  if (declaredType === 'boolean' || typeof value === 'boolean') return 'boolean';
  if (unit === 'eurocent') return 'currency';
  if (typeof value === 'number' && value > 0 && value < 1 && !unit) return 'percentage';
  if (declaredType === 'number' || declaredType === 'amount' || typeof value === 'number') return 'number';
  return 'text';
}

function toDisplay(value, controlType) {
  if (controlType === 'currency') return +(value / 100).toFixed(2);
  if (controlType === 'percentage') return +(value * 100).toFixed(6);
  return value;
}

function fromDisplay(value, controlType) {
  if (controlType === 'currency') return Math.round(value * 100);
  if (controlType === 'percentage') return value / 100;
  return value;
}

// Definition editing
function startEditDef(name) {
  const rawDef = mr.value?.definitions?.[name];
  const val = typeof rawDef === 'object' ? rawDef.value : rawDef;
  const unit = typeof rawDef === 'object' ? rawDef.type_spec?.unit : undefined;
  const ct = inferControlType(val, unit);
  editingId.value = `def:${name}`;
  editValues.value = {
    name,
    displayValue: toDisplay(val, ct),
    controlType: ct,
    unit,
    rawDef: JSON.parse(JSON.stringify(rawDef)),
  };
}

function saveDef(originalKey) {
  const { name: newName, displayValue, controlType, rawDef } = editValues.value;
  const stored = controlType === 'boolean'
    ? displayValue
    : fromDisplay(Number(displayValue), controlType);
  let data;
  if (typeof rawDef === 'object') {
    data = { ...rawDef, value: stored };
  } else {
    data = stored;
  }
  emit('save', { section: 'definition', key: originalKey, newKey: newName, data });
  cancelEdit();
}

// Parameter editing
function startEditParam(index) {
  const p = execution.value?.parameters?.[index];
  if (!p) return;
  editingId.value = `param:${index}`;
  editValues.value = { name: p.name, type: p.type, required: p.required ?? false };
}

function saveParam(index) {
  const { name, type, required } = editValues.value;
  emit('save', { section: 'parameter', index, data: { name, type, required } });
  cancelEdit();
}

// Input editing
function startEditInput(index) {
  const raw = execution.value?.input?.[index];
  if (!raw) return;
  editingId.value = `input:${index}`;
  editValues.value = {
    name: raw.name,
    type: raw.type,
    sourceRegulation: raw.source?.regulation ?? '',
    sourceOutput: raw.source?.output ?? '',
  };
}

function saveInput(index) {
  const { name, type, sourceRegulation, sourceOutput } = editValues.value;
  const raw = execution.value?.input?.[index];
  const data = { name, type };
  if (sourceRegulation || sourceOutput) {
    data.source = {};
    if (sourceRegulation) data.source.regulation = sourceRegulation;
    if (sourceOutput) data.source.output = sourceOutput;
    if (raw?.source?.parameters) data.source.parameters = raw.source.parameters;
  }
  if (raw?.type_spec) data.type_spec = raw.type_spec;
  emit('save', { section: 'input', index, data });
  cancelEdit();
}

// Output editing
function startEditOutput(index) {
  const o = execution.value?.output?.[index];
  if (!o) return;
  editingId.value = `output:${index}`;
  editValues.value = { name: o.name, type: o.type };
}

function saveOutput(index) {
  const { name, type } = editValues.value;
  const raw = execution.value?.output?.[index];
  const data = { name, type };
  if (raw?.type_spec) data.type_spec = raw.type_spec;
  emit('save', { section: 'output', index, data });
  cancelEdit();
}

// ── Adding new items ──

function addDef() {
  editingId.value = 'new:def';
  editValues.value = { name: '', displayValue: 0, controlType: 'number', unit: undefined, rawDef: { value: 0 } };
}

function saveNewDef() {
  const { name, displayValue, controlType, rawDef } = editValues.value;
  if (!name.trim()) return;
  const stored = controlType === 'boolean'
    ? displayValue
    : fromDisplay(Number(displayValue), controlType);
  const data = typeof rawDef === 'object' ? { ...rawDef, value: stored } : stored;
  emit('save', { section: 'add-definition', key: name.trim(), data });
  cancelEdit();
}

function addParam() {
  editingId.value = 'new:param';
  editValues.value = { name: '', type: 'string', required: false };
}

function saveNewParam() {
  const { name, type, required } = editValues.value;
  if (!name.trim()) return;
  emit('save', { section: 'add-parameter', data: { name: name.trim(), type, required } });
  cancelEdit();
}

function addInput() {
  editingId.value = 'new:input';
  editValues.value = { name: '', type: 'string', sourceRegulation: '', sourceOutput: '' };
}

function saveNewInput() {
  const { name, type, sourceRegulation, sourceOutput } = editValues.value;
  if (!name.trim()) return;
  const data = { name: name.trim(), type };
  if (sourceRegulation || sourceOutput) {
    data.source = {};
    if (sourceRegulation) data.source.regulation = sourceRegulation;
    if (sourceOutput) data.source.output = sourceOutput;
  }
  emit('save', { section: 'add-input', data });
  cancelEdit();
}

function addOutput() {
  editingId.value = 'new:output';
  editValues.value = { name: '', type: 'string' };
}

function saveNewOutput() {
  const { name, type } = editValues.value;
  if (!name.trim()) return;
  emit('save', { section: 'add-output', data: { name: name.trim(), type } });
  cancelEdit();
}

const typeOptions = ['string', 'number', 'boolean', 'amount'];
</script>

<template>
  <div v-if="!mr" style="padding: 32px; color: #999; text-align: center;">
    Geen machine-leesbare gegevens voor dit artikel
  </div>

  <template v-else>
    <!-- Metadata: produces -->
    <rr-list v-if="produces" variant="box">
      <rr-list-item v-if="produces.legal_character" size="md">
        <rr-label-cell>Juridische basis</rr-label-cell>
        <rr-button-cell slot="end">
          <rr-button variant="neutral-tinted" size="md">
            {{ produces.legal_character }}
            <img src="/assets/icons/chevron-down-small.svg" alt="" width="16" height="16">
          </rr-button>
        </rr-button-cell>
      </rr-list-item>
      <rr-list-item v-if="produces.decision_type" size="md">
        <rr-label-cell>Besluit-type</rr-label-cell>
        <rr-button-cell slot="end">
          <rr-button variant="neutral-tinted" size="md">
            {{ produces.decision_type }}
            <img src="/assets/icons/chevron-down-small.svg" alt="" width="16" height="16">
          </rr-button>
        </rr-button-cell>
      </rr-list-item>
    </rr-list>

    <rr-spacer v-if="produces" size="12"></rr-spacer>

    <!-- Definities -->
    <template v-if="definitions.length || editable">
      <h3 class="machine-section-title">Definities</h3>
      <rr-list variant="box">
        <rr-list-item v-for="def in definitions" :key="def.name" size="md">
          <!-- Edit mode -->
          <div v-if="editable && isEditing(`def:${def.name}`)" class="edit-form">
            <label>Naam</label>
            <input type="text" v-model="editValues.name" class="edit-input">
            <label>Waarde</label>
            <input
              v-if="editValues.controlType === 'boolean'"
              type="checkbox"
              v-model="editValues.displayValue"
              class="edit-checkbox"
            >
            <div v-else-if="editValues.controlType === 'currency'" class="edit-input-group">
              <span class="edit-input-prefix">&euro;</span>
              <input type="number" step="0.01" v-model.number="editValues.displayValue" class="edit-input">
            </div>
            <div v-else-if="editValues.controlType === 'percentage'" class="edit-input-group">
              <input type="number" step="0.001" v-model.number="editValues.displayValue" class="edit-input">
              <span class="edit-input-suffix">%</span>
            </div>
            <input v-else-if="editValues.controlType === 'number'" type="number" v-model.number="editValues.displayValue" class="edit-input">
            <input v-else type="text" v-model="editValues.displayValue" class="edit-input">
            <div class="edit-actions">
              <rr-button variant="neutral-tinted" size="sm" @click="saveDef(def.name)">Opslaan</rr-button>
              <rr-button variant="neutral-tinted" size="sm" @click="cancelEdit">Annuleer</rr-button>
            </div>
          </div>
          <!-- Display mode -->
          <template v-else>
            <rr-text-cell>{{ def.name }} = {{ formatValue(def.value, def.unit) }}</rr-text-cell>
            <rr-button-cell slot="end">
              <rr-button variant="neutral-tinted" size="sm" @click="editable && startEditDef(def.name)">Bewerk</rr-button>
            </rr-button-cell>
          </template>
        </rr-list-item>

        <!-- Add new definition -->
        <rr-list-item v-if="editable && isEditing('new:def')" size="md">
          <div class="edit-form">
            <label>Naam</label>
            <input type="text" v-model="editValues.name" class="edit-input" placeholder="naam_definitie">
            <label>Waarde</label>
            <input type="number" v-model.number="editValues.displayValue" class="edit-input">
            <div class="edit-actions">
              <rr-button variant="neutral-tinted" size="sm" @click="saveNewDef">Opslaan</rr-button>
              <rr-button variant="neutral-tinted" size="sm" @click="cancelEdit">Annuleer</rr-button>
            </div>
          </div>
        </rr-list-item>
        <rr-list-item v-if="editable && !isEditing('new:def')" size="md">
          <button class="add-button" @click="addDef">+ Nieuwe definitie</button>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>

    <!-- Parameters -->
    <template v-if="parameters.length || editable">
      <h3 class="machine-section-title">Parameters</h3>
      <rr-list variant="box">
        <rr-list-item v-for="(param, index) in parameters" :key="param.name" size="md">
          <!-- Edit mode -->
          <div v-if="editable && isEditing(`param:${index}`)" class="edit-form">
            <label>Naam</label>
            <input type="text" v-model="editValues.name" class="edit-input">
            <label>Type</label>
            <select v-model="editValues.type" class="edit-input">
              <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
            </select>
            <label>Verplicht</label>
            <input type="checkbox" v-model="editValues.required" class="edit-checkbox">
            <div class="edit-actions">
              <rr-button variant="neutral-tinted" size="sm" @click="saveParam(index)">Opslaan</rr-button>
              <rr-button variant="neutral-tinted" size="sm" @click="cancelEdit">Annuleer</rr-button>
            </div>
          </div>
          <!-- Display mode -->
          <template v-else>
            <rr-text-cell>{{ param.name }} ({{ param.type }})</rr-text-cell>
            <rr-button-cell slot="end">
              <rr-button variant="neutral-tinted" size="sm" @click="editable && startEditParam(index)">Bewerk</rr-button>
            </rr-button-cell>
          </template>
        </rr-list-item>

        <!-- Add new parameter -->
        <rr-list-item v-if="editable && isEditing('new:param')" size="md">
          <div class="edit-form">
            <label>Naam</label>
            <input type="text" v-model="editValues.name" class="edit-input" placeholder="parameter_naam">
            <label>Type</label>
            <select v-model="editValues.type" class="edit-input">
              <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
            </select>
            <label>Verplicht</label>
            <input type="checkbox" v-model="editValues.required" class="edit-checkbox">
            <div class="edit-actions">
              <rr-button variant="neutral-tinted" size="sm" @click="saveNewParam">Opslaan</rr-button>
              <rr-button variant="neutral-tinted" size="sm" @click="cancelEdit">Annuleer</rr-button>
            </div>
          </div>
        </rr-list-item>
        <rr-list-item v-if="editable && !isEditing('new:param')" size="md">
          <button class="add-button" @click="addParam">+ Nieuwe parameter</button>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>

    <!-- Inputs -->
    <template v-if="inputs.length || editable">
      <h3 class="machine-section-title">Inputs</h3>
      <rr-list variant="box">
        <rr-list-item v-for="(input, index) in inputs" :key="input.name" size="md">
          <!-- Edit mode -->
          <div v-if="editable && isEditing(`input:${index}`)" class="edit-form">
            <label>Naam</label>
            <input type="text" v-model="editValues.name" class="edit-input">
            <label>Type</label>
            <select v-model="editValues.type" class="edit-input">
              <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
            </select>
            <label>Bron regelgeving</label>
            <input type="text" v-model="editValues.sourceRegulation" class="edit-input">
            <label>Bron output</label>
            <input type="text" v-model="editValues.sourceOutput" class="edit-input">
            <div class="edit-actions">
              <rr-button variant="neutral-tinted" size="sm" @click="saveInput(index)">Opslaan</rr-button>
              <rr-button variant="neutral-tinted" size="sm" @click="cancelEdit">Annuleer</rr-button>
            </div>
          </div>
          <!-- Display mode -->
          <template v-else>
            <rr-text-cell>{{ input.name }} ({{ input.type }})<template v-if="input.source"> — {{ input.source }}</template></rr-text-cell>
            <rr-button-cell slot="end">
              <rr-button variant="neutral-tinted" size="sm" @click="editable && startEditInput(index)">Bewerk</rr-button>
            </rr-button-cell>
          </template>
        </rr-list-item>

        <!-- Add new input -->
        <rr-list-item v-if="editable && isEditing('new:input')" size="md">
          <div class="edit-form">
            <label>Naam</label>
            <input type="text" v-model="editValues.name" class="edit-input" placeholder="input_naam">
            <label>Type</label>
            <select v-model="editValues.type" class="edit-input">
              <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
            </select>
            <label>Bron regelgeving</label>
            <input type="text" v-model="editValues.sourceRegulation" class="edit-input">
            <label>Bron output</label>
            <input type="text" v-model="editValues.sourceOutput" class="edit-input">
            <div class="edit-actions">
              <rr-button variant="neutral-tinted" size="sm" @click="saveNewInput">Opslaan</rr-button>
              <rr-button variant="neutral-tinted" size="sm" @click="cancelEdit">Annuleer</rr-button>
            </div>
          </div>
        </rr-list-item>
        <rr-list-item v-if="editable && !isEditing('new:input')" size="md">
          <button class="add-button" @click="addInput">+ Nieuwe input</button>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>

    <!-- Outputs -->
    <template v-if="outputs.length || editable">
      <h3 class="machine-section-title">Outputs</h3>
      <rr-list variant="box">
        <rr-list-item v-for="(output, index) in outputs" :key="output.name" size="md">
          <!-- Edit mode -->
          <div v-if="editable && isEditing(`output:${index}`)" class="edit-form">
            <label>Naam</label>
            <input type="text" v-model="editValues.name" class="edit-input">
            <label>Type</label>
            <select v-model="editValues.type" class="edit-input">
              <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
            </select>
            <div class="edit-actions">
              <rr-button variant="neutral-tinted" size="sm" @click="saveOutput(index)">Opslaan</rr-button>
              <rr-button variant="neutral-tinted" size="sm" @click="cancelEdit">Annuleer</rr-button>
            </div>
          </div>
          <!-- Display mode -->
          <template v-else>
            <rr-text-cell>{{ output.name }} ({{ output.type }})</rr-text-cell>
            <rr-button-cell slot="end">
              <rr-button variant="neutral-tinted" size="sm" @click="editable && startEditOutput(index)">Bewerk</rr-button>
            </rr-button-cell>
          </template>
        </rr-list-item>

        <!-- Add new output -->
        <rr-list-item v-if="editable && isEditing('new:output')" size="md">
          <div class="edit-form">
            <label>Naam</label>
            <input type="text" v-model="editValues.name" class="edit-input" placeholder="output_naam">
            <label>Type</label>
            <select v-model="editValues.type" class="edit-input">
              <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
            </select>
            <div class="edit-actions">
              <rr-button variant="neutral-tinted" size="sm" @click="saveNewOutput">Opslaan</rr-button>
              <rr-button variant="neutral-tinted" size="sm" @click="cancelEdit">Annuleer</rr-button>
            </div>
          </div>
        </rr-list-item>
        <rr-list-item v-if="editable && !isEditing('new:output')" size="md">
          <button class="add-button" @click="addOutput">+ Nieuwe output</button>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>

    <!-- Acties -->
    <template v-if="actions.length">
      <h3 class="machine-section-title">Acties</h3>
      <rr-list variant="box">
        <rr-list-item v-for="action in actions" :key="action.output" size="md">
          <rr-text-cell>{{ action.output }}</rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm" @click="emit('open-action', action)">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>
  </template>
</template>

<style>
.machine-section-title {
  font-family: var(--rr-font-family-title, 'RijksSansVF', sans-serif);
  font-weight: 550;
  font-size: 16px;
  line-height: 1.3;
  color: var(--semantics-text-primary-color, #333B44);
  margin: 0 0 4px 0;
}
rr-list-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  min-height: 36px;
}
rr-list[variant="box"] rr-list-item + rr-list-item {
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

/* Edit form */
.edit-form {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 6px 12px;
  align-items: center;
  padding: 8px 0;
  width: 100%;
}
.edit-form label {
  font-size: 13px;
  font-weight: 500;
  color: var(--semantics-text-secondary-color, #6B7280);
}
.edit-input {
  padding: 6px 10px;
  border: 1px solid var(--semantics-dividers-color, #D1D5DB);
  border-radius: 6px;
  font-size: 13px;
  font-family: inherit;
  background: white;
  color: var(--semantics-text-primary-color, #333B44);
  outline: none;
  min-width: 0;
}
.edit-input:focus {
  border-color: #154273;
  box-shadow: 0 0 0 2px rgba(21, 66, 115, 0.15);
}
.edit-input--disabled {
  background: var(--semantics-surfaces-tinted-background-color, #F4F6F9);
  color: #999;
}
.edit-checkbox {
  width: 18px;
  height: 18px;
  justify-self: start;
  accent-color: #154273;
}
.edit-input-group {
  display: flex;
  align-items: center;
  gap: 4px;
}
.edit-input-group .edit-input {
  flex: 1;
  min-width: 0;
}
.edit-input-prefix,
.edit-input-suffix {
  font-size: 13px;
  font-weight: 500;
  color: var(--semantics-text-secondary-color, #6B7280);
  flex-shrink: 0;
}
.edit-actions {
  grid-column: 1 / -1;
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  padding-top: 4px;
}
.add-button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 14px;
  border: none;
  border-radius: 20px;
  background: var(--semantics-surfaces-tinted-background-color, #E8EBF0);
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  color: var(--semantics-text-primary-color, #4B5563);
  cursor: pointer;
}
.add-button:hover {
  background: #D5DAE1;
  color: #333B44;
}
</style>

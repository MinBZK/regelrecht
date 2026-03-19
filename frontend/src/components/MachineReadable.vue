<script setup>
import { computed } from 'vue';

const props = defineProps({
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
});

const emit = defineEmits(['open-action', 'open-edit']);

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

// Open edit sheet for existing items
function editDef(name) {
  const rawDef = mr.value?.definitions?.[name];
  emit('open-edit', { section: 'definition', key: name, rawDef });
}

function editParam(index) {
  const p = execution.value?.parameters?.[index];
  if (p) emit('open-edit', { section: 'parameter', index, data: p });
}

function editInput(index) {
  const raw = execution.value?.input?.[index];
  if (raw) emit('open-edit', { section: 'input', index, data: raw });
}

function editOutput(index) {
  const raw = execution.value?.output?.[index];
  if (raw) emit('open-edit', { section: 'output', index, data: raw });
}

// Open edit sheet for new items
function addDef() {
  emit('open-edit', { section: 'add-definition', isNew: true });
}

function addParam() {
  emit('open-edit', { section: 'add-parameter', isNew: true });
}

function addInput() {
  emit('open-edit', { section: 'add-input', isNew: true });
}

function addOutput() {
  emit('open-edit', { section: 'add-output', isNew: true });
}
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
          <rr-text-cell>{{ def.name }} = {{ formatValue(def.value, def.unit) }}</rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm" @click="editable && editDef(def.name)">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
        <rr-list-item v-if="editable" size="md">
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
          <rr-text-cell>{{ param.name }} ({{ param.type }})</rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm" @click="editable && editParam(index)">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
        <rr-list-item v-if="editable" size="md">
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
          <rr-text-cell>{{ input.name }} ({{ input.type }})<template v-if="input.source"> — {{ input.source }}</template></rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm" @click="editable && editInput(index)">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
        <rr-list-item v-if="editable" size="md">
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
          <rr-text-cell>{{ output.name }} ({{ output.type }})</rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm" @click="editable && editOutput(index)">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
        <rr-list-item v-if="editable" size="md">
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

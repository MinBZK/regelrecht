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
    if (val > 0 && val < 1 && !unit) {
      return (val * 100).toLocaleString('nl-NL', { maximumFractionDigits: 3 }) + '%';
    }
  }
  return String(val);
}

// Open edit sheet for existing items
function editDef(name) {
  const rawDef = mr.value?.definitions?.[name];
  if (rawDef == null) return;
  emit('open-edit', { section: 'definition', key: name, rawDef: JSON.parse(JSON.stringify(rawDef)) });
}

function editParam(index) {
  const p = execution.value?.parameters?.[index];
  if (p) emit('open-edit', { section: 'parameter', index, data: JSON.parse(JSON.stringify(p)) });
}

function editInput(index) {
  const raw = execution.value?.input?.[index];
  if (raw) emit('open-edit', { section: 'input', index, data: JSON.parse(JSON.stringify(raw)) });
}

function editOutput(index) {
  const raw = execution.value?.output?.[index];
  if (raw) emit('open-edit', { section: 'output', index, data: JSON.parse(JSON.stringify(raw)) });
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
  <div v-if="!mr" style="padding: 32px; color: var(--semantics-text-secondary-color, #999); text-align: center;">
    Geen machine-leesbare gegevens voor dit artikel
  </div>

  <div v-else>
    <!-- Metadata: produces -->
    <rr-list v-if="produces" variant="box">
      <rr-list-item v-if="produces.legal_character" size="md">
        <rr-text-cell>Juridische basis</rr-text-cell>
        <rr-cell>
          <rr-button variant="neutral-tinted" size="md" expandable>
            {{ produces.legal_character }}
          </rr-button>
        </rr-cell>
      </rr-list-item>
      <rr-list-item v-if="produces.decision_type" size="md">
        <rr-text-cell>Besluit-type</rr-text-cell>
        <rr-cell>
          <rr-button variant="neutral-tinted" size="md" expandable>
            {{ produces.decision_type }}
          </rr-button>
        </rr-cell>
      </rr-list-item>
    </rr-list>

    <rr-spacer v-if="produces" size="12"></rr-spacer>

    <!-- Definities -->
    <template v-if="definitions.length || editable">
      <rr-title-bar size="5">Definities</rr-title-bar>
      <rr-spacer size="4"></rr-spacer>
      <rr-list variant="box">
        <rr-list-item v-for="def in definitions" :key="def.name" size="md">
          <rr-text-cell>{{ def.name }} = {{ formatValue(def.value, def.unit) }}</rr-text-cell>
          <rr-cell v-if="editable">
            <rr-button variant="neutral-tinted" size="sm" @click="editDef(def.name)">Bewerk</rr-button>
          </rr-cell>
        </rr-list-item>
        <rr-list-item v-if="editable" size="md">
          <rr-button variant="neutral-tinted" size="sm" @click="addDef">
            <rr-icon slot="start" name="plus-small"></rr-icon>
            Nieuwe definitie
          </rr-button>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>

    <!-- Parameters -->
    <template v-if="parameters.length || editable">
      <rr-title-bar size="5">Parameters</rr-title-bar>
      <rr-spacer size="4"></rr-spacer>
      <rr-list variant="box">
        <rr-list-item v-for="(param, index) in parameters" :key="param.name" size="md">
          <rr-text-cell>{{ param.name }} ({{ param.type }})</rr-text-cell>
          <rr-cell v-if="editable">
            <rr-button variant="neutral-tinted" size="sm" @click="editParam(index)">Bewerk</rr-button>
          </rr-cell>
        </rr-list-item>
        <rr-list-item v-if="editable" size="md">
          <rr-button variant="neutral-tinted" size="sm" @click="addParam">
            <rr-icon slot="start" name="plus-small"></rr-icon>
            Nieuwe parameter
          </rr-button>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>

    <!-- Inputs -->
    <template v-if="inputs.length || editable">
      <rr-title-bar size="5">Inputs</rr-title-bar>
      <rr-spacer size="4"></rr-spacer>
      <rr-list variant="box">
        <rr-list-item v-for="(input, index) in inputs" :key="input.name" size="md">
          <rr-text-cell>{{ input.name }} ({{ input.type }})<template v-if="input.source"> — {{ input.source }}</template></rr-text-cell>
          <rr-cell v-if="editable">
            <rr-button variant="neutral-tinted" size="sm" @click="editInput(index)">Bewerk</rr-button>
          </rr-cell>
        </rr-list-item>
        <rr-list-item v-if="editable" size="md">
          <rr-button variant="neutral-tinted" size="sm" @click="addInput">
            <rr-icon slot="start" name="plus-small"></rr-icon>
            Nieuwe input
          </rr-button>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>

    <!-- Outputs -->
    <template v-if="outputs.length || editable">
      <rr-title-bar size="5">Outputs</rr-title-bar>
      <rr-spacer size="4"></rr-spacer>
      <rr-list variant="box">
        <rr-list-item v-for="(output, index) in outputs" :key="output.name" size="md">
          <rr-text-cell>{{ output.name }} ({{ output.type }})</rr-text-cell>
          <rr-cell v-if="editable">
            <rr-button variant="neutral-tinted" size="sm" @click="editOutput(index)">Bewerk</rr-button>
          </rr-cell>
        </rr-list-item>
        <rr-list-item v-if="editable" size="md">
          <rr-button variant="neutral-tinted" size="sm" @click="addOutput">
            <rr-icon slot="start" name="plus-small"></rr-icon>
            Nieuwe output
          </rr-button>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>

    <!-- Acties -->
    <template v-if="actions.length">
      <rr-title-bar size="5">Acties</rr-title-bar>
      <rr-spacer size="4"></rr-spacer>
      <rr-list variant="box">
        <rr-list-item v-for="(action, index) in actions" :key="index" size="md">
          <rr-text-cell>{{ action.output }}</rr-text-cell>
          <rr-cell>
            <rr-button variant="neutral-tinted" size="sm" @click="emit('open-action', action)">{{ editable ? 'Bewerk' : 'Bekijk' }}</rr-button>
          </rr-cell>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="12"></rr-spacer>
    </template>
  </div>
</template>

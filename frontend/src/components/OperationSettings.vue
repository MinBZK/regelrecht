<script setup>
import { computed } from 'vue';
import {
  OPERATION_LABELS,
  collectAvailableVariables,
  describeSubtitle,
  formatValueLabel,
} from '../utils/operationTree.js';

const props = defineProps({
  operation: { type: Object, default: null },
  article: { type: Object, default: null },
});

const emit = defineEmits(['select-operation']);

const availableVariables = computed(() => collectAvailableVariables(props.article));

const typeOptions = computed(() =>
  Object.entries(OPERATION_LABELS).map(([key, label]) => ({ value: key, label }))
);

const variableOptions = computed(() =>
  availableVariables.value.map(v => ({
    value: v.ref,
    label: `${v.name.replace(/_/g, ' ')} (${v.category.toLowerCase()})`,
  }))
);

const COMPARISON_OPS = new Set([
  'EQUALS', 'NOT_EQUALS', 'GREATER_THAN', 'GREATER_THAN_OR_EQUAL',
  'LESS_THAN', 'LESS_THAN_OR_EQUAL', 'NOT_NULL', 'IN', 'NOT_IN',
]);

const isComparisonOp = computed(() => COMPARISON_OPS.has(props.operation?.operation));

const operationValues = computed(() => {
  const node = props.operation?.node;
  if (!node) return [];

  if (isComparisonOp.value) {
    const vals = [];
    if (node.subject != null) vals.push({ _label: 'Onderwerp', _value: node.subject, _kind: 'subject' });
    if (node.value !== undefined) vals.push({ _label: 'Waarde', _value: node.value, _kind: 'value' });
    return vals;
  }

  if (node.operation === 'IF') {
    const vals = [];
    if (node.when) vals.push({ _label: 'Voorwaarde', _value: node.when, _kind: 'operation' });
    if (node.then !== undefined) vals.push({ _label: 'Dan', _value: node.then, _kind: 'value' });
    if (node.else !== undefined) vals.push({ _label: 'Anders', _value: node.else, _kind: 'value' });
    return vals;
  }

  if (node.operation === 'SWITCH') {
    const vals = [];
    if (Array.isArray(node.cases)) {
      node.cases.forEach((c, i) => {
        if (c.when !== undefined) vals.push({ _label: `Geval ${i + 1} — als`, _value: c.when, _kind: 'value' });
        if (c.then !== undefined) vals.push({ _label: `Geval ${i + 1} — dan`, _value: c.then, _kind: 'value' });
      });
    }
    if (node.default !== undefined) vals.push({ _label: 'Standaard', _value: node.default, _kind: 'value' });
    return vals;
  }

  if (Array.isArray(node.values)) {
    return node.values.map((v, i) => ({ _label: `Waarde ${i + 1}`, _value: v, _kind: 'value' }));
  }
  if (Array.isArray(node.conditions)) {
    return node.conditions.map((c, i) => ({ _label: `Conditie ${i + 1}`, _value: c, _kind: 'condition' }));
  }

  const vals = [];
  if (node.subject != null) vals.push({ _label: 'Onderwerp', _value: node.subject, _kind: 'subject' });
  if (node.value !== undefined) vals.push({ _label: 'Waarde', _value: node.value, _kind: 'value' });
  return vals;
});

function isNestedOperation(val) {
  return val != null && typeof val === 'object' && val.operation;
}

function isLiteralValue(val) {
  return val === null || typeof val === 'number' || typeof val === 'boolean' || (typeof val === 'string' && !val.startsWith('$'));
}

function valueDropdownOptions(val) {
  const opts = [...variableOptions.value];
  if (isNestedOperation(val)) {
    const label = formatValueLabel(val) + ' (operatie)';
    opts.unshift({ value: '__nested__', label });
  }
  return opts;
}

function currentDropdownValue(val) {
  if (isNestedOperation(val)) return '__nested__';
  if (typeof val === 'string' && val.startsWith('$')) return val;
  return String(val);
}
</script>

<template>
  <div v-if="operation">
    <div class="settings-title-bar">
      <rr-title-bar size="4">Instellingen operatie {{ operation.number }}</rr-title-bar>
      <rr-icon-button variant="neutral-tinted" size="s" icon="ellipsis" title="Meer opties">
      </rr-icon-button>
    </div>

    <rr-list variant="box" class="settings-list">
      <!-- Titel -->
      <rr-list-item size="md">
        <rr-text-cell>Titel</rr-text-cell>
        <rr-cell>
          <rr-text-field size="md" :value="operation.title"></rr-text-field>
        </rr-cell>
      </rr-list-item>

      <!-- Type -->
      <rr-list-item size="md">
        <rr-text-cell>Type</rr-text-cell>
        <rr-cell>
          <rr-dropdown size="md">
            <select aria-label="Operatie type" :value="operation.operation">
              <option v-for="opt in typeOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
            </select>
          </rr-dropdown>
        </rr-cell>
      </rr-list-item>

      <!-- Waarde rows -->
      <rr-list-item v-for="(val, i) in operationValues" :key="i" size="md">
        <rr-text-cell>{{ val._label }}</rr-text-cell>
        <rr-cell>
          <div class="value-row">
            <template v-if="isLiteralValue(val._value)">
              <rr-text-field size="md" :value="String(val._value)" is-full-width></rr-text-field>
            </template>
            <template v-else>
              <rr-dropdown size="md" style="flex: 1; min-width: 0;">
                <select :aria-label="val._label" :value="currentDropdownValue(val._value)">
                  <option v-for="opt in valueDropdownOptions(val._value)" :key="opt.value" :value="opt.value" :selected="opt.value === currentDropdownValue(val._value)">{{ opt.label }}</option>
                </select>
              </rr-dropdown>
            </template>
            <rr-icon-button variant="neutral-tinted" size="s" icon="minus" title="Verwijder waarde">
            </rr-icon-button>
          </div>
          <p v-if="isNestedOperation(val._value)" class="value-help-text">
            {{ describeSubtitle(val._value) }}
            <a href="#" @click.prevent="emit('select-operation', val._value)">Bewerk</a>
          </p>
        </rr-cell>
      </rr-list-item>

      <!-- Add value -->
      <rr-list-item size="md">
        <rr-button variant="neutral-tinted" size="md" style="width: 100%;">
          <rr-icon slot="start" name="plus-small"></rr-icon>
          Voeg waarde toe
        </rr-button>
      </rr-list-item>
    </rr-list>
  </div>
</template>

<style>
.settings-title-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 4px;
}


.value-row {
  display: flex;
  gap: 8px;
  align-items: center;
  width: 100%;
}
.value-row rr-text-field,
.value-row rr-dropdown {
  flex: 1;
  min-width: 0;
}

.value-help-text {
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
  font-size: 14px;
  font-weight: 400;
  line-height: 1.25;
  color: var(--semantics-text-secondary-color, #545D68);
  margin: 2px 0 0 0;
}

.value-help-text a {
  color: var(--semantics-text-accent-color, #007BC7);
  text-decoration: none;
}

.value-help-text a:hover {
  text-decoration: underline;
}
</style>

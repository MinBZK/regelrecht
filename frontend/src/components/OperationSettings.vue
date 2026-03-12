<script setup>
import { computed } from 'vue';
import { OPERATION_LABELS, describeSubtitle, formatValueLabel } from '../utils/operationTree.js';

const props = defineProps({
  operation: { type: Object, default: null },
});

const operationValues = computed(() => {
  const node = props.operation?.node;
  if (!node) return [];
  if (Array.isArray(node.values)) return node.values;
  if (Array.isArray(node.conditions)) return node.conditions;
  if (node.operation === 'IF') {
    const vals = [];
    if (node.when) vals.push({ _label: 'Voorwaarde', _value: node.when });
    if (node.then !== undefined) vals.push({ _label: 'Dan', _value: node.then });
    if (node.else !== undefined) vals.push({ _label: 'Anders', _value: node.else });
    return vals;
  }
  const vals = [];
  if (node.subject != null) vals.push(node.subject);
  if (node.value !== undefined) vals.push(node.value);
  return vals;
});

const operationLabel = computed(() =>
  OPERATION_LABELS[props.operation?.operation] || props.operation?.operation || ''
);

function isNestedOperation(val) {
  return val != null && typeof val === 'object' && val.operation;
}
</script>

<template>
  <div v-if="operation">
    <div class="settings-title-bar">
      <h3 class="section-title" style="margin: 0;">Instellingen operatie {{ operation.number }}</h3>
      <rr-icon-button variant="neutral-tinted" size="s" title="Meer opties">
        <img slot="__icon" src="/assets/icons/ellipsis-horizontal.svg" alt="" width="20" height="20">
      </rr-icon-button>
    </div>

    <rr-list variant="box">
      <!-- Titel -->
      <rr-list-item size="md">
        <rr-label-cell>Titel</rr-label-cell>
        <rr-button-cell slot="end">
          <rr-text-field size="md" :value="operation.title"></rr-text-field>
        </rr-button-cell>
      </rr-list-item>

      <!-- Type -->
      <rr-list-item size="md">
        <rr-label-cell>Type</rr-label-cell>
        <rr-button-cell slot="end">
          <rr-drop-down-field size="md" :value="operationLabel">
            <option v-for="(label, key) in OPERATION_LABELS" :key="key" :value="label" :selected="key === operation.operation || undefined">
              {{ label }}
            </option>
          </rr-drop-down-field>
        </rr-button-cell>
      </rr-list-item>

      <!-- Waarde rows -->
      <rr-list-item v-for="(val, i) in operationValues" :key="i" size="md">
        <rr-label-cell>{{ val._label || ('Waarde ' + (i + 1)) }}</rr-label-cell>
        <rr-button-cell slot="end">
          <div class="value-row">
            <rr-drop-down-field size="md" is-full-width :value="formatValueLabel(val._value ?? val)">
              <option selected>{{ formatValueLabel(val._value ?? val) }}</option>
            </rr-drop-down-field>
            <rr-icon-button variant="neutral-tinted" size="s" title="Verwijder waarde">
              <img slot="__icon" src="/assets/icons/minus.svg" alt="" width="20" height="20">
            </rr-icon-button>
          </div>
          <p v-if="isNestedOperation(val._value ?? val)" class="value-help-text">
            {{ describeSubtitle(val._value ?? val) }}
            <a href="#" @click.prevent>Bewerk</a>
          </p>
        </rr-button-cell>
      </rr-list-item>

      <!-- Add value -->
      <rr-list-item size="md">
        <rr-button variant="neutral-tinted" size="md" style="width: 100%;" has-leading-icon>
          <img slot="icon-start" src="/assets/icons/plus.svg" alt="" width="16" height="16">
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
  margin-bottom: 8px;
}

.value-row {
  display: flex;
  gap: 8px;
  align-items: center;
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

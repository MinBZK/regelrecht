<script setup>
import { computed } from 'vue';
import { centsToEuros, eurosToCents } from '../utils/currency.js';

// Generic, datatype-driven scenario input control. Given a declared datatype
// (and optional unit), it renders the matching NDD component and emits a
// correctly-typed JS value via `update`. It stays purely presentational: the
// parent owns the value store and the `change`/auto-execute side effects.
//
// Round-trip is handled here: scenario values often arrive as strings parsed
// from Gherkin ("true" / "1500" / "2025-01-01"), and downstream consumers
// (engine execute + Gherkin re-serialisation) accept real JS types — so we
// coerce to the right type on the way out. See ScenarioForm.execute() and
// formMapper.syncEditedValues().
const props = defineProps({
  /** Declared datatype: string | number | boolean | amount | date */
  type: { type: String, default: 'string' },
  /** type_spec.unit for amounts (e.g. 'eurocent' | 'euro'); null when unannotated */
  unit: { type: String, default: null },
  /** Raw stored value (string or already-typed) */
  value: { default: '' },
  /** Parameter name (used as the boolean switch label) */
  name: { type: String, default: '' },
  /** Mark the field invalid */
  invalid: { type: Boolean, default: false },
  /** aria error-message id(s) to associate with the field when invalid */
  errorMessageIds: { type: String, default: undefined },
});

const emit = defineEmits(['update']);

// Amounts whose unit is eurocent are stored as integer cents but entered in
// euros. Any other amount (unit 'euro' or unannotated) is entered raw, so we
// never silently scale a value whose unit we don't actually know.
const isEurocent = computed(() => props.type === 'amount' && props.unit === 'eurocent');

// Interpret the incoming (possibly string) value for display per datatype.
const displayValue = computed(() => {
  const v = props.value;
  switch (props.type) {
    case 'boolean':
      return v === true || v === 'true' || v === '1';
    case 'number':
      return v === '' || v == null ? '' : Number(v);
    case 'amount':
      if (v === '' || v == null) return '';
      return isEurocent.value ? centsToEuros(v) : Number(v);
    default:
      // date + string
      return String(v ?? '');
  }
});

function emitNumber(detailValue) {
  emit('update', detailValue === '' || detailValue == null ? '' : Number(detailValue));
}

function emitAmount(detailValue) {
  if (detailValue === '' || detailValue == null) return emit('update', '');
  emit('update', isEurocent.value ? eurosToCents(detailValue) : Number(detailValue));
}
</script>

<template>
  <!-- boolean -> switch (consistent with EditSheet's boolean control) -->
  <nldd-switch-field
    v-if="type === 'boolean'"
    :checked="displayValue ? true : undefined"
    @change="emit('update', Boolean($event.detail?.checked))"
  >{{ name }}</nldd-switch-field>

  <!-- amount -> number-field; enters euros (eurocent unit) or raw otherwise.
       Both @input and @change are intentional: @input gives live updates,
       @change delivers the locale-normalised value on commit/blur. -->
  <nldd-number-field
    v-else-if="type === 'amount'"
    :value="displayValue"
    :invalid="invalid || undefined"
    step="0.01"
    width="full"
    hide-spin-buttons
    @input="emitAmount($event.detail?.value)"
    @change="emitAmount($event.detail?.value)"
  ></nldd-number-field>

  <!-- number -> number-field -->
  <nldd-number-field
    v-else-if="type === 'number'"
    :value="displayValue"
    :invalid="invalid || undefined"
    width="full"
    hide-spin-buttons
    @input="emitNumber($event.detail?.value)"
    @change="emitNumber($event.detail?.value)"
  ></nldd-number-field>

  <!-- date -> native date picker via text-field -->
  <nldd-text-field
    v-else-if="type === 'date'"
    size="md"
    type="date"
    :invalid="invalid || undefined"
    :error-message-ids="invalid ? errorMessageIds : undefined"
    :value="displayValue"
    @input="emit('update', $event.target?.value ?? $event.detail?.value ?? '')"
  ></nldd-text-field>

  <!-- string (fallback) -->
  <nldd-text-field
    v-else
    size="md"
    :invalid="invalid || undefined"
    :error-message-ids="invalid ? errorMessageIds : undefined"
    :value="displayValue"
    @input="emit('update', $event.target?.value ?? $event.detail?.value ?? '')"
  ></nldd-text-field>
</template>

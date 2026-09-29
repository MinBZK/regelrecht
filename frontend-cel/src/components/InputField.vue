<script setup>
// An input element for a form field type: text, number, date, choice,
// yes_no or checkbox. An unknown type is text.
//
// yes_no is a choice between Ja and Nee with "Kies" as the initial state: not
// answered stays null and does not silently become no. A checkbox is a
// declaration; not checked is not declared.
import { fieldText, options } from '../form.js';

const props = defineProps({
  kind: { type: String, default: 'text' },
  label: { type: String, required: true },
  choices: { type: Array, default: null },
  modelValue: { type: [String, Number, Boolean], default: null },
});
const emit = defineEmits(['update:modelValue']);

function yesNo(e) {
  const t = e.target?.value;
  return t === 'ja' ? true : t === 'nee' ? false : null;
}

// The value of a choice as the form names it (a number stays a number), not
// the text from the select.
function choice(e) {
  const t = e.target?.value ?? '';
  if (t === '') return null;
  return options(props.choices).find((o) => String(o.value) === t)?.value ?? t;
}

function number(e) {
  const t = String(fieldText(e)).trim();
  if (t === '') return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}
</script>

<template>
  <nldd-number-field
    v-if="kind === 'number'"
    :value="modelValue ?? ''"
    :accessible-label="label"
    @input="emit('update:modelValue', number($event))"
    @change="emit('update:modelValue', number($event))"
  ></nldd-number-field>
  <nldd-date-field
    v-else-if="kind === 'date'"
    :value="modelValue ?? ''"
    :accessible-label="label"
    @input="emit('update:modelValue', fieldText($event))"
    @change="emit('update:modelValue', fieldText($event))"
  ></nldd-date-field>
  <nldd-dropdown v-else-if="kind === 'choice'" :accessible-label="label">
    <select :value="modelValue === null ? '' : String(modelValue)" @change="emit('update:modelValue', choice($event))">
      <option value="">Kies</option>
      <option v-for="o in options(props.choices)" :key="String(o.value)" :value="String(o.value)">{{ o.label }}</option>
    </select>
  </nldd-dropdown>
  <nldd-dropdown v-else-if="kind === 'yes_no'" :accessible-label="label">
    <select :value="modelValue === true ? 'ja' : modelValue === false ? 'nee' : ''" @change="emit('update:modelValue', yesNo($event))">
      <option value="">Kies</option>
      <option value="ja">Ja</option>
      <option value="nee">Nee</option>
    </select>
  </nldd-dropdown>
  <nldd-checkbox-field
    v-else-if="kind === 'checkbox'"
    :label="label"
    :checked="modelValue === true || undefined"
    @change="emit('update:modelValue', $event.detail?.checked ?? $event.target?.checked ?? false)"
  ></nldd-checkbox-field>
  <nldd-text-field
    v-else
    :value="modelValue ?? ''"
    :accessible-label="label"
    @input="emit('update:modelValue', fieldText($event))"
  ></nldd-text-field>
</template>

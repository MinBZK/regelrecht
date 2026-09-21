<script setup>
// Een invoerelement voor een formuliersoort: tekst, getal, datum, keuze,
// janee of vink. Een onbekende soort is tekst.
import { opties } from '../formulier.js';

const props = defineProps({
  soort: { type: String, default: 'tekst' },
  label: { type: String, required: true },
  keuzes: { type: Array, default: null },
  modelValue: { type: [String, Number, Boolean], default: null },
});
const emit = defineEmits(['update:modelValue']);

function tekst(e) {
  return e.detail?.value ?? e.target?.value ?? '';
}

function getal(e) {
  const t = String(tekst(e)).trim();
  if (t === '') return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}
</script>

<template>
  <nldd-number-field
    v-if="soort === 'getal'"
    :value="modelValue ?? ''"
    :accessible-label="label"
    @input="emit('update:modelValue', getal($event))"
    @change="emit('update:modelValue', getal($event))"
  ></nldd-number-field>
  <nldd-date-field
    v-else-if="soort === 'datum'"
    :value="modelValue ?? ''"
    :accessible-label="label"
    @input="emit('update:modelValue', tekst($event))"
    @change="emit('update:modelValue', tekst($event))"
  ></nldd-date-field>
  <nldd-dropdown v-else-if="soort === 'keuze'" :accessible-label="label">
    <select :value="modelValue ?? ''" @change="emit('update:modelValue', $event.target.value || null)">
      <option value="">Kies</option>
      <option v-for="o in opties(props.keuzes)" :key="String(o.waarde)" :value="o.waarde">{{ o.label }}</option>
    </select>
  </nldd-dropdown>
  <nldd-checkbox-field
    v-else-if="soort === 'janee' || soort === 'vink'"
    :label="label"
    :checked="modelValue === true || undefined"
    @change="emit('update:modelValue', $event.detail?.checked ?? $event.target?.checked ?? false)"
  ></nldd-checkbox-field>
  <nldd-text-field
    v-else
    :value="modelValue ?? ''"
    :accessible-label="label"
    @input="emit('update:modelValue', tekst($event))"
  ></nldd-text-field>
</template>

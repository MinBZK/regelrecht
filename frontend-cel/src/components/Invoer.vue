<script setup>
// Een invoerelement voor een formuliersoort: tekst, getal, datum, keuze,
// janee of vink. Een onbekende soort is tekst.
//
// janee is een keuze uit Ja en Nee met "Kies" als beginstand: niet
// beantwoord blijft null en wordt niet stilzwijgend nee. Een vink is een
// verklaring; niet aangevinkt is niet verklaard.
import { opties, veldTekst } from '../formulier.js';

const props = defineProps({
  soort: { type: String, default: 'tekst' },
  label: { type: String, required: true },
  keuzes: { type: Array, default: null },
  modelValue: { type: [String, Number, Boolean], default: null },
});
const emit = defineEmits(['update:modelValue']);


function janee(e) {
  const t = e.target?.value;
  return t === 'ja' ? true : t === 'nee' ? false : null;
}

// De waarde van een keuze zoals het formulier haar noemt (een getal blijft
// een getal), niet de tekst uit de select.
function keuze(e) {
  const t = e.target?.value ?? '';
  if (t === '') return null;
  return opties(props.keuzes).find((o) => String(o.waarde) === t)?.waarde ?? t;
}

function getal(e) {
  const t = String(veldTekst(e)).trim();
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
    @input="emit('update:modelValue', veldTekst($event))"
    @change="emit('update:modelValue', veldTekst($event))"
  ></nldd-date-field>
  <nldd-dropdown v-else-if="soort === 'keuze'" :accessible-label="label">
    <select :value="modelValue === null ? '' : String(modelValue)" @change="emit('update:modelValue', keuze($event))">
      <option value="">Kies</option>
      <option v-for="o in opties(props.keuzes)" :key="String(o.waarde)" :value="String(o.waarde)">{{ o.label }}</option>
    </select>
  </nldd-dropdown>
  <nldd-dropdown v-else-if="soort === 'janee'" :accessible-label="label">
    <select :value="modelValue === true ? 'ja' : modelValue === false ? 'nee' : ''" @change="emit('update:modelValue', janee($event))">
      <option value="">Kies</option>
      <option value="ja">Ja</option>
      <option value="nee">Nee</option>
    </select>
  </nldd-dropdown>
  <nldd-checkbox-field
    v-else-if="soort === 'vink'"
    :label="label"
    :checked="modelValue === true || undefined"
    @change="emit('update:modelValue', $event.detail?.checked ?? $event.target?.checked ?? false)"
  ></nldd-checkbox-field>
  <nldd-text-field
    v-else
    :value="modelValue ?? ''"
    :accessible-label="label"
    @input="emit('update:modelValue', veldTekst($event))"
  ></nldd-text-field>
</template>

<script setup>
import { fieldValue } from '../world/events.js';

// "Aanvragen als": de keuze van een fictieve aanvrager uit het portaal.
//
// Een mock-login zonder authenticatie. De keuze gaat naar de server en hoort bij
// de sessie; deze keuzelijst toont alleen wat het beeld zegt dat er gekozen is.
// Daardoor geldt een wissel op de ene pagina ook op de andere, en na verversen
// ook nog.

const props = defineProps({
  /** Het portaal uit het wereldbestand. */
  portaal: { type: Object, required: true },
  /** Het id van de gekozen persona, of leeg. */
  current: { type: String, default: '' },
  /** Staat er een wijziging onderweg? */
  busy: { type: Boolean, default: false },
});

const emit = defineEmits(['choose']);

function onChange(event) {
  const id = fieldValue(event, props.current);
  if (id !== props.current) emit('choose', id || null);
}
</script>

<template>
  <nldd-form-field label="Aanvragen als" supporting-label="fictief; er wordt niet ingelogd">
    <nldd-dropdown width="360px" :disabled="busy || undefined" @change="onChange">
      <!-- De keuze ook als waarde van de select, en niet alleen als
           `selected` per optie: bij een nieuw geplaatste keuzelijst zet Vue de
           waarde pas nadat de opties er staan, en dan staat hij op de persona
           en niet op de eerste optie die toevallig als eerste geplaatst werd. -->
      <select :value="current">
        <option value="" :selected="!current">Kies een aanvrager</option>
        <option
          v-for="persona in portaal.personas"
          :key="persona.id"
          :value="persona.id"
          :selected="persona.id === current"
        >
          {{ persona.label }}
        </option>
      </select>
    </nldd-dropdown>
  </nldd-form-field>
</template>

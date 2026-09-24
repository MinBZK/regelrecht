<script setup>
// Nagebootste login van een medewerker: alleen een naam. Er is geen register
// en geen wachtwoord; het proces controleert alleen dat er een naam is.
import { inject, ref } from 'vue';

const api = inject('api');
const emit = defineEmits(['ingelogd']);

const naam = ref('');
const fout = ref('');
const bezig = ref(false);

async function inloggen() {
  fout.value = '';
  bezig.value = true;
  try {
    emit('ingelogd', await api.medewerkerInloggen(naam.value));
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-title size="2"><h1>Inloggen als behandelaar</h1></nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>Dit is een nagebootste inlog voor een medewerker van de cel. Er is geen register: vul uw naam in.</p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <nldd-form novalidate @submit.prevent="inloggen">
    <nldd-form-field label="Naam">
      <nldd-text-field :value="naam" name="naam" @input="naam = $event.detail?.value ?? $event.target?.value ?? ''"></nldd-text-field>
    </nldd-form-field>
    <template v-if="fout">
      <nldd-inline-dialog variant="alert" text="Inloggen lukt niet" :supporting-text="fout"></nldd-inline-dialog>
    </template>
    <nldd-form-actions>
      <nldd-button variant="primary" type="submit" text="Inloggen" :loading="bezig || undefined"></nldd-button>
    </nldd-form-actions>
  </nldd-form>
</template>

<script setup>
// Nep-eHerkenning: KvK-nummer en persoon. De cel controleert alleen de vorm.
// Of de persoon namens de organisatie mag handelen, zegt het handelsregister,
// niet deze inlog.
import { inject, ref } from 'vue';

const api = inject('api');

const emit = defineEmits(['ingelogd']);

const kvk = ref('');
const persoon = ref('');
const fout = ref('');
const bezig = ref(false);

function tekst(e) {
  return e.detail?.value ?? e.target?.value ?? '';
}

async function inloggen() {
  fout.value = '';
  bezig.value = true;
  try {
    const sessie = await api.inloggen({ kvk: kvk.value, persoon: persoon.value });
    emit('ingelogd', sessie);
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-title size="2"><h1>Inloggen met eHerkenning</h1></nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>Dit is een nagebootste inlog. Vul een KvK-nummer van acht cijfers in en uw naam. Of u namens de vereniging mag handelen, haalt het portaal uit het handelsregister.</p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <nldd-form novalidate @submit.prevent="inloggen">
    <nldd-form-field label="KvK-nummer">
      <nldd-text-field :value="kvk" name="kvk" keyboard="numeric" @input="kvk = tekst($event)"></nldd-text-field>
    </nldd-form-field>
    <nldd-form-field label="Uw naam">
      <nldd-text-field :value="persoon" name="persoon" @input="persoon = tekst($event)"></nldd-text-field>
    </nldd-form-field>
    <template v-if="fout">
      <nldd-inline-dialog variant="alert" text="Inloggen lukt niet" :supporting-text="fout"></nldd-inline-dialog>
    </template>
    <nldd-form-actions>
      <nldd-button variant="primary" type="submit" text="Inloggen" :loading="bezig || undefined"></nldd-button>
    </nldd-form-actions>
  </nldd-form>
</template>

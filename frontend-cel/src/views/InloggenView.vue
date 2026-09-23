<script setup>
// Nep-eHerkenning: KvK-nummer, persoon en machtiging. Er is geen register;
// de cel controleert alleen de vorm. Wat een ontbrekende machtiging
// betekent, zegt daarna de regeling, niet deze inlog.
import { inject, ref } from 'vue';

const api = inject('api');

const emit = defineEmits(['ingelogd']);

const kvk = ref('');
const persoon = ref('');
const machtiging = ref('volledig');
const fout = ref('');
const bezig = ref(false);

function tekst(e) {
  return e.detail?.value ?? e.target?.value ?? '';
}

async function inloggen() {
  fout.value = '';
  bezig.value = true;
  try {
    const sessie = await api.inloggen({ kvk: kvk.value, persoon: persoon.value, machtiging: machtiging.value });
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
    <p>Dit is een nagebootste inlog. Er is geen register: vul een KvK-nummer van acht cijfers in, uw naam als gemachtigde, en de machtiging.</p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <nldd-form novalidate @submit.prevent="inloggen">
    <nldd-form-field label="KvK-nummer">
      <nldd-text-field :value="kvk" name="kvk" keyboard="numeric" @input="kvk = tekst($event)"></nldd-text-field>
    </nldd-form-field>
    <nldd-form-field label="Naam van de gemachtigde">
      <nldd-text-field :value="persoon" name="persoon" @input="persoon = tekst($event)"></nldd-text-field>
    </nldd-form-field>
    <nldd-form-field label="Machtiging">
      <nldd-dropdown accessible-label="Machtiging">
        <select :value="machtiging" @change="machtiging = $event.target.value">
          <option value="volledig">Volledig</option>
          <option value="geen">Geen (niet gemachtigd voor deze dienst)</option>
        </select>
      </nldd-dropdown>
    </nldd-form-field>
    <template v-if="fout">
      <nldd-inline-dialog variant="alert" text="Inloggen lukt niet" :supporting-text="fout"></nldd-inline-dialog>
    </template>
    <nldd-form-actions>
      <nldd-button variant="primary" type="submit" text="Inloggen" :loading="bezig || undefined"></nldd-button>
    </nldd-form-actions>
  </nldd-form>
</template>

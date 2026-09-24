<script setup>
// Nep-eHerkenning: KvK-nummer en persoon. De cel controleert alleen de vorm.
// Of de persoon namens de organisatie mag handelen, zegt het handelsregister,
// niet deze inlog. Levert de cel inlogvoorbeelden, dan staat eronder per
// voorbeeld een knop om er direct mee in te loggen of het formulier ermee in
// te vullen.
import { inject, ref } from 'vue';

const api = inject('api');
const voorbeelden = inject('voorbeelden');

const emit = defineEmits(['ingelogd']);

const kvk = ref('');
const persoon = ref('');
const fout = ref('');
const bezig = ref(false);

function tekst(e) {
  return e.detail?.value ?? e.target?.value ?? '';
}

function invullen(v) {
  kvk.value = v.kvk;
  persoon.value = v.persoon;
}

async function inloggen() {
  if (bezig.value) return;
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
  <template v-if="voorbeelden.inloggen.length">
    <nldd-spacer size="32"></nldd-spacer>
    <nldd-title size="4"><h2>Voorbeelden</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-table columns="minmax(240px,1fr) auto" accessible-label="Inlogvoorbeelden">
      <nldd-table-row v-for="v in voorbeelden.inloggen" :key="v.label">
        <nldd-text-cell :text="v.label" :supporting-text="`KvK ${v.kvk}, ${v.persoon}`"></nldd-text-cell>
        <nldd-cell>
          <nldd-button-group orientation="horizontal">
            <nldd-button variant="secondary" size="sm" text="Vul in" :disabled="bezig || undefined" :accessible-label="`Vul in met ${v.label}`" @click="invullen(v)"></nldd-button>
            <nldd-button
              variant="secondary"
              size="sm"
              text="Inloggen"
              :accessible-label="`Inloggen met ${v.label}`"
              :loading="bezig || undefined"
              :disabled="bezig || undefined"
              @click="invullen(v); inloggen()"
            ></nldd-button>
          </nldd-button-group>
        </nldd-cell>
      </nldd-table-row>
    </nldd-table>
  </template>
</template>

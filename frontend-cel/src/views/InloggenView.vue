<script setup>
// Inloggen langs een kanaal uit de configuratie van het proces (`kanalen` in
// proces.yaml): het label, de uitleg en de velden komen uit het kanaal. Elk
// kanaal is nagebootst: het proces controleert alleen de vorm van wat u
// invult, niet wie u bent. Logt er langs het kanaal meer dan een rol in, dan
// gaat de gekozen rol mee. Levert het proces inlogvoorbeelden voor dit
// kanaal, dan staat eronder per voorbeeld een knop om er direct mee in te
// loggen of het formulier ermee in te vullen.
import { computed, inject, ref } from 'vue';
import { veldTekst } from '../formulier.js';
import { lege } from '../kanaal.js';

const props = defineProps({
  // De id van het kanaal en zijn beschrijving uit GET /api/processen.
  kanaalId: { type: String, required: true },
  kanaal: { type: Object, required: true },
  // De rol waarin wordt ingelogd; mee als het kanaal er meer heeft.
  rol: { type: String, default: null },
  rolMeesturen: { type: Boolean, default: false },
});

const api = inject('api');
const voorbeelden = inject('voorbeelden');

const emit = defineEmits(['ingelogd']);

const waarden = ref(lege(props.kanaal));
const fout = ref('');
const bezig = ref(false);

// De voorbeelden van dit kanaal, met hun velden in de volgorde van het kanaal.
const eigen = computed(() => voorbeelden.value.inloggen.filter((v) => v.kanaal === props.kanaalId));

function omschrijving(v) {
  return props.kanaal.velden.map((veld) => v.velden[veld.naam]).filter(Boolean).join(', ');
}

function invullen(v) {
  waarden.value = lege(props.kanaal, v.velden);
}

async function inloggen() {
  if (bezig.value) return;
  fout.value = '';
  bezig.value = true;
  try {
    const invoer = { ...waarden.value };
    if (props.rolMeesturen && props.rol) invoer.rol = props.rol;
    emit('ingelogd', await api.inloggen(props.kanaalId, invoer));
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-title size="2"><h1>{{ kanaal.label }}</h1></nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>Dit is een nagebootste inlog: het proces controleert alleen de vorm van wat u invult, niet wie u bent.</p>
    <p v-if="kanaal.uitleg">{{ kanaal.uitleg }}</p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <nldd-form novalidate @submit.prevent="inloggen">
    <nldd-form-field v-for="v in kanaal.velden" :key="v.naam" :label="v.label">
      <nldd-text-field
        :value="waarden[v.naam]"
        :name="v.naam"
        :keyboard="v.numeriek ? 'numeric' : undefined"
        @input="waarden = { ...waarden, [v.naam]: veldTekst($event) }"
      ></nldd-text-field>
    </nldd-form-field>
    <template v-if="fout">
      <nldd-inline-dialog variant="alert" text="Inloggen lukt niet" :supporting-text="fout"></nldd-inline-dialog>
    </template>
    <nldd-form-actions>
      <nldd-button variant="primary" type="submit" text="Inloggen" :loading="bezig || undefined"></nldd-button>
    </nldd-form-actions>
  </nldd-form>
  <template v-if="eigen.length">
    <nldd-spacer size="32"></nldd-spacer>
    <nldd-title size="4"><h2>Voorbeelden</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-table columns="minmax(240px,1fr) auto" accessible-label="Inlogvoorbeelden">
      <nldd-table-row v-for="v in eigen" :key="v.label">
        <nldd-text-cell :text="v.label" :supporting-text="omschrijving(v)"></nldd-text-cell>
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

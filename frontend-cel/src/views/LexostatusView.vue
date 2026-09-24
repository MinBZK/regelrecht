<script setup>
// Een lexostatus van de cel opvragen: kies de lexostatus, vul de inputs in,
// en zie de parameters die de reductie oplevert.
import { computed, inject, ref } from 'vue';
import { waardeTekst as waarde } from '../tekst.js';

const props = defineProps({ lexostatussen: { type: Array, required: true } });
// De routes van de cel: kroniek en lexostatus zijn van haar, niet van het
// proces.
const api = inject('celApi');

const naam = ref(props.lexostatussen[0]?.name ?? '');
const invoer = ref({});
const uitkomst = ref(null);
const fout = ref('');
const bezig = ref(false);

const definitie = computed(() => props.lexostatussen.find((l) => l.name === naam.value) ?? null);

function kies(e) {
  naam.value = e.target.value;
  invoer.value = {};
  uitkomst.value = null;
}

function tekst(e) {
  return e.detail?.value ?? e.target?.value ?? '';
}

async function opvragen() {
  fout.value = '';
  bezig.value = true;
  try {
    uitkomst.value = await api.lexostatus(naam.value, invoer.value);
  } catch (e) {
    uitkomst.value = null;
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

// Een lijst-lexostatus: een regel per zaak, met de kolommen als velden.
const lijst = computed(() => uitkomst.value?.lijst ?? null);
const kolommen = computed(() => definitie.value?.kolommen ?? []);

const rijen = computed(() => {
  const u = uitkomst.value;
  if (!u) return [];
  const uit = Object.entries(u.parameters ?? {}).map(([n, w]) => ({ naam: n, waarde: waarde(w), soort: 'parameter' }));
  for (const [n, w] of Object.entries(u.extra_velden ?? {})) uit.push({ naam: n, waarde: waarde(w), soort: 'extra veld (niet naar de engine)' });
  for (const n of u.niet_afgeleid ?? []) uit.push({ naam: n, waarde: '', soort: 'niet af te leiden' });
  return uit;
});
</script>

<template>
  <nldd-title size="2"><h1>Lexostatus</h1></nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <nldd-form novalidate @submit.prevent="opvragen">
    <nldd-form-field label="Lexostatus">
      <nldd-dropdown accessible-label="Lexostatus">
        <select :value="naam" @change="kies">
          <option v-for="l in lexostatussen" :key="l.name" :value="l.name">{{ l.name }}</option>
        </select>
      </nldd-dropdown>
    </nldd-form-field>
    <nldd-form-field v-for="i in definitie?.inputs ?? []" :key="i.name" :label="i.name" :supporting-label="i.type">
      <nldd-text-field
        :value="invoer[i.name] ?? ''"
        :accessible-label="i.name"
        @input="invoer = { ...invoer, [i.name]: tekst($event) }"
      ></nldd-text-field>
    </nldd-form-field>
    <template v-if="fout">
      <nldd-inline-dialog variant="alert" text="De lexostatus is niet op te vragen" :supporting-text="fout"></nldd-inline-dialog>
    </template>
    <nldd-form-actions>
      <nldd-button variant="primary" type="submit" text="Opvragen" :loading="bezig || undefined"></nldd-button>
    </nldd-form-actions>
  </nldd-form>
  <template v-if="lijst">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-table
      :columns="['minmax(280px,1.4fr)', ...kolommen.map(() => 'minmax(120px,1fr)')].join(' ')"
      accessible-label="Regels van de lijst"
      empty-text="Geen regels"
      empty-supporting-text="Een lijst gaat nooit naar de engine."
    >
      <nldd-table-row slot="header">
        <nldd-text-cell text="Zaakkenmerk"></nldd-text-cell>
        <nldd-text-cell v-for="k in kolommen" :key="k" :text="k"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="r in lijst" :key="r.zaakkenmerk">
        <nldd-text-cell :text="r.zaakkenmerk"></nldd-text-cell>
        <nldd-text-cell v-for="k in kolommen" :key="k" :text="waarde(r.velden[k])"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </template>
  <template v-else-if="uitkomst">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-table columns="minmax(200px,1fr) minmax(160px,1fr) minmax(200px,1fr)" accessible-label="Parameters van de lexostatus">
      <nldd-table-row slot="header">
        <nldd-text-cell text="Naam"></nldd-text-cell>
        <nldd-text-cell text="Waarde"></nldd-text-cell>
        <nldd-text-cell text="Soort"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="r in rijen" :key="r.naam">
        <nldd-text-cell :text="r.naam"></nldd-text-cell>
        <nldd-text-cell :text="r.waarde"></nldd-text-cell>
        <nldd-text-cell :text="r.soort"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </template>
</template>

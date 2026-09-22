<script setup>
// De grammen van de cel, als tabel met de ruwe YAML eronder. Bij een cel met
// een portaal alleen die van de ingelogde KvK.
import { inject, onMounted, ref } from 'vue';

const api = inject('api');

const props = defineProps({
  nieuw: { type: String, default: null },
  portaal: { type: Boolean, default: false },
});

const items = ref([]);
const fout = ref('');
const geladen = ref(false);

onMounted(async () => {
  try {
    items.value = (await api.kroniek()).reverse();
  } catch (e) {
    fout.value = e.message;
  } finally {
    geladen.value = true;
  }
});
</script>

<template>
  <nldd-title size="2"><h1>Kroniek</h1></nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="De kroniek is niet te laden" :supporting-text="fout"></nldd-inline-dialog>
  </template>
  <template v-else-if="geladen">
    <nldd-table
      columns="220px minmax(160px,1fr) 140px minmax(200px,1fr) 120px"
      accessible-label="Vastgelegde grammen"
      empty-text="Nog niets vastgelegd"
      :empty-supporting-text="props.portaal ? 'Een ingediende aanvraag verschijnt hier als gram.' : undefined"
    >
      <nldd-table-row slot="header">
        <nldd-text-cell text="Op moment"></nldd-text-cell>
        <nldd-text-cell text="Event"></nldd-text-cell>
        <nldd-text-cell text="Type"></nldd-text-cell>
        <nldd-text-cell text="Zaakkenmerk"></nldd-text-cell>
        <nldd-text-cell text="Herkomst"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="i in items" :key="i.gram.zaakkenmerk + i.gram.op_moment" :selected="i.gram.zaakkenmerk === props.nieuw || undefined">
        <nldd-text-cell :text="i.gram.op_moment"></nldd-text-cell>
        <nldd-text-cell :text="i.gram.name"></nldd-text-cell>
        <nldd-text-cell :text="[i.gram.type, i.gram.soort].filter(Boolean).join(' / ')"></nldd-text-cell>
        <nldd-text-cell :text="i.gram.zaakkenmerk"></nldd-text-cell>
        <nldd-text-cell :text="i.gram.herkomst ?? 'vastgesteld'"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
    <template v-for="i in items" :key="'yaml-' + i.gram.zaakkenmerk + i.gram.op_moment">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="5"><h2>{{ i.gram.name }}, zaak {{ i.gram.zaakkenmerk }}</h2></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-code-viewer language="yaml" wrap>{{ i.yaml }}</nldd-code-viewer>
    </template>
  </template>
</template>

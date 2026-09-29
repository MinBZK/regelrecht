<script setup>
// De werkvoorraad: een lijst-lexostatus met een regel per zaak. Welke zaken
// erin staan en met welke kolommen, zegt de lexostatus, niet de code.
import { computed, inject, onMounted, ref } from 'vue';
import { waardeTekst } from '../tekst.js';

const props = defineProps({ kolommen: { type: Array, default: () => [] } });
const emit = defineEmits(['open']);
const api = inject('api');

const lijst = ref([]);
const fout = ref('');
const geladen = ref(false);

onMounted(async () => {
  try {
    lijst.value = (await api.werkvoorraad()).list ?? [];
  } catch (e) {
    fout.value = e.message;
  } finally {
    geladen.value = true;
  }
});

const kolommen = computed(() => props.kolommen);
const sjabloon = computed(() => ['90px', 'minmax(280px,1.4fr)', ...kolommen.value.map(() => 'minmax(120px,1fr)')].join(' '));
</script>

<template>
  <nldd-title size="2">
    <h1>Werkvoorraad</h1>
    <span slot="subtitle">De zaken van de lijst-lexostatus van de werkvoorraad</span>
  </nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="De werkvoorraad is niet te laden" :supporting-text="fout"></nldd-inline-dialog>
  </template>
  <nldd-table v-else-if="geladen" :columns="sjabloon" accessible-label="Werkvoorraad" empty-text="Geen zaken in de werkvoorraad">
    <nldd-table-row slot="header">
      <nldd-text-cell text=""></nldd-text-cell>
      <nldd-text-cell text="Zaakkenmerk"></nldd-text-cell>
      <nldd-text-cell v-for="k in kolommen" :key="k" :text="k"></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="r in lijst" :key="r.root">
      <nldd-cell>
        <nldd-button variant="secondary" text="Open" :accessible-label="`Open zaak ${r.root}`" @click="emit('open', r.root)"></nldd-button>
      </nldd-cell>
      <nldd-text-cell :text="r.root"></nldd-text-cell>
      <nldd-text-cell v-for="k in kolommen" :key="k" :text="waardeTekst(r.fields[k])"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
</template>

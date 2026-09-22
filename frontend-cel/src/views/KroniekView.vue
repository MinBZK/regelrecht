<script setup>
// De grammen van de cel. De aanvrager ziet alleen die van zijn KvK, de
// behandelaar alle, en een cel zonder rollen toont ze aan iedereen.
import { inject, onMounted, ref } from 'vue';
import Grammen from '../components/Grammen.vue';

const api = inject('api');

const props = defineProps({
  // Het zojuist vastgelegde gram, om het in de tabel aan te wijzen.
  nieuw: { type: Object, default: null },
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
  <Grammen
    v-else-if="geladen"
    :items="items"
    :nieuw="props.nieuw"
    :leeg-tekst="props.portaal ? 'Een ingediende aanvraag verschijnt hier als gram.' : undefined"
  />
</template>

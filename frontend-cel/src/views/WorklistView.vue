<script setup>
// The worklist: a list lexostatus with a row per case. Which cases are in it
// and with which columns, the lexostatus says, not the code.
import { computed, inject, onMounted, ref } from 'vue';
import { valueText } from '../text.js';

const props = defineProps({ columns: { type: Array, default: () => [] } });
const emit = defineEmits(['open']);
const api = inject('api');

const list = ref([]);
const error = ref('');
const loaded = ref(false);

onMounted(async () => {
  try {
    list.value = (await api.worklist()).list ?? [];
  } catch (e) {
    error.value = e.message;
  } finally {
    loaded.value = true;
  }
});

const template = computed(() => ['90px', 'minmax(280px,1.4fr)', ...props.columns.map(() => 'minmax(120px,1fr)')].join(' '));
</script>

<template>
  <nldd-title size="2">
    <h1>Werkvoorraad</h1>
    <span slot="subtitle">De zaken van de lijst-lexostatus van de werkvoorraad</span>
  </nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="error">
    <nldd-inline-dialog variant="alert" text="De werkvoorraad is niet te laden" :supporting-text="error"></nldd-inline-dialog>
  </template>
  <nldd-table v-else-if="loaded" :columns="template" accessible-label="Werkvoorraad" empty-text="Geen zaken in de werkvoorraad">
    <nldd-table-row slot="header">
      <nldd-text-cell text=""></nldd-text-cell>
      <nldd-text-cell text="Zaakkenmerk"></nldd-text-cell>
      <nldd-text-cell v-for="c in columns" :key="c" :text="c"></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="r in list" :key="r.root">
      <nldd-cell>
        <nldd-button variant="secondary" text="Open" :accessible-label="`Open zaak ${r.root}`" @click="emit('open', r.root)"></nldd-button>
      </nldd-cell>
      <nldd-text-cell :text="r.root"></nldd-text-cell>
      <nldd-text-cell v-for="c in columns" :key="c" :text="valueText(r.fields[c])"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
</template>

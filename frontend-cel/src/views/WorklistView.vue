<script setup>
// A list of cases: the worklist (the undecided cases) or all cases (also a
// decided one, for what follows the decision). A list lexostatus with a row
// per case; which cases are in it and with which columns, the lexostatus
// says, not the code.
import { computed, inject, onMounted, ref } from 'vue';
import { valueText } from '../text.js';

const props = defineProps({
  // 'worklist' or 'cases': the route of the process that gives the list.
  source: { type: String, default: 'worklist' },
  title: { type: String, default: 'Werkvoorraad' },
  subtitle: { type: String, default: 'De zaken waarop nog niet is besloten' },
  empty: { type: String, default: 'Geen zaken in de werkvoorraad' },
  columns: { type: Array, default: () => [] },
});
const emit = defineEmits(['open']);
const api = inject('api');

const list = ref([]);
const error = ref('');
const loaded = ref(false);

onMounted(async () => {
  try {
    list.value = (await api[props.source]()).list ?? [];
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
    <h1>{{ title }}</h1>
    <span slot="subtitle">{{ subtitle }}</span>
  </nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="error">
    <nldd-inline-dialog variant="alert" :text="`${title}: niet te laden`" :supporting-text="error"></nldd-inline-dialog>
  </template>
  <nldd-table v-else-if="loaded" :columns="template" :accessible-label="title" :empty-text="empty">
    <nldd-table-row slot="header">
      <nldd-text-cell text=""></nldd-text-cell>
      <nldd-text-cell text="Zaak (gram-id)"></nldd-text-cell>
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

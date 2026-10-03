<script setup>
// A cell: its chronicle and its lexostatuses. A cell knows no roles and no
// portal; whoever acts is a process. Its read routes are not open (the grams
// carry the identity of whoever submitted): someone logged in as a handler
// in a process that reads the cell sees it through that process's
// inspection. Otherwise the screen says in which process that is possible.
import { onMounted, provide, ref } from 'vue';
import { inspectionApi, processApi } from '../api.js';
import { processLabel } from '../text.js';
import ChronicleView from './ChronicleView.vue';
import LexostatusView from './LexostatusView.vue';

const props = defineProps({
  cell: { type: Object, required: true },
  // The processes that read this cell (`inspection` in GET /api/processes).
  processes: { type: Array, default: () => [] },
});
const emit = defineEmits(['open']);

// The process through which the user inspects the cell, or null.
const via = ref(null);
const loaded = ref(false);
provide('cellApi', {
  chronicle: () => inspectionApi(via.value, props.cell.id).chronicle(),
  lexostatus: (name, input) => inspectionApi(via.value, props.cell.id).lexostatus(name, input),
});
const screen = ref('chronicle');

onMounted(async () => {
  for (const p of props.processes) {
    const s = await processApi(p.id)
      .session()
      .catch(() => null);
    if (s && p.roles?.[s.role]?.routes?.includes('handling')) {
      via.value = p.id;
      break;
    }
  }
  loaded.value = true;
});

function tab(e) {
  const to = e.detail?.item?.dataset?.screen;
  if (to) screen.value = to;
}
</script>

<template>
  <template v-if="loaded && via">
    <nldd-tab-bar size="md" accessible-label="Scherm" @tabchange="tab">
      <nldd-tab-bar-item data-screen="chronicle" text="Kroniek" :current="screen === 'chronicle' || undefined"></nldd-tab-bar-item>
      <nldd-tab-bar-item
        v-if="cell.lexostatuses.length"
        data-screen="lexostatus"
        text="Lexostatus"
        :current="screen === 'lexostatus' || undefined"
      ></nldd-tab-bar-item>
    </nldd-tab-bar>
    <nldd-spacer size="24"></nldd-spacer>
    <ChronicleView v-if="screen === 'chronicle'" />
    <LexostatusView v-else :lexostatuses="cell.lexostatuses" />
  </template>
  <template v-else-if="loaded">
    <nldd-inline-dialog
      text="Inzage via een proces"
      :supporting-text="
        processes.length
          ? 'De kroniek en de lexostatussen van een cel zijn niet open. Log in als behandelaar in een proces dat deze cel leest.'
          : 'De kroniek en de lexostatussen van een cel zijn niet open, en geen proces in deze runtime leest deze cel.'
      "
    ></nldd-inline-dialog>
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-button-group v-if="processes.length" orientation="horizontal">
      <nldd-button v-for="p in processes" :key="p.id" variant="secondary" :text="`Open ${processLabel(p)}`" @click="emit('open', p.id)"></nldd-button>
    </nldd-button-group>
  </template>
</template>

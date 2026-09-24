<script setup>
// Een cel: haar kroniek en haar lexostatussen, zonder inloggen. Een cel kent
// geen rollen en geen portaal; wie er handelt, is een proces.
import { provide, ref } from 'vue';
import { celApi } from '../api.js';
import KroniekView from './KroniekView.vue';
import LexostatusView from './LexostatusView.vue';

const props = defineProps({ cel: { type: Object, required: true } });

provide('celApi', celApi(props.cel.id));
const scherm = ref('kroniek');

function tab(e) {
  const naar = e.detail?.item?.dataset?.scherm;
  if (naar) scherm.value = naar;
}
</script>

<template>
  <nldd-tab-bar size="md" accessible-label="Scherm" @tabchange="tab">
    <nldd-tab-bar-item data-scherm="kroniek" text="Kroniek" :current="scherm === 'kroniek' || undefined"></nldd-tab-bar-item>
    <nldd-tab-bar-item
      v-if="cel.lexostatussen.length"
      data-scherm="lexostatus"
      text="Lexostatus"
      :current="scherm === 'lexostatus' || undefined"
    ></nldd-tab-bar-item>
  </nldd-tab-bar>
  <nldd-spacer size="24"></nldd-spacer>
  <KroniekView v-if="scherm === 'kroniek'" />
  <LexostatusView v-else :lexostatussen="cel.lexostatussen" />
</template>

<script setup>
// Een cel: haar kroniek en haar lexostatussen. Een cel kent geen rollen en
// geen portaal; wie er handelt, is een proces. Haar leesroutes zijn niet
// open (de grammen dragen de identiteit van wie indiende): wie ingelogd is
// als behandelaar in een proces dat de cel leest, ziet haar via de inzage
// van dat proces. Anders zegt het scherm in welk proces dat kan.
import { onMounted, provide, ref } from 'vue';
import { inzageApi, procesApi } from '../api.js';
import KroniekView from './KroniekView.vue';
import LexostatusView from './LexostatusView.vue';

const props = defineProps({
  cel: { type: Object, required: true },
  // De processen die deze cel lezen (`inzage` in GET /api/processen).
  processen: { type: Array, default: () => [] },
});
const emit = defineEmits(['open']);

// Het proces waarlangs de gebruiker de cel inziet, of null.
const via = ref(null);
const geladen = ref(false);
provide('celApi', {
  kroniek: () => inzageApi(via.value, props.cel.id).kroniek(),
  lexostatus: (naam, invoer) => inzageApi(via.value, props.cel.id).lexostatus(naam, invoer),
});
const scherm = ref('kroniek');

onMounted(async () => {
  for (const p of props.processen) {
    const s = await procesApi(p.id)
      .sessie()
      .catch(() => null);
    if (s && p.rollen?.[s.rol]?.routes?.includes('behandeling')) {
      via.value = p.id;
      break;
    }
  }
  geladen.value = true;
});

function tab(e) {
  const naar = e.detail?.item?.dataset?.scherm;
  if (naar) scherm.value = naar;
}
</script>

<template>
  <template v-if="geladen && via">
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
  <template v-else-if="geladen">
    <nldd-inline-dialog
      text="Inzage via een proces"
      :supporting-text="
        processen.length
          ? 'De kroniek en de lexostatussen van een cel zijn niet open. Log in als behandelaar in een proces dat deze cel leest.'
          : 'De kroniek en de lexostatussen van een cel zijn niet open, en geen proces in deze runtime leest deze cel.'
      "
    ></nldd-inline-dialog>
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-button-group v-if="processen.length" orientation="horizontal">
      <nldd-button v-for="p in processen" :key="p.id" variant="secondary" :text="`Open ${p.id}`" @click="emit('open', p.id)"></nldd-button>
    </nldd-button-group>
  </template>
</template>

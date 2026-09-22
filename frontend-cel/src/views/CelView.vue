<script setup>
// Een cel. Met een portaal: inloggen, indienen, en de eigen kroniek en
// lexostatus van de ingelogde KvK. Zonder portaal: de kroniek en de
// lexostatussen, zonder inloggen.
import { onMounted, provide, ref } from 'vue';
import { celApi } from '../api.js';
import InloggenView from './InloggenView.vue';
import AanvraagView from './AanvraagView.vue';
import KroniekView from './KroniekView.vue';
import LexostatusView from './LexostatusView.vue';

const props = defineProps({ cel: { type: Object, required: true } });

const api = celApi(props.cel.id);
provide('api', api);

const sessie = ref(null);
const geladen = ref(!props.cel.portaal);
const scherm = ref(props.cel.portaal ? 'aanvraag' : 'kroniek');
const nieuw = ref(null);

onMounted(async () => {
  if (!props.cel.portaal) return;
  try {
    sessie.value = await api.sessie();
  } catch {
    sessie.value = null;
  } finally {
    geladen.value = true;
  }
});

function ingediend(gram) {
  nieuw.value = gram;
  scherm.value = 'kroniek';
}

async function uitloggen() {
  await api.uitloggen().catch(() => {});
  sessie.value = null;
}

function tab(e) {
  const naar = e.detail?.item?.dataset?.scherm;
  if (naar) scherm.value = naar;
}
</script>

<template>
  <template v-if="cel.portaal && geladen && !sessie">
    <InloggenView @ingelogd="sessie = $event" />
  </template>
  <template v-else-if="geladen">
    <nldd-container layout="row" horizontal-alignment="space-between" vertical-alignment="center">
      <nldd-tab-bar size="md" accessible-label="Scherm" @tabchange="tab">
        <nldd-tab-bar-item v-if="cel.portaal" data-scherm="aanvraag" text="Indienen" :current="scherm === 'aanvraag' || undefined"></nldd-tab-bar-item>
        <nldd-tab-bar-item data-scherm="kroniek" text="Kroniek" :current="scherm === 'kroniek' || undefined"></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="cel.lexostatussen.length"
          data-scherm="lexostatus"
          text="Lexostatus"
          :current="scherm === 'lexostatus' || undefined"
        ></nldd-tab-bar-item>
      </nldd-tab-bar>
      <nldd-button
        v-if="sessie"
        variant="neutral-transparent"
        start-icon="logout"
        :text="`Uitloggen (${sessie.persoon}, KvK ${sessie.kvk})`"
        @click="uitloggen"
      ></nldd-button>
    </nldd-container>
    <nldd-spacer size="24"></nldd-spacer>
    <AanvraagView v-if="scherm === 'aanvraag'" @ingediend="ingediend" />
    <KroniekView v-else-if="scherm === 'kroniek'" :nieuw="nieuw" :portaal="cel.portaal" />
    <LexostatusView v-else :lexostatussen="cel.lexostatussen" />
  </template>
</template>

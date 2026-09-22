<script setup>
// Drie schermen: inloggen, indienen en de kroniek. De frontend kent geen
// casus; wat er te vragen valt, komt van de cel.
import { onMounted, ref } from 'vue';
import { api } from './api.js';
import InloggenView from './views/InloggenView.vue';
import AanvraagView from './views/AanvraagView.vue';
import KroniekView from './views/KroniekView.vue';

const sessie = ref(null);
const geladen = ref(false);
const scherm = ref('aanvraag');
const nieuw = ref(null);

onMounted(async () => {
  try {
    sessie.value = await api.sessie();
  } catch {
    sessie.value = null;
  } finally {
    geladen.value = true;
  }
});

function ingediend(gram) {
  nieuw.value = gram.zaakkenmerk;
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
  <nldd-page>
    <nldd-top-navigation-bar slot="header" no-logo website-title="Aanvraag-cel"></nldd-top-navigation-bar>
    <nldd-simple-section>
      <template v-if="geladen && !sessie">
        <InloggenView @ingelogd="sessie = $event" />
      </template>
      <template v-else-if="sessie">
        <nldd-container layout="row" horizontal-alignment="space-between" vertical-alignment="center">
          <nldd-tab-bar size="md" accessible-label="Scherm" @tabchange="tab">
            <nldd-tab-bar-item data-scherm="aanvraag" text="Indienen" :current="scherm === 'aanvraag' || undefined"></nldd-tab-bar-item>
            <nldd-tab-bar-item data-scherm="kroniek" text="Kroniek" :current="scherm === 'kroniek' || undefined"></nldd-tab-bar-item>
          </nldd-tab-bar>
          <nldd-button
            variant="neutral-transparent"
            start-icon="logout"
            :text="`Uitloggen (${sessie.persoon}, KvK ${sessie.kvk})`"
            @click="uitloggen"
          ></nldd-button>
        </nldd-container>
        <nldd-spacer size="24"></nldd-spacer>
        <AanvraagView v-if="scherm === 'aanvraag'" @ingediend="ingediend" />
        <KroniekView v-else :nieuw="nieuw" />
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>

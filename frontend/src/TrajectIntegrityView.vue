<script setup>
// Integriteitspagina van een traject: `/trajecten/{ref}/integriteit`.
//
// Top-level route (geen AppShell-child), net als de trajectkeuze en het
// aanmaakformulier: de pagina draagt haar eigen top-title-bar met een terugknop
// naar Instellingen, waar de link vandaan komt. Geen app-chrome, want dit is
// een diagnose die je erbij pakt - geen sectie waar je in werkt.
//
// Het rapport zelf zit in TrajectIntegrityPane, zodat die los te testen is en
// later ook in een paneel kan hangen.
import { computed, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import TrajectIntegrityPane from './components/TrajectIntegrityPane.vue';

const route = useRoute();
const router = useRouter();

const trajectRef = computed(() => route.params.trajectRef);

watchEffect(() => {
  document.title = 'Integriteit · RegelRecht';
});

function goBack() {
  router.push({
    name: 'instellingen-traject',
    params: { trajectRef: trajectRef.value, tab: 'details' },
  });
}
</script>

<template>
  <nldd-app-view>
    <nldd-page sticky-header>
      <nldd-top-title-bar
        slot="header"
        text="Integriteit"
        back-text="Instellingen"
        collapse-anchor="integriteit-pane-titel"
        @back="goBack"
      ></nldd-top-title-bar>

      <TrajectIntegrityPane :traject-ref="trajectRef"></TrajectIntegrityPane>
    </nldd-page>
  </nldd-app-view>
</template>

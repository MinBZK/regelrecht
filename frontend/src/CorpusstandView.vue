<script setup>
import { computed, onMounted, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useCorpusstand } from './composables/useCorpusstand.js';
import CorpusstandReport from './components/CorpusstandReport.vue';

// Top-level pagina met eigen chrome, naar het model van Corpusinwinning en de
// trajectkiezer. Bewust géén modus van LibraryView: als deze pagina in
// LibraryView's `main` zou renderen, moesten daar de leeg/laden-guards, de
// titelopbouw en de terugknop alle drie leren dat er een vierde sectie is.
// Zo blijft de wijziging aan bestaande bestanden één regel per bestand.
//
// Prijs daarvan: de traject-sidebar (Instellingen / Werkdocumenten / Taken)
// staat hier niet. De terugknop brengt je terug naar het traject.

const route = useRoute();
const router = useRouter();

const trajectRef = computed(() => route.params.trajectRef || null);

const { rapport, loading, error, overgeslagen, laad, displayName } = useCorpusstand(trajectRef);

onMounted(laad);
// Van traject wisselen via de URL herleest het corpus; zonder dit blijft het
// rapport van het vorige traject staan onder een nieuwe ref.
watch(trajectRef, laad);

function terug() {
  router.push({ name: 'traject-home', params: { trajectRef: trajectRef.value } });
}

// Elke rij linkt naar het artikel waar hij over gaat. Dat is de hele opzet:
// het dashboard is de inventaris, de editor de werkplek. Zonder artikelnummer
// openen we de wet en laat de editor de eerste bepaling zien.
function hrefVoor(lawId, artikel) {
  return router.resolve({
    name: 'editor-traject',
    params: { trajectRef: trajectRef.value, lawId, articleNumber: artikel || undefined },
  }).href;
}
</script>

<template>
  <nldd-app-view>
    <nldd-page sticky-header>
      <nldd-top-title-bar
        slot="header"
        text="Corpusstand"
        back-text="Traject"
        collapse-anchor="corpusstand-titel"
        @back="terug"
      ></nldd-top-title-bar>

      <nldd-simple-section width="800px">
        <nldd-title id="corpusstand-titel" size="3"><h3>Corpusstand</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>

        <nldd-activity-indicator
          v-if="loading"
          text="Notities uit het corpus lezen"
          show-text
        ></nldd-activity-indicator>

        <nldd-inline-dialog
          v-else-if="error"
          variant="alert"
          text="Corpusstand is niet geladen"
          :supporting-text="error.message"
        ></nldd-inline-dialog>

        <template v-else-if="overgeslagen.length">
          <nldd-banner
            variant="warning"
            :text="`${overgeslagen.length} wet(ten) overgeslagen`"
            :supporting-text="overgeslagen.join(' · ')"
          ></nldd-banner>
        </template>
      </nldd-simple-section>

      <CorpusstandReport
        v-if="!loading && !error"
        :rapport="rapport"
        :naam-voor="displayName"
        :href-voor="hrefVoor"
      />
    </nldd-page>
  </nldd-app-view>
</template>

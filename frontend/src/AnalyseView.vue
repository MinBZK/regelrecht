<script setup>
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useAnalyse } from './composables/useAnalyse.js';
import { graafUitMetrieken } from './lib/graafUitMetrieken.js';
import MetriekenRij from './components/MetriekenRij.vue';
import CorpusGraafView from './components/CorpusGraafView.vue';
import ModelDefecten from './components/ModelDefecten.vue';
import NotenRapport from './components/NotenRapport.vue';

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

const {
  rapport,
  metrieken,
  geweigerd,
  peildatum,
  zetPeildatum,
  loading,
  error,
  overgeslagen,
  laad,
  displayName,
} = useAnalyse(trajectRef);

// Wetten die op de peildatum geen geldende versie hebben. Apart getoond, want
// een wet die niet meetelt is iets anders dan een wet die in orde is.
const nietGeldend = computed(() => metrieken.value?.not_in_force ?? []);

// De graaf komt uit hetzelfde rapport als de tegels, niet uit een eigen lezing
// van de YAML. Daardoor kan hij niet iets anders beweren dan de cijfers erboven.
const graaf = computed(() => (metrieken.value ? graafUitMetrieken(metrieken.value) : null));
const gekozenKnoop = ref(null);

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
        text="Analyse"
        back-text="Traject"
        collapse-anchor="analyse-titel"
        @back="terug"
      ></nldd-top-title-bar>

      <nldd-simple-section width="800px">
        <nldd-title id="analyse-titel" size="3"><h3>Analyse</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>

        <nldd-activity-indicator
          v-if="loading"
          text="Notities lezen"
          show-text
        ></nldd-activity-indicator>

        <nldd-inline-dialog
          v-else-if="error"
          variant="alert"
          text="De analyse kon niet worden opgehaald"
          :supporting-text="error.message"
        ></nldd-inline-dialog>

        <template v-else-if="overgeslagen.length">
          <nldd-banner
            variant="warning"
            :text="overgeslagen.length === 1 ? 'Eén wet overgeslagen' : `${overgeslagen.length} wetten overgeslagen`"
            :supporting-text="overgeslagen.join(' · ')"
          ></nldd-banner>
        </template>
      </nldd-simple-section>

      <nldd-simple-section v-if="!loading && !error && metrieken" width="800px">
        <nldd-form-field label="Peildatum" for="analyse-peildatum">
          <nldd-date-field
            id="analyse-peildatum"
            size="sm"
            :value="peildatum"
            @change="zetPeildatum($event.detail.value || peildatum)"
          ></nldd-date-field>
          <span slot="help-text">
            Bepaalt welke versie van elke regeling meetelt. De cijfers hieronder gelden op deze datum.
          </span>
        </nldd-form-field>

        <template v-if="nietGeldend.length">
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-inline-dialog
            data-test="niet-geldend"
            :text="nietGeldend.length === 1 ? 'Eén regeling geldt niet op deze datum' : `${nietGeldend.length} regelingen gelden niet op deze datum`"
            :supporting-text="nietGeldend.map((r) => `${displayName(r.law_id)}: ${r.reason === 'not-yet-in-force' ? 'nog niet in werking' : r.ended_on ? `vervallen per ${r.ended_on}` : 'niet gevonden'}`).join(' · ')"
          ></nldd-inline-dialog>
        </template>
        <nldd-spacer size="16"></nldd-spacer>
      </nldd-simple-section>

      <MetriekenRij v-if="!loading && !error && metrieken" :rapport="metrieken" :geweigerd="geweigerd" />

      <nldd-simple-section v-if="!loading && !error && graaf && graaf.knopen.length" width="1100px">
        <nldd-title size="6"><h3>Afhankelijkheidsgraaf</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-inline-dialog
          :text="`${graaf.knopen.length} regelingen, ${graaf.randen.filter((r) => r.van !== r.naar).length} bindingen`"
          supporting-text="Doorgetrokken pijl is een waarde die de engine ophaalt. Stippel is een open term die door een lagere regeling wordt ingevuld."
        ></nldd-inline-dialog>
        <nldd-spacer size="12"></nldd-spacer>
        <CorpusGraafView :graaf="graaf" :naam-voor="displayName" @knoop="gekozenKnoop = $event" />
        <template v-if="gekozenKnoop">
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-inline-dialog
            :text="displayName(gekozenKnoop.lawId)"
            :supporting-text="`${gekozenKnoop.laag ?? 'laag onbekend'} · ${gekozenKnoop.artikelen} artikelen · ${gekozenKnoop.outputs} outputs · ${gekozenKnoop.inkomend} inkomende binding(en)`"
          ></nldd-inline-dialog>
        </template>
        <nldd-spacer size="24"></nldd-spacer>
      </nldd-simple-section>

      <ModelDefecten
        v-if="metrieken"
        :defecten="metrieken.findings"
        :naam-voor="displayName"
        :href-voor="hrefVoor"
      />

      <NotenRapport
        v-if="!loading && !error"
        :rapport="rapport"
        :naam-voor="displayName"
        :href-voor="hrefVoor"
      />
    </nldd-page>
  </nldd-app-view>
</template>

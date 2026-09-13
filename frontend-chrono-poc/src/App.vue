<script setup>
import { computed, onMounted, ref } from 'vue';
import ActionPanel from './components/ActionPanel.vue';
import CellColumn from './components/CellColumn.vue';
import GramPanel from './components/GramPanel.vue';
import LexostatusPanel from './components/LexostatusPanel.vue';
import ObservationLog from './components/ObservationLog.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import WorldTimeline from './components/WorldTimeline.vue';
import { formatMoment } from './world/format.js';
import { cells, missedDeadlines } from './world/snapshot.js';
import { useWorld } from './world/useWorld.js';

// De testopstelling in één pagina: bovenaan de bediening over de volle breedte
// (acties, instellingen, een vraag aan een cel, alle grammen, en het
// observatielog als meetinstrument), daaronder een kolom per cel met haar
// kronieken, en onderaan de tijdlijn met de klok.
//
// Boven en onder, niet links en rechts: een cel is breed — een kroniekrij draagt
// een naam, een moment, een kanaal en een grondslag — en in een halve pagina
// paste geen enkele kolom nog heel. De bediening is smal van zichzelf, dus die
// kan de volle breedte hebben zonder er iets mee te doen, en de cellen krijgen
// de hele breedte om in te staan.
//
// Deze app kent geen casus. Elke naam die hier op het scherm komt — een cel, een
// kroniek, een actie, een instelling — staat in het beeld dat de server geeft.
// Een andere wet met andere organisaties is een ander wereldbestand en dezelfde
// frontend.

// De refs los uit de store, zodat de template ze uitgepakt ziet.
const {
  snapshot,
  previousCounts,
  loading,
  busy,
  error,
  actionError,
  result,
  ready,
  clock,
  load,
  act,
  advance,
  saveSettings,
  reset,
  askLexostatus,
  dismissResult,
  dismissError,
} = useWorld();

const columns = computed(() => cells(snapshot.value));
const deadlines = computed(() => missedDeadlines(snapshot.value));

/**
 * De panelen van de bediening bovenaan.
 *
 * De eerste drie doen iets met de wereld, de laatste twee kijken er alleen naar:
 * "Grammen" legt alles wat er ligt naast elkaar, en het observatielog is een
 * meetinstrument en zegt dat zelf ook.
 */
const tabs = [
  { key: 'acties', text: 'Acties', icon: 'hand' },
  { key: 'instellingen', text: 'Instellingen', icon: 'settings' },
  { key: 'lexostatus', text: 'Lexostatus', icon: 'radar' },
  { key: 'grammen', text: 'Grammen', icon: 'table' },
  { key: 'log', text: 'Observatielog', icon: 'binoculars' },
];
const tab = ref('acties');

function onTabChange(event) {
  const key = event?.detail?.item?.dataset?.tabKey;
  if (key) tab.value = key;
}

onMounted(() => load());

/** Wat de laatste stap opleverde, in de woorden van de wereld. */
const resultText = computed(() => {
  const grams = result.value?.grams ?? [];
  if (grams.length === 0) return 'Er kwam geen gram bij.';
  return grams
    .map((gram) => `${gram.name} in kroniek '${gram.stream}' van cel '${gram.cell}' (${formatMoment(gram.opMoment)})`)
    .join('; ');
});

function runAction({ action, values }) {
  act(action, values);
}

/**
 * Staat de fout bovenaan de pagina, of bij het formulier dat hem uitlokte?
 *
 * Bij de actie zelf als het er een van een actie is én dat paneel op het scherm
 * staat. Dat tweede is geen detail: de panelen wisselen elkaar af, dus wie na
 * een mislukte actie naar een ander tabblad gaat, zou de melding anders nergens
 * meer zien staan — niet in de kaart, want die is weg, en niet bovenaan, want
 * die zweeg voor de kaart.
 */
const showBanner = computed(() => Boolean(error.value) && !(actionError.value && tab.value === 'acties'));
</script>

<template>
  <nldd-app-view background="tinted">
    <nldd-bar-split-view>
      <!-- De skip-link omsluit de werkbalk: zijn knop staat ervóór en zet de
           focus op het eerstvolgende element, de inhoud zelf. -->
      <nldd-skip-link slot="toolbar" text="Direct naar de inhoud">
        <!-- Alleen padding: nldd-container kent geen achtergrond, de bar leest de
             zijne uit nldd-app-view. Zelfde vorm als de bars in frontend/. -->
        <nldd-container padding="8">
          <nldd-toolbar size="md" label="Testopstelling">
            <nldd-toolbar-item slot="start">
              <nldd-title size="6">
                <span slot="overline">Chronolexografie</span>
                <span>Testopstelling</span>
              </nldd-title>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-tag color="accent" icon="clock" :text="`Klok: ${formatMoment(clock)}`"></nldd-tag>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button
                variant="neutral-tinted"
                start-icon="refresh"
                text="Beeld verversen"
                :loading="loading || undefined"
                @click="load()"
              ></nldd-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-skip-link>

      <nldd-split-view-pane slot="main" has-content>
        <nldd-stacked-split-view panes="2">
          <!-- Boven de bediening, over de volle breedte. Meldingen over de
               wereld staan hier: dit is de bovenkant van de pagina, en een
               melding die je pas ziet na scrollen is geen melding. -->
          <nldd-split-view-pane slot="pane-1" has-content background="tinted">
            <nldd-page>
              <nldd-simple-section width="full">
                <nldd-container layout="stack" gap="16">
                  <!-- Een fout op een actie staat bij haar eigen formulier (zie
                       ActionCard); die hier nog eens herhalen zou dezelfde zin
                       twee keer op één scherm zetten. Staat dat formulier niet
                       op het scherm, dan staat hij hier alsnog: zie
                       `showBanner`. -->
                  <nldd-banner
                    v-if="showBanner"
                    variant="critical"
                    text="De server kon dit niet doen"
                    :supporting-text="error"
                    dismissible
                    @dismiss="dismissError()"
                  ></nldd-banner>

                  <nldd-banner
                    v-if="result"
                    variant="success"
                    :text="result.label"
                    :supporting-text="resultText"
                    dismissible
                    @dismiss="dismissResult()"
                  ></nldd-banner>

                  <nldd-tab-bar accessible-label="Bediening van de wereld" @tabchange="onTabChange">
                    <nldd-tab-bar-item
                      v-for="item in tabs"
                      :key="item.key"
                      :data-tab-key="item.key"
                      :text="item.text"
                      :selected="tab === item.key || undefined"
                    >
                      <nldd-icon slot="icon" :name="item.icon"></nldd-icon>
                    </nldd-tab-bar-item>
                  </nldd-tab-bar>

                  <ActionPanel
                    v-if="tab === 'acties'"
                    :snapshot="snapshot"
                    :busy="busy"
                    :error="actionError"
                    @run="runAction"
                  />
                  <SettingsPanel
                    v-else-if="tab === 'instellingen'"
                    :snapshot="snapshot"
                    :busy="busy"
                    @save="saveSettings($event)"
                    @reset="reset()"
                  />
                  <LexostatusPanel
                    v-else-if="tab === 'lexostatus'"
                    :snapshot="snapshot"
                    :ask="askLexostatus"
                  />
                  <GramPanel v-else-if="tab === 'grammen'" :snapshot="snapshot" />
                  <ObservationLog v-else :snapshot="snapshot" />
                </nldd-container>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Daaronder de wereld zelf: een kolom per cel, over de volle
               breedte, en die kolommen schuiven binnen hun eigen paneel opzij
               als er meer cellen zijn dan er naast elkaar passen. -->
          <nldd-split-view-pane slot="pane-2" has-content>
            <nldd-page>
              <nldd-simple-section width="full">
                <nldd-container layout="stack" gap="16">
                  <template v-if="deadlines.length > 0">
                    <nldd-title size="6">
                      <span>Verstreken termijnen</span>
                      <span slot="subtitle">de wet zegt wat de termijn was; besluiten kan nog</span>
                    </nldd-title>
                    <nldd-list variant="box-tinted" accessible-label="Verstreken termijnen">
                      <nldd-list-item v-for="(deadline, index) in deadlines" :key="index" size="sm">
                        <nldd-icon-cell icon="warning" size="16" color="warning"></nldd-icon-cell>
                        <nldd-spacer-cell size="8"></nldd-spacer-cell>
                        <nldd-text-cell
                          size="sm"
                          :text="deadline.label"
                          :supporting-text="`${formatMoment(deadline.at)}: cel '${deadline.cell}' had geen '${deadline.name}' in kroniek '${deadline.chronicle}'`"
                        ></nldd-text-cell>
                      </nldd-list-item>
                    </nldd-list>
                  </template>

                  <nldd-activity-indicator
                    v-if="loading && !ready"
                    show-text
                    text="Beeld van de wereld ophalen…"
                    timing="instant"
                    size="48"
                  ></nldd-activity-indicator>

                  <nldd-inline-dialog
                    v-else-if="ready && columns.length === 0"
                    icon="database"
                    text="Geen cellen"
                    supporting-text="Dit wereldbestand beschrijft geen cellen."
                  ></nldd-inline-dialog>

                  <nldd-collection v-else layout="horizontal-scroll" item-width="420px">
                    <CellColumn
                      v-for="cell in columns"
                      :key="cell.id"
                      :cell="cell"
                      :clock="clock"
                      :previous-counts="previousCounts"
                    />
                  </nldd-collection>
                </nldd-container>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>
        </nldd-stacked-split-view>
      </nldd-split-view-pane>

      <nldd-split-view-pane slot="timeline-bar">
        <WorldTimeline
          :snapshot="snapshot"
          :previous-counts="previousCounts"
          :busy="busy"
          @advance="advance($event)"
        />
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-app-view>
</template>

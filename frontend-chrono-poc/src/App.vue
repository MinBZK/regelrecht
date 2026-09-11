<script setup>
import { computed, onMounted, ref } from 'vue';
import ActionPanel from './components/ActionPanel.vue';
import CellColumn from './components/CellColumn.vue';
import LexostatusPanel from './components/LexostatusPanel.vue';
import ObservationLog from './components/ObservationLog.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import WorldTimeline from './components/WorldTimeline.vue';
import { formatMoment } from './world/format.js';
import { cells, warnings } from './world/snapshot.js';
import { useWorld } from './world/useWorld.js';

// De testopstelling in één pagina: links een kolom per cel met haar kronieken,
// rechts de bediening (acties, instellingen, een vraag aan een cel, en het
// observatielog als meetinstrument), onderaan de tijdlijn met de klok.
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
const deadlines = computed(() => warnings(snapshot.value));

/** De panelen rechts. De vierde is een meetinstrument en zegt dat zelf ook. */
const tabs = [
  { key: 'acties', text: 'Acties', icon: 'hand' },
  { key: 'instellingen', text: 'Instellingen', icon: 'settings' },
  { key: 'lexostatus', text: 'Lexostatus', icon: 'radar' },
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
        <nldd-side-by-side-split-view panes="2">
          <!-- Links de wereld zelf: de kolommen zijn waar het om gaat, dus ze
               staan vooraan en verdwijnen als laatste op een smal scherm. -->
          <nldd-split-view-pane slot="pane-1" has-content>
            <nldd-page>
              <nldd-simple-section width="full">
                <nldd-container layout="stack" gap="16">
                  <nldd-banner
                    v-if="error"
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

          <!-- Rechts de bediening. Secundair, dus dit paneel gaat op een smal
               scherm als eerste weg. -->
          <nldd-split-view-pane slot="pane-2" has-content background="tinted">
            <nldd-page>
              <nldd-simple-section width="full">
                <nldd-container layout="stack" gap="16">
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
                  <ObservationLog v-else :snapshot="snapshot" />
                </nldd-container>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>
        </nldd-side-by-side-split-view>
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

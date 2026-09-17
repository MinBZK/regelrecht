<script setup>
import { computed, onMounted } from 'vue';
import InzichtView from './views/InzichtView.vue';
import PortaalView from './views/PortaalView.vue';
import WereldView from './views/WereldView.vue';
import { formatMoment } from './world/format.js';
import { currentPage, pages, useHash } from './world/route.js';
import { useWorld } from './world/useWorld.js';

// De schil om de pagina's: de werkbalk met de klok, en daaronder de pagina die
// het adres aanwijst.
//
// Zonder portaal in het wereldbestand is er één pagina, de wereld zelf, en staat
// er geen navigatie. Met een portaal komen er twee pagina's voor een aanvrager
// bij — het aanvraagportaal en inzicht in je aanvraag — en heet de wereld
// "Achter de schermen". Wat een aanvrager ziet, is wat zij kan doen en wat de
// cellen over haar publiceren; het totaalbeeld staat op die derde pagina, en
// daar hoort het ook: het bestaat nergens anders dan in deze opstelling.
//
// Eén store voor alle pagina's (`useWorld`): wie op de ene pagina een aanvrager
// kiest of een aanvraag indient, ziet dat op de andere terug.

const { loading, clock, portaal, load, loadPortaal } = useWorld();

const hash = useHash();

/** De pagina's van deze wereld; leeg zonder portaal. */
const navigation = computed(() => pages(portaal.value));

/**
 * De pagina die open staat.
 *
 * Pas bekend als het portaal opgehaald is: tot dan is niet te zeggen of er naast
 * de wereld nog pagina's zijn, en een adres als `#/inzicht` zou anders eerst de
 * wereld tonen en dan wegspringen.
 */
const page = computed(() => (portaal.value === undefined ? null : currentPage(hash.value, portaal.value)));

onMounted(() => {
  load();
  loadPortaal();
});
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
            <!-- De navigatie tussen de pagina's, zoals frontend-demo die doet:
                 één tab-bar als `navigation`, met een echte link per pagina.
                 De browser volgt de hash zelf; de app leest alleen waar hij
                 staat. Alleen als er iets te kiezen valt. -->
            <nldd-toolbar-item v-if="navigation.length > 0" slot="start" :priority="90">
              <nldd-tab-bar navigation accessible-label="Pagina">
                <nldd-tab-bar-item
                  v-for="item in navigation"
                  :key="item.key"
                  :text="item.text"
                  :href="item.href"
                  :current="page === item.key || undefined"
                >
                  <nldd-icon slot="icon" :name="item.icon"></nldd-icon>
                </nldd-tab-bar-item>
              </nldd-tab-bar>
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

      <PortaalView v-if="page === 'portaal'" />
      <InzichtView v-else-if="page === 'inzicht'" />
      <WereldView v-else-if="page === 'wereld'" />
      <nldd-split-view-pane v-else slot="main" has-content>
        <nldd-page>
          <nldd-simple-section>
            <nldd-activity-indicator
              show-text
              text="Beeld van de wereld ophalen…"
              timing="instant"
              size="48"
            ></nldd-activity-indicator>
          </nldd-simple-section>
        </nldd-page>
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-app-view>
</template>

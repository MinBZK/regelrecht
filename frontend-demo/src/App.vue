<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useColorScheme } from '@regelrecht/frontend-shared';
import { FEATURES, useDemo } from './store/demoStore.js';
import { delegationLabel } from './data/delegation.js';
import PresentationDeck from './presentation/PresentationDeck.vue';
import { usePresentation } from './presentation/usePresentation.js';

// The workspace shell: one bar with the tab bar and the presenter menu, and
// the active tab below it. Every tab is a route; <keep-alive> keeps the tabs
// the presenter already visited mounted, so opened law tabs, a running
// scenario or an expanded tile survive switching back and forth.

const route = useRoute();
const router = useRouter();
const demo = useDemo();
const { ready, loadError, profile, profileKey, corpus, state, delegations, delegationEnabled, activeDelegation, features } = demo;

/** Heeft de presentator een vlag omgezet? Dan kan hij terug naar het profiel. */
const hasFeatureOverrides = computed(() => Object.keys(state.featureOverrides ?? {}).length > 0);
/** Hoeveel er aanstaan, zodat je het ziet zonder de uitklapper te openen. */
const featureSummary = computed(() => {
  const aan = FEATURES.filter((f) => features.value[f.key]).length;
  return `${aan} van ${FEATURES.length} aan`;
});

// The design system derives its scroll mode (document vs. per-pane) from the
// outermost split view once, at connect. Ours arrives later (the tab views are
// lazy routes), so the app-view settles on "document scrolls" and the pane
// headers stop sticking. Re-deriving after every route change puts it right;
// it is a design-system timing gap, not something the demo should own.
const appView = ref(null);
function refreshScrollMode(attempt = 0) {
  nextTick(() => {
    const view = appView.value;
    view?._evaluateScrollMode?.();
    // The split view measures itself a frame or two after it upgrades; retry
    // until the derived mode is in, then stop.
    if (view && view._derivedMode !== 'nested' && attempt < 6) setTimeout(() => refreshScrollMode(attempt + 1), 100 * (attempt + 1));
  });
}
onMounted(() => {
  demo.boot().catch(() => {});
  refreshScrollMode();
});
router.afterEach(refreshScrollMode);

// The presentation deck drives the tabs; it needs the router, the store (to
// switch persona) and the slides from the demo config once that has loaded.
const presentation = usePresentation();
presentation.init({ router, demo });
watch(corpus, (c) => presentation.init({ slides: c?.config?.slides ?? [] }), { immediate: true });
function onGlobalKey(e) {
  if (e.key === 'P' && e.shiftKey && !e.target?.closest?.('input, textarea, select, [contenteditable]')) {
    e.preventDefault();
    if (presentation.active.value) presentation.stop();
    else presentation.start(presentation.index.value);
  }
}
onMounted(() => window.addEventListener('keydown', onGlobalKey));
onUnmounted(() => window.removeEventListener('keydown', onGlobalKey));

const tabs = computed(() => [
  { name: 'presentatie', text: 'Presentatie', icon: 'display', to: '/' },
  { name: 'wetten', text: 'Wetten', icon: 'books', to: '/wetten' },
  { name: 'graaf', text: 'Graaf', icon: 'centralized-network', to: '/graaf' },
  { name: 'scenarios', text: "Scenario's", icon: 'checklist', to: '/scenarios' },
  { name: 'simulatie', text: 'Simulatie', icon: 'chart-x-y-axis-line', to: '/simulatie' },
  // Namens een onderneming heet het tabblad naar die onderneming: 'Mijn
  // overheid' gaat over de ingelogde burger, en dat is dan niet het onderwerp.
  {
    name: 'portaal',
    text: activeDelegation.value?.subjectType === 'BUSINESS' ? activeDelegation.value.subjectName : profile.value?.portal_tab_label ?? 'Mijn overheid',
    icon: activeDelegation.value?.subjectType === 'BUSINESS' ? 'building' : 'person',
    to: '/portaal',
  },
  { name: 'zaaksysteem', text: 'Zaaksysteem', icon: 'inbox', to: '/zaaksysteem' },
]);

function isActive(tab) {
  return route.name === tab.name;
}

// De tabbalk krimpt met het venster mee in plaats van tabbladen weg te laten
// vallen. Gemeten met zeven tabbladen: icoon met tekst 878px, alleen tekst
// 696px, alleen icoon 314px. Bij de drempels zit ruimte voor de knoppen rechts
// (namens wie, profiel) en de overloopknop.
const viewportWidth = ref(typeof window === 'undefined' ? 1600 : window.innerWidth);
function onResize() {
  viewportWidth.value = window.innerWidth;
}
onMounted(() => window.addEventListener('resize', onResize));
onUnmounted(() => window.removeEventListener('resize', onResize));

/**
 * De tabbladen in het vangnet-menu hebben een `href` zodat ze als echte link
 * renderen: dat zet `aria-current="page"` op het actieve tabblad en houdt
 * middenklik heel. Het menu-item roept er alleen geen preventDefault op, dus
 * de browser volgt die href en herlaadt de hele pagina naast de
 * router-navigatie (gemeten: vier framenavigaties).
 *
 * De luisteraar moet op het menu van de toolbar zelf: dat menu bevat klonen
 * van onze items en is naar document.body verplaatst, dus een handler op onze
 * eigen nldd-menu-group in de template ziet die klik nooit. Capture-fase, zodat
 * we er vóór het menu-item bij zijn. Een klik met een modifier laten we staan,
 * want dat is iemand die bewust een nieuw tabblad of venster wil.
 */
function onOverflowMenuClick(event) {
  if (event.defaultPrevented) return;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
  const item = event.composedPath?.().find((n) => n?.tagName?.toLowerCase?.() === 'nldd-menu-item');
  const to = item?.getAttribute?.('href');
  if (!to || !tabs.value.some((t) => t.to === to)) return;
  event.preventDefault();
  router.push(to);
}

// Het menu bestaat pas nadat de toolbar het heeft aangemaakt, en het verhuist
// naar document.body. Daarom luisteren we op document en filteren we op de
// href van een eigen tabblad.
onMounted(() => document.addEventListener('click', onOverflowMenuClick, true));
onUnmounted(() => document.removeEventListener('click', onOverflowMenuClick, true));

const tabVariant = computed(() => {
  if (viewportWidth.value >= 1240) return 'icon-and-text';
  if (viewportWidth.value >= 1040) return 'text';
  return 'icon';
});

const profileOptions = computed(() => Object.entries(corpus.value?.config?.profiles ?? {}));

function onProfileSelect(e) {
  const value = e.target?.getAttribute?.('value');
  if (value) demo.setProfile(value);
}

// ---- machtigingen ----------------------------------------------------------
// Namens wie er gehandeld wordt. De lijst komt uit de wet (elke wet die het
// machtigingscontract vervult), niet uit de app.

/** Toon de keuze pas als er echt iets te kiezen valt. */
const showDelegation = computed(() => delegationEnabled.value && delegations.value.length > 1);

/** Wat er in de knop staat: 'Mezelf' of degene namens wie gehandeld wordt. */
const delegationButtonText = computed(() => activeDelegation.value?.subjectName ?? 'Mezelf');

const DELEGATION_ICONS = { SELF: 'person', CITIZEN: 'person', BUSINESS: 'building' };

function onDelegationSelect(e) {
  const value = e.target?.getAttribute?.('value');
  if (!value) return;
  demo.setDelegation(delegations.value.find((d) => `${d.subjectType}:${d.subjectId}` === value) ?? null);
}

const { colorScheme, setColorScheme } = useColorScheme();
const colorSchemeOptions = [
  ['auto', 'Systeem', 'display'],
  ['light', 'Licht', 'light-mode'],
  ['dark', 'Donker', 'dark-mode'],
];
function onColorSchemeSelect(e) {
  const value = e.target?.getAttribute?.('value');
  if (value) setColorScheme(value);
}

function toggleFullscreen() {
  if (document.fullscreenElement) document.exitFullscreen?.();
  else document.documentElement.requestFullscreen?.();
}

const resetDialog = ref(null);
function askReset() {
  resetDialog.value?.show?.();
}
function confirmReset() {
  resetDialog.value?.hide?.();
  demo.resetState();
  router.push('/');
}

function toggleManualReview() {
  state.manualReview = !state.manualReview;
}

const openCases = computed(() => state.cases.filter((c) => c.status === 'IN_REVIEW').length);
</script>

<template>
  <nldd-app-view ref="appView" background="tinted">
    <PresentationDeck />
    <nldd-bar-split-view>
      <nldd-container slot="toolbar" padding="8" background="base">
        <nldd-toolbar size="md" label="Werkruimte">
          <!-- Eén tab-bar met alle tabbladen, niet één per tabblad. Een tab-bar
               per tabblad leek de overloop netjes op te lossen, maar elke bar
               rendert zijn eigen `<nav>`-landmark en regelt pijltjesnavigatie
               binnen zijn eigen items: zeven bars gaven zeven gelijknamige
               landmarks, zeven tabstops achter elkaar, en pijltjes die nergens
               meer heen gingen.
               De balk blijft heel en wordt smal via `variant`: alleen iconen
               meet 314px tegen 878px met tekst, dus hij past tot ruim onder
               400px. De tekst blijft de toegankelijke naam van elk item. -->
          <!-- Hoogste priority: de navigatie is het laatste wat mag wijken.
               Namens wie (20) en het profiel (30) gaan eerst het menu in, en
               die hebben daar allebei een eigen menu-variant voor. -->
          <nldd-toolbar-item slot="start" :priority="90">
            <nldd-tab-bar
              size="md"
              navigation
              accessible-label="Demo-onderdeel"
              :variant="tabVariant"
            >
              <nldd-tab-bar-item
                v-for="tab in tabs"
                :key="tab.name"
                :text="tab.text"
                :href="tab.to"
                :selected="isActive(tab) || undefined"
                @click.prevent="router.push(tab.to)"
              >
                <nldd-icon slot="icon" :name="tab.icon"></nldd-icon>
              </nldd-tab-bar-item>
            </nldd-tab-bar>
            <!-- Vangnet: past zelfs de iconenbalk niet meer, dan verbergt de
                 toolbar dit item en komen de tabbladen hier terug. Zonder dit
                 was de navigatie onder 500px weg, dezelfde fout als eerst. -->
            <nldd-menu-group slot="overflow" text="Ga naar">
              <!-- Met `href` rendert het item als een echte link en zet het
                   `aria-current="page"` op het actieve tabblad. Zonder href
                   doet `selected` hier niets: het vinkje hoort bij checkbox
                   en radio, en aria-current komt alleen op de link-variant.
                   Middenklik en 'openen in nieuw tabblad' werken zo ook. -->
              <nldd-menu-item
                v-for="tab in tabs"
                :key="tab.name"
                :text="tab.text"
                :icon="tab.icon"
                :href="tab.to"
                :selected="isActive(tab) || undefined"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end" v-if="openCases > 0">
            <nldd-button size="sm" variant="neutral-tinted" start-icon="inbox" :text="`${openCases} te beoordelen`" @click="router.push('/zaaksysteem')"></nldd-button>
          </nldd-toolbar-item>
          <!-- Namens wie: alleen als de wet meer dan één mogelijkheid geeft.
               Staat naast het profiel, want het hoort bij wie er ingelogd is. -->
          <nldd-toolbar-item slot="end" v-if="showDelegation" class="rr-hide-presenting" :priority="20">
            <nldd-button
              size="md"
              :variant="activeDelegation ? 'accent-tinted' : 'neutral-transparent'"
              :start-icon="activeDelegation ? DELEGATION_ICONS[activeDelegation.subjectType] : 'switch'"
              :text="delegationButtonText"
              expandable
              popup-type="menu"
            >
              <nldd-menu slot="popup" accessible-label="Namens wie" @select="onDelegationSelect">
                <nldd-menu-item
                  v-for="d in delegations"
                  :key="`${d.subjectType}:${d.subjectId}`"
                  type="radio"
                  :value="`${d.subjectType}:${d.subjectId}`"
                  :text="d.subjectName"
                  :details="delegationLabel(d)"
                  :icon="DELEGATION_ICONS[d.subjectType] ?? 'person'"
                  :selected="(activeDelegation ? `${activeDelegation.subjectType}:${activeDelegation.subjectId}` : `SELF:${profile?.bsn}`) === `${d.subjectType}:${d.subjectId}` || undefined"
                ></nldd-menu-item>
              </nldd-menu>
            </nldd-button>
            <!-- Dezelfde keuze als menu, voor als de knop niet meer past.
                 `@select` per item: het overloopmenu toont een kloon en de
                 toolbar dispatcht `select` op het originele item, dus een
                 handler op de groep eromheen vuurt nooit. -->
            <nldd-menu-group slot="overflow" text="Namens wie">
              <nldd-menu-item
                v-for="d in delegations"
                :key="`${d.subjectType}:${d.subjectId}`"
                type="radio"
                :value="`${d.subjectType}:${d.subjectId}`"
                :text="d.subjectName"
                :details="delegationLabel(d)"
                :icon="DELEGATION_ICONS[d.subjectType] ?? 'person'"
                :selected="(activeDelegation ? `${activeDelegation.subjectType}:${activeDelegation.subjectId}` : `SELF:${profile?.bsn}`) === `${d.subjectType}:${d.subjectId}` || undefined"
                @select="demo.setDelegation(d)"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end" v-if="profile" class="rr-hide-presenting" :priority="30">
            <nldd-button size="md" variant="neutral-transparent" start-icon="person" :text="profile.name" expandable popup-type="menu">
              <nldd-menu slot="popup" accessible-label="Demoprofiel" @select="onProfileSelect">
                <nldd-menu-item
                  v-for="[key, p] in profileOptions"
                  :key="key"
                  type="radio"
                  :value="key"
                  :text="p.name"
                  :details="p.type"
                  :selected="profileKey === key || undefined"
                ></nldd-menu-item>
              </nldd-menu>
            </nldd-button>
            <!-- Idem voor het personage: zonder deze variant verdween de
                 profielkiezer onder 1130px zonder vervanging. -->
            <nldd-menu-group slot="overflow" text="Demoprofiel">
              <nldd-menu-item
                v-for="[key, p] in profileOptions"
                :key="key"
                type="radio"
                :value="key"
                :text="p.name"
                :details="p.type"
                :selected="profileKey === key || undefined"
                @select="demo.setProfile(key)"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
          <!-- Het demo-menu hangt aan de toolbar zelf en niet meer aan een eigen
               ellipsis-knop. Die knop stond náást de overloopknop van de
               toolbar, allebei als `...`, en zodra er iets overliep won de
               overloopknop: het demo-menu was dan onbereikbaar. Als vaste
               inhoud van `slot="overflow"` staat alles onder één knop, met de
               overgelopen tabbladen erboven. -->
          <!-- Alles plat in groepen, geen uitklappers. Een submenu werkt hier
               niet: nldd-menu verplaatst een geopend submenu naar
               document.body, en de toolbar luistert op het hoofdmenu om het
               `select` van een kloon terug te mappen naar het origineel.
               Buiten dat menu bubbelt het event er nooit heen, dus Features en
               Weergave deden als uitklapper helemaal niets, op elke breedte.
               Groepen mét een titel geven dezelfde ordening zonder die klik. -->
          <nldd-menu-group slot="overflow" text="Features">
            <!-- Geen `details` op deze items: dat is een kort label rechts,
                 geen ondertitel. Een hele zin erin duwt het label op een smal
                 scherm in een kolom van één woord breed, zodat "Wijziging
                 doorgeven" over vier regels brak. -->
            <nldd-menu-item
              v-for="f in FEATURES"
              :key="f.key"
              type="checkbox"
              :text="f.label"
              :icon="f.icon"
              :selected="features[f.key] || undefined"
              @select="demo.toggleFeature(f.key)"
            ></nldd-menu-item>
            <!-- Handmatig beoordelen staat hier zonder streep ertussen: het is
                 dezelfde soort schakelaar als de vlaggen erboven. Het gaat over
                 aanvragen waar 'correcties direct goedkeuren' over correcties
                 gaat, maar dat is geen ander soort instelling. Het verschil is
                 alleen dat het in de demostaat zit en niet in de vlaggen, en
                 dat is niets wat een presentator hoeft te zien. -->
            <nldd-menu-item
              type="checkbox"
              text="Alle aanvragen handmatig beoordelen"
              icon="checklist"
              :selected="state.manualReview || undefined"
              @select="toggleManualReview"
            ></nldd-menu-item>
            <nldd-menu-item
              v-if="hasFeatureOverrides"
              text="Terug naar het profiel"
              icon="refresh"
              @select="demo.resetFeatures()"
            ></nldd-menu-item>
          </nldd-menu-group>
          <nldd-menu-group slot="overflow" text="Weergave">
            <nldd-menu-item
              v-for="[value, label, icon] in colorSchemeOptions"
              :key="value"
              type="radio"
              :value="value"
              :text="label"
              :icon="icon"
              :selected="colorScheme === value || undefined"
              @select="setColorScheme(value)"
            ></nldd-menu-item>
          </nldd-menu-group>
          <nldd-menu-group slot="overflow" text="Demo">
            <nldd-menu-item text="Volledig scherm" icon="square-arrow-up" @select="toggleFullscreen"></nldd-menu-item>
            <nldd-menu-item text="Demo resetten…" icon="refresh" @select="askReset"></nldd-menu-item>
          </nldd-menu-group>
        </nldd-toolbar>
      </nldd-container>

      <nldd-split-view-pane slot="main" has-content>
        <nldd-page v-if="loadError">
          <nldd-simple-section>
            <nldd-banner variant="critical" text="De demo kon niet starten" :supporting-text="String(loadError)"></nldd-banner>
          </nldd-simple-section>
        </nldd-page>
        <nldd-page v-else-if="!ready">
          <nldd-simple-section height="60vh">
            <nldd-activity-indicator show-text text="Wetten en engine laden…" timing="instant" size="48"></nldd-activity-indicator>
          </nldd-simple-section>
        </nldd-page>
        <router-view v-else v-slot="{ Component }">
          <keep-alive>
            <component :is="Component" />
          </keep-alive>
        </router-view>
      </nldd-split-view-pane>
    </nldd-bar-split-view>

    <nldd-modal-dialog
      ref="resetDialog"
      variant="alert"
      text="Demo resetten?"
      supporting-text="Alle aanvragen en correcties uit deze demo worden gewist. De wetten en persona's blijven."
      accessible-label="Demo resetten"
    >
      <nldd-button slot="actions" variant="destructive" text="Resetten" @click="confirmReset"></nldd-button>
      <nldd-button slot="actions" variant="secondary" text="Annuleren" @click="resetDialog?.hide?.()"></nldd-button>
    </nldd-modal-dialog>
  </nldd-app-view>
</template>

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

const profileOptions = computed(() => Object.entries(corpus.value?.config?.profiles ?? {}));

function onProfileSelect(e) {
  const value = e.target?.getAttribute?.('value');
  if (value) demo.setProfile(value);
}

// ---- machtigingen ----------------------------------------------------------
// Namens wie er gehandeld wordt. De lijst komt uit de wet (elke wet met
// discoverable: DELEGATION_PROVIDER), niet uit de app.

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
          <nldd-toolbar-item slot="start">
            <nldd-tab-bar size="md" navigation accessible-label="Demo-onderdelen" :compact="presentation.active.value || undefined">
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
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end" v-if="openCases > 0">
            <nldd-button size="sm" variant="neutral-tinted" start-icon="inbox" :text="`${openCases} te beoordelen`" @click="router.push('/zaaksysteem')"></nldd-button>
          </nldd-toolbar-item>
          <!-- Namens wie: alleen als de wet meer dan één mogelijkheid geeft.
               Staat naast het profiel, want het hoort bij wie er ingelogd is. -->
          <nldd-toolbar-item slot="end" v-if="showDelegation" class="rr-hide-presenting">
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
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end" v-if="profile" class="rr-hide-presenting">
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
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button size="md" variant="neutral-transparent" icon="ellipsis" text="Meer" tooltip-timing="never" popup-type="menu">
              <nldd-menu slot="popup" accessible-label="Demo-menu">
                <nldd-menu-item
                  type="checkbox"
                  text="Alle aanvragen handmatig beoordelen"
                  icon="checklist"
                  :selected="state.manualReview || undefined"
                  @select="toggleManualReview"
                ></nldd-menu-item>
                <nldd-menu-divider></nldd-menu-divider>
                <!-- De features aan en uit, midden in een demo. De POC kon dit
                     alleen via omgevingsvariabelen bij het starten; een
                     presentator moet het tijdens zijn verhaal kunnen omzetten.
                     In een uitklapper zoals Weergave, zodat het hoofdmenu kort
                     blijft; het aantal aanstaande features staat ernaast, want
                     dat is wat je wilt weten zonder open te klappen. -->
                <nldd-menu-item text="Features" icon="puzzle-piece" :details="featureSummary">
                  <nldd-menu accessible-label="Features">
                    <nldd-menu-item
                      v-for="f in FEATURES"
                      :key="f.key"
                      type="checkbox"
                      :text="f.label"
                      :details="f.hint"
                      :icon="f.icon"
                      :selected="features[f.key] || undefined"
                      @select="demo.toggleFeature(f.key)"
                    ></nldd-menu-item>
                    <template v-if="hasFeatureOverrides">
                      <nldd-menu-divider></nldd-menu-divider>
                      <nldd-menu-item
                        text="Terug naar het profiel"
                        icon="refresh"
                        @select="demo.resetFeatures()"
                      ></nldd-menu-item>
                    </template>
                  </nldd-menu>
                </nldd-menu-item>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item text="Weergave" icon="appearance">
                  <nldd-menu @select="onColorSchemeSelect">
                    <nldd-menu-item
                      v-for="[value, label, icon] in colorSchemeOptions"
                      :key="value"
                      type="radio"
                      :value="value"
                      :text="label"
                      :icon="icon"
                      :selected="colorScheme === value || undefined"
                    ></nldd-menu-item>
                  </nldd-menu>
                </nldd-menu-item>
                <nldd-menu-item text="Volledig scherm" icon="square-arrow-up" @select="toggleFullscreen"></nldd-menu-item>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item text="Demo resetten…" icon="refresh" @select="askReset"></nldd-menu-item>
              </nldd-menu>
            </nldd-icon-button>
          </nldd-toolbar-item>
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

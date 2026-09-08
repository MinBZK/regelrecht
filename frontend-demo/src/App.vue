<script setup>
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useColorScheme } from '@regelrecht/frontend-shared';
import { useDemo } from './store/demoStore.js';

// The workspace shell: one bar with the tab bar and the presenter menu, and
// the active tab below it. Every tab is a route; <keep-alive> keeps the tabs
// the presenter already visited mounted, so opened law tabs, a running
// scenario or an expanded tile survive switching back and forth.

const route = useRoute();
const router = useRouter();
const demo = useDemo();
const { ready, loadError, profile, profileKey, corpus, state } = demo;

onMounted(() => {
  demo.boot().catch(() => {});
});

const tabs = computed(() => [
  { name: 'presentatie', text: 'Presentatie', icon: 'display', to: '/' },
  { name: 'wetten', text: 'Wetten', icon: 'books', to: '/wetten' },
  { name: 'graaf', text: 'Graaf', icon: 'centralized-network', to: '/graaf' },
  { name: 'scenarios', text: "Scenario's", icon: 'checklist', to: '/scenarios' },
  { name: 'portaal', text: profile.value?.portal_tab_label ?? 'Burger.nl', icon: 'person', to: '/portaal' },
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
  <nldd-app-view background="tinted">
    <nldd-bar-split-view>
      <nldd-container slot="toolbar" padding="8" background="base">
        <nldd-toolbar size="md" label="Werkruimte">
          <nldd-toolbar-item slot="start">
            <img src="/favicon.svg" alt="" width="28" height="28" style="display:block" />
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="start">
            <nldd-tab-bar size="md" navigation accessible-label="Demo-onderdelen">
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
            <nldd-tag color="warning" :text="`${openCases} te beoordelen`" icon="inbox"></nldd-tag>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end" v-if="profile">
            <nldd-tag color="accent" :text="`Profiel: ${profile.name}`" icon="person"></nldd-tag>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button size="md" icon="menu" text="Menu" tooltip-timing="never" expandable>
              <nldd-menu slot="popup" accessible-label="Demo-menu">
                <nldd-menu-item text="Demoprofiel" icon="users">
                  <nldd-menu @select="onProfileSelect">
                    <nldd-menu-item
                      v-for="[key, p] in profileOptions"
                      :key="key"
                      type="radio"
                      :value="key"
                      :text="`${p.name} (${p.type})`"
                      :selected="profileKey === key || undefined"
                    ></nldd-menu-item>
                  </nldd-menu>
                </nldd-menu-item>
                <nldd-menu-item
                  type="checkbox"
                  text="Alle aanvragen handmatig beoordelen"
                  icon="checklist"
                  :selected="state.manualReview || undefined"
                  @select="toggleManualReview"
                ></nldd-menu-item>
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

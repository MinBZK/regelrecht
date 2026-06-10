<script setup>
import { computed, ref, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import SearchPopover from './components/SearchPopover.vue';
import TrajectCreateForm from './components/TrajectCreateForm.vue';
import { createTraject } from './composables/useTrajects.js';
import { useAuth } from './composables/useAuth.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useColorScheme } from './composables/useColorScheme.js';
import { lastLibraryPath, sectionTarget } from './composables/useLastVisitedRoute.js';

// Nieuw-traject-pagina — bereikbaar vanaf de trajectkeuze-pagina
// (/editor). Gebruikt hetzelfde gedeelde formulier als de
// TrajectMenu-sheet; na het aanmaken ga je direct de editor in op het
// nieuwe traject, met een eventueel meegekregen wet (query
// `law`/`article`) voorgeselecteerd.
const route = useRoute();
const router = useRouter();
const { authenticated, loading: authLoading, person, login, logout } = useAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();
const { colorScheme, setColorScheme } = useColorScheme();

const colorSchemeOptions = [
  ['auto', 'Systeem'],
  ['light', 'Licht'],
  ['dark', 'Donker'],
];

// Kept in sync with LibraryApp/EditorApp so toggling from this page
// affects those views the next time they mount.
const editorPanelFlags = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
];

const formEl = ref(null);
const createBusy = ref(false);
const createError = ref(null);

watchEffect(() => {
  document.title = 'Nieuw traject · RegelRecht';
});

// Terug naar de keuzepagina, met behoud van een meegekregen wet.
const backTarget = computed(() => ({ name: 'editor', query: route.query }));

// Bibliotheek-tab: naar de laatst bezochte bibliotheekpositie, zonder
// traject-scope (geen actief traject op deze pagina).
const libraryTarget = computed(() => sectionTarget(router, lastLibraryPath.value, null));
const libraryTabHref = computed(() => router.resolve(libraryTarget.value).href);

// --- Search (mirrors LibraryApp) ---
const searchPopoverRef = ref(null);

function openSearch(e, initialSearch = '') {
  searchPopoverRef.value?.show(e?.currentTarget, initialSearch);
}

/**
 * Spotlight-style: any printable single-character keystroke on the bar's
 * search-field opens the popover with that character as the initial query.
 * preventDefault keeps the character out of the bar's own input — popover
 * shows it instead. Modifier-combos (Ctrl-A, Cmd-V, etc.), Tab, Enter,
 * arrows etc. fall through (length !== 1).
 */
function onBarSearchKeydown(e) {
  if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
    e.preventDefault();
    openSearch(e, e.key);
  }
}

// Deze pagina heeft zelf geen wetweergave — een zoekresultaat opent in de
// bibliotheek.
function openLawFromSearch(lawId) {
  router.push({ name: 'library', params: { lawId } });
}

async function submitCreate() {
  createError.value = null;
  const { payload, error } = formEl.value.buildPayload();
  if (error) {
    createError.value = error;
    return;
  }
  createBusy.value = true;
  try {
    const created = await createTraject(payload);
    // Mirror the TrajectMenu guard: the backend's `create` handler always
    // calls `fill_ref()`, but a half-filled TrajectSummary would otherwise
    // navigate to `/editor/undefined/...` and silently no-op.
    if (!created.ref) {
      console.warn('TrajectCreateApp: created traject has no ref', created);
      return;
    }
    await router.push({
      name: 'editor-traject',
      params: {
        trajectRef: created.ref,
        lawId: route.query.law || undefined,
        articleNumber: route.query.article || undefined,
      },
    });
  } catch (e) {
    createError.value = e.message || 'Aanmaken mislukt';
  } finally {
    createBusy.value = false;
  }
}
</script>

<template>
  <nldd-app-view>
    <nldd-bar-split-view>
      <!-- Primary Bar: md only — search and settings as buttons -->
      <nldd-split-view-pane slot="primary-bar-md" only="md">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation>
                <nldd-tab-bar-item :href="libraryTabHref" @click.prevent="router.push(libraryTarget)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="md" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-md" size="md" icon="account" text="Account" expandable tooltip-timing="never" popovertarget="settings-menu-md"></nldd-icon-button>
              <nldd-menu id="settings-menu-md" anchor="settings-menu-btn-md">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <template v-else-if="!authLoading && !authenticated">
                  <nldd-menu-item text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                  <nldd-menu-divider></nldd-menu-divider>
                </template>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in editorPanelFlags"
                  :key="key"
                  type="checkbox"
                  :selected="isEnabled(key) || undefined"
                  :text="label"
                  @select="toggleFlag(key)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-group text="Thema">
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-md-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-divider v-if="!authLoading && authenticated"></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Primary Bar: lg+ — search as input, settings as button -->
      <nldd-split-view-pane slot="primary-bar-lg" above="lg">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation>
                <nldd-tab-bar-item :href="libraryTabHref" @click.prevent="router.push(libraryTarget)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="center" min-width="240px" width="33%" max-width="480px">
              <nldd-search-field
                size="md"
                placeholder="Zoeken"
                @click="openSearch"
                @keydown="onBarSearchKeydown"
              ></nldd-search-field>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-lg" size="md" icon="account" text="Account" expandable tooltip-timing="never" popovertarget="settings-menu-lg"></nldd-icon-button>
              <nldd-menu id="settings-menu-lg" anchor="settings-menu-btn-lg">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <template v-else-if="!authLoading && !authenticated">
                  <nldd-menu-item text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                  <nldd-menu-divider></nldd-menu-divider>
                </template>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in editorPanelFlags"
                  :key="key"
                  type="checkbox"
                  :selected="isEnabled(key) || undefined"
                  :text="label"
                  @select="toggleFlag(key)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-group text="Thema">
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-lg-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-divider v-if="!authLoading && authenticated"></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Main: aanmaakformulier -->
      <nldd-split-view-pane slot="main">
        <nldd-page sticky-header>
          <nldd-top-title-bar
            slot="header"
            text="Nieuw traject"
            :back-text="createBusy ? undefined : 'Kies een traject'"
            collapse-anchor="nieuw-traject-titel"
            @back="router.push(backTarget)"
          ></nldd-top-title-bar>

          <nldd-simple-section width="800px">
            <nldd-title id="nieuw-traject-titel" size="3"><h3>Nieuw traject</h3></nldd-title>
            <nldd-spacer size="16"></nldd-spacer>
            <TrajectCreateForm ref="formEl"
              :busy="createBusy"
              :error="createError"
              @submit="submitCreate"
            />
          </nldd-simple-section>
        </nldd-page>
      </nldd-split-view-pane>

      <!-- Mobile Bar (sm only): tab bar + icon-buttons for search and settings -->
      <nldd-split-view-pane slot="mobile-bar" only="sm">
        <nldd-container padding="8">
          <nldd-toolbar size="lg">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar navigation>
                <nldd-tab-bar-item :href="libraryTabHref" @click.prevent="router.push(libraryTarget)" icon="books" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected icon="edit" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button icon="search" text="Zoeken" @click="openSearch"></nldd-icon-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-sm" icon="account" text="Account" popovertarget="settings-menu-sm"></nldd-icon-button>
              <nldd-menu id="settings-menu-sm" anchor="settings-menu-btn-sm">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <template v-else-if="!authLoading && !authenticated">
                  <nldd-menu-item text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                  <nldd-menu-divider></nldd-menu-divider>
                </template>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in editorPanelFlags"
                  :key="key"
                  type="checkbox"
                  :selected="isEnabled(key) || undefined"
                  :text="label"
                  @select="toggleFlag(key)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-group text="Thema">
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-sm-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-divider v-if="!authLoading && authenticated"></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-app-view>

  <SearchPopover
    ref="searchPopoverRef"
    @select-law="openLawFromSearch"
  />
</template>

<script setup>
import { computed, onMounted, ref, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import SearchPopover from './components/SearchPopover.vue';
import { useTrajects, refreshTrajects } from './composables/useTrajects.js';
import { useAuth } from './composables/useAuth.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useColorScheme } from './composables/useColorScheme.js';
import { lastLibraryPath, sectionTarget } from './composables/useLastVisitedRoute.js';

// Trajectkeuze-pagina — de landing van de kale /editor. De editor vereist
// een traject; hier kies je een bestaand traject of ga je door naar de
// aanmaakpagina. Een meegekregen wet (query `law`/`article`, gezet door de
// redirect van oude no-traject editor-links) opent na de keuze direct.
const route = useRoute();
const router = useRouter();
const { trajects, loading, error } = useTrajects();
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

// Always refetch on entry: the landing page should reflect trajects
// created elsewhere (other tab, other device) since the cached
// module-level fetch.
onMounted(() => {
  refreshTrajects();
});

watchEffect(() => {
  document.title = 'Kies een traject · RegelRecht';
});

// Bibliotheek-tab + terugknop: naar de laatst bezochte bibliotheekpositie.
// Geen actief traject op deze pagina, dus sectionTarget strips een
// eventueel traject uit het opgeslagen pad.
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

function editorTarget(trajectRef) {
  return {
    name: 'editor-traject',
    params: {
      trajectRef,
      lawId: route.query.law || undefined,
      articleNumber: route.query.article || undefined,
    },
  };
}

function selectTraject(t) {
  // `t.ref` serialises to null when the backend builds a TrajectSummary
  // without fill_ref() — refuse to navigate (same guard as TrajectMenu).
  if (!t.ref) {
    console.warn('TrajectChooser: traject has no ref', t);
    return;
  }
  router.push(editorTarget(t.ref));
}

function openCreate() {
  // Query meegeven zodat een meegekregen wet ook na het aanmaken opent.
  router.push({ name: 'editor-nieuw-traject', query: route.query });
}

function trajectSupportingText(t) {
  const parts = [];
  if (t.status === 'afgerond') parts.push('Afgerond');
  if (t.description) parts.push(t.description);
  return parts.join(' — ') || undefined;
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

      <!-- Main: trajectkeuze -->
      <nldd-split-view-pane slot="main">
        <nldd-page sticky-header>
          <nldd-top-title-bar
            slot="header"
            text="Kies een traject"
            collapse-anchor="kies-traject-titel"
          ></nldd-top-title-bar>

          <nldd-simple-section width="800px">
            <nldd-title id="kies-traject-titel" size="3"><h3>Kies een traject</h3></nldd-title>
            <nldd-spacer size="16"></nldd-spacer>
            <nldd-activity-indicator v-if="loading" text="Trajecten laden" show-text></nldd-activity-indicator>
            <nldd-inline-dialog
              v-else-if="error"
              variant="alert"
              text="Trajecten zijn niet geladen"
              supporting-text="De gegevens konden niet worden opgehaald."
            >
              <nldd-button slot="actions" variant="primary" text="Probeer opnieuw" @click="refreshTrajects()"></nldd-button>
            </nldd-inline-dialog>
            <!-- "Nieuw traject" is een gewoon list item onderaan, zodat de
                 interactie identiek is mét bestaande trajecten (onderaan de
                 lijst) en zonder (als enige item). -->
            <nldd-list v-else variant="box">
              <nldd-list-item
                v-for="t in trajects"
                :key="t.id"
                size="md"
                type="button"
                @click="selectTraject(t)"
              >
                <nldd-text-cell :text="t.name" :supporting-text="trajectSupportingText(t)"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-icon-cell size="20">
                  <nldd-icon name="chevron-right"></nldd-icon>
                </nldd-icon-cell>
              </nldd-list-item>
              <nldd-list-item size="md"
                type="button"
                @click="openCreate"
              >
                <nldd-text-cell text="Nieuw traject"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-icon-cell size="20">
                  <nldd-icon name="chevron-right"></nldd-icon>
                </nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
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

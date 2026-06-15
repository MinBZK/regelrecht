<script setup>
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import TrajectMenu from './components/TrajectMenu.vue';
import TrajectDocuments from './components/TrajectDocuments.vue';
import { useAuth } from './composables/useAuth.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useColorScheme } from './composables/useColorScheme.js';
import { useTrajects } from './composables/useTrajects.js';
import {
  lastLibraryPath,
  lastEditorPath,
  sectionTarget,
} from './composables/useLastVisitedRoute.js';
import { useAppChrome, openSearch, onBarSearchKeydown } from './composables/useAppChrome.js';

// Persistent shell that owns the shared chrome (tab-bar, search trigger,
// TrajectMenu, settings menu) and a nested <router-view> for the editor /
// library bodies. Because both views are children of this one route record,
// switching between them swaps only the nested router-view — the shell
// instance is reused, so the chrome never rebuilds (no refresh flash).

const { authenticated, loading: authLoading, oidcConfigured, person, login, logout } = useAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();
const { colorScheme, setColorScheme } = useColorScheme();
const { activeTrajectRef } = useTrajects();

const colorSchemeOptions = [
  ['auto', 'Systeem'],
  ['light', 'Licht'],
  ['dark', 'Donker'],
];

// Editor panel feature flags, toggled from the settings menu of either
// section (the menu lives in the shell now, so the full editor set is in one
// place — the library previously listed only the first four). Toggling from
// the library affects the editor the next time its panes render.
const editorPanelFlags = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
  // Capability gate: when on, the Tekst pane offers a "Notities" toggle that
  // overlays resolved notes on the article text. Not a separate pane (notes
  // are a layer over the text, not other content).
  ['panel.notes', 'Notities'],
  // Note authoring (RFC-018 write path). Separate gate from panel.notes so
  // notes can be shown read-only without exposing the (MVP, local-only)
  // creation + export flow.
  ['notes.create', 'Notities aanmaken'],
  ['editor.article_text_edit', 'Tekst bewerken'],
];

const route = useRoute();
const router = useRouter();

// Which top-level section is active, derived from the route. Both tabs are
// always rendered; the active one shows `selected` and does not navigate,
// the other carries the cross-section target (traject re-stamped via
// sectionTarget) — matching the previous per-app behaviour.
const isLibraryRoute = computed(
  () => route.name === 'library' || route.name === 'library-traject',
);
const libraryTabTarget = computed(() =>
  sectionTarget(router, lastLibraryPath.value, activeTrajectRef.value),
);
const editorTabTarget = computed(() =>
  sectionTarget(router, lastEditorPath.value, activeTrajectRef.value),
);
const libraryTabHref = computed(() => router.resolve(libraryTabTarget.value).href);
const editorTabHref = computed(() => router.resolve(editorTabTarget.value).href);

// View-specific toolbar bits published by the active view.
const { lastSavedPr, documentTabs, activeDocumentTab, tabActions } = useAppChrome();
</script>

<template>
  <nldd-app-view>
    <nldd-bar-split-view>
      <!-- Primary Bar: md only — search and settings as buttons.
           The divider sits under the bar on the library (no document tabs);
           the editor suppresses it because its document-tab-bar separates. -->
      <nldd-split-view-pane slot="primary-bar-md" only="md" :no-divider="!isLibraryRoute || undefined">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="isLibraryRoute ? editorTabHref : undefined" @click.prevent="isLibraryRoute && router.push(editorTabTarget)" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item v-if="lastSavedPr" slot="end">
              <!-- Federated write-back indicator (editor only). New tab so the
                   editor state isn't lost. -->
              <nldd-button size="md" start-icon="external-link" :text="`PR #${lastSavedPr.number}`" :href="lastSavedPr.url" target="_blank" rel="noopener"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="md" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <TrajectMenu id-suffix="md" />
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button id="settings-menu-btn-md" size="md" start-icon="account" text="Account" expandable popovertarget="settings-menu-md"></nldd-button>
              <nldd-menu id="settings-menu-md" anchor="settings-menu-btn-md">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
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
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login()"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Primary Bar: lg+ — search as input field in center slot -->
      <nldd-split-view-pane slot="primary-bar-lg" above="lg" :no-divider="!isLibraryRoute || undefined">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="isLibraryRoute ? editorTabHref : undefined" @click.prevent="isLibraryRoute && router.push(editorTabTarget)" text="Editor"></nldd-tab-bar-item>
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
            <nldd-toolbar-item v-if="lastSavedPr" slot="end">
              <nldd-button size="md" start-icon="external-link" :text="`PR #${lastSavedPr.number}`" :href="lastSavedPr.url" target="_blank" rel="noopener"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <TrajectMenu id-suffix="lg" />
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button id="settings-menu-btn-lg" size="md" start-icon="account" text="Account" expandable popovertarget="settings-menu-lg"></nldd-button>
              <nldd-menu id="settings-menu-lg" anchor="settings-menu-btn-lg">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
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
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login()"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Document Tab Bar (editor only, md+). Rendered only while the active
           view publishes open tabs, so the library never shows an empty bar. -->
      <nldd-split-view-pane v-if="documentTabs.length > 0 && tabActions" slot="document-tabs" sm-order="2">
        <nldd-container padding-inline="8" padding-top="0" padding-bottom="8" sm-padding-top="8" sm-padding-bottom="0">
          <nldd-document-tab-bar>
            <nldd-document-tab-bar-item
              v-for="tab in documentTabs"
              :key="tabActions.key(tab)"
              :text="`Artikel ${tab.articleNumber}`"
              :supporting-text="tabActions.displayName(tab)"
              :short-text="`Art. ${tab.articleNumber}`"
              :short-supporting-text="tabActions.displayName(tab)"
              :selected="activeDocumentTab && tabActions.key(activeDocumentTab) === tabActions.key(tab) || undefined"
              has-dismiss-button
              @click="tabActions.select(tab)"
              @dismiss="tabActions.close(tab)"
            >
            </nldd-document-tab-bar-item>
          </nldd-document-tab-bar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Main content area — the active section's body. -->
      <nldd-split-view-pane slot="main">
        <router-view />
      </nldd-split-view-pane>

      <!-- Mobile Bar (sm only): TrajectMenu on its own full-width row, then the
           tab bar + search + account row below. -->
      <nldd-split-view-pane slot="mobile-bar" only="sm">
        <nldd-container padding="8" padding-bottom="0">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start" width="100%">
              <TrajectMenu id-suffix="sm" full-width />
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
        <nldd-container padding="8">
          <nldd-toolbar size="lg">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" icon="books" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="isLibraryRoute ? editorTabHref : undefined" @click.prevent="isLibraryRoute && router.push(editorTabTarget)" icon="edit" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <span>
                <nldd-icon-button size="lg" icon="search" text="Zoeken" @click="openSearch"></nldd-icon-button>
              </span>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <span>
                <nldd-icon-button id="settings-menu-btn-sm" size="lg" icon="account" text="Account" popovertarget="settings-menu-sm"></nldd-icon-button>
              </span>
              <nldd-menu id="settings-menu-sm" anchor="settings-menu-btn-sm">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
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
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login()"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-app-view>

  <!-- Traject-documents browser sheet + edit window, opened from TrajectMenu.
       Shared by both sections, so it lives in the shell (mounted once). -->
  <TrajectDocuments />
</template>

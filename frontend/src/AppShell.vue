<script setup>
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import TrajectMenu from './components/TrajectMenu.vue';
import TrajectDocuments from './components/TrajectDocuments.vue';
import DocumentTabsSheet from './components/DocumentTabsSheet.vue';
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
const { lastSavedPr, documentTabs, activeDocumentTab, tabActions, editorChanges, editorActions } = useAppChrome();

// Editor with open tabs → the mobile traject row splits 50/50 to fit a tabs
// button next to the traject menu, and the md+ document-tab-bar shows. The
// library never publishes tabs, so its mobile row keeps the full-width traject
// menu (the two sections are intentionally decoupled here).
const hasDocumentTabs = computed(
  () => documentTabs.value.length > 0 && !!tabActions.value,
);
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
              <nldd-icon-button id="settings-menu-btn-md" size="md" icon="account" text="Account" tooltip-timing="never" expandable popovertarget="settings-menu-md"></nldd-icon-button>
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
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" icon="logout" @click="logout"></nldd-menu-item>
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
              <nldd-icon-button id="settings-menu-btn-lg" size="md" icon="account" text="Account" tooltip-timing="never" expandable popovertarget="settings-menu-lg"></nldd-icon-button>
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
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" icon="logout" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login()"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Document Tab Bar (editor only, md+). Hidden on sm — there the tabs
           live in the DocumentTabsSheet opened from the traject row. Rendered
           only while the active view publishes open tabs, so the library never
           shows an empty bar. -->
      <nldd-split-view-pane v-if="hasDocumentTabs" slot="document-tabs" above="md">
        <nldd-container padding-inline="8" padding-top="0" padding-bottom="8">
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

      <!-- Wijzigingenbalk (editor only): one article-level bar with Opslaan +
           Wijzigingen-ongedaan (+ text undo/redo), replacing the per-pane save
           footers. Published by EditorView via useAppChrome; shown only while
           the article has unsaved changes. Sits after `main` in the DOM, so on
           sm it lands above the two mobile bars and on md+ it's the bottom bar. -->
      <nldd-split-view-pane v-if="editorChanges && editorChanges.dirty" slot="changes-bar">
        <nldd-container padding="8" sm-padding-bottom="0">
          <nldd-toolbar size="md" label="Wijzigingen">
            <!-- Discard zit bewust achter een 'meer'-menu zodat deze
                 destructieve actie niet per ongeluk gekozen wordt. -->
            <nldd-toolbar-item slot="start" label="Meer acties" :priority="1">
              <nldd-icon-button
                id="changes-more-btn"
                icon="more"
                text="Meer acties"
                popup-type="menu"
                popovertarget="changes-more-menu"
              ></nldd-icon-button>
              <nldd-menu id="changes-more-menu" anchor="changes-more-btn">
                <nldd-menu-item
                  text="Maak alle wijzigingen ongedaan"
                  destructive
                  @select="editorActions?.discard?.()"
                ></nldd-menu-item>
              </nldd-menu>
              <nldd-menu-item
                slot="overflow"
                text="Maak alle wijzigingen ongedaan"
                destructive
                @select="editorActions?.discard?.()"
              ></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="start" label="Ongedaan maken / Opnieuw" :priority="1">
              <nldd-button-bar>
                <nldd-icon-button
                  icon="undo"
                  text="Ongedaan maken"
                  :disabled="!editorChanges.canUndo || undefined"
                  @click="editorActions?.undo?.()"
                ></nldd-icon-button>
                <nldd-button-bar-divider></nldd-button-bar-divider>
                <nldd-icon-button
                  icon="redo"
                  text="Opnieuw"
                  :disabled="!editorChanges.canRedo || undefined"
                  @click="editorActions?.redo?.()"
                ></nldd-icon-button>
              </nldd-button-bar>
              <nldd-menu-item
                slot="overflow"
                icon="undo"
                text="Ongedaan maken"
                :disabled="!editorChanges.canUndo || undefined"
                @select="editorActions?.undo?.()"
              ></nldd-menu-item>
              <nldd-menu-item
                slot="overflow"
                icon="redo"
                text="Opnieuw"
                :disabled="!editorChanges.canRedo || undefined"
                @select="editorActions?.redo?.()"
              ></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" label="Opslaan" width="320px" :priority="3">
              <nldd-button
                variant="primary"
                size="md"
                width="full"
                :disabled="editorChanges.saving || undefined"
                :text="editorChanges.saving ? 'Opslaan…' : 'Opslaan'"
                @click="editorActions?.save?.()"
              ></nldd-button>
              <nldd-menu-item
                slot="overflow"
                icon="save"
                :text="editorChanges.saving ? 'Opslaan…' : 'Opslaan'"
                :disabled="editorChanges.saving || undefined"
                @select="editorActions?.save?.()"
              ></nldd-menu-item>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Mobile Bar (sm only): traject row on top (in the editor it splits
           50/50 with the open-tabs button, replacing the document-tab-bar),
           then the tab bar + search + account row below. -->
      <nldd-split-view-pane slot="mobile-bar" only="sm">
        <nldd-container padding="8" padding-bottom="0">
          <nldd-toolbar size="md">
            <template v-if="hasDocumentTabs">
              <nldd-toolbar-item slot="start" width="50%">
                <TrajectMenu id-suffix="sm" full-width />
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end" width="50%">
                <DocumentTabsSheet />
              </nldd-toolbar-item>
            </template>
            <nldd-toolbar-item v-else slot="start" width="100%">
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
                <nldd-icon-button id="settings-menu-btn-sm" size="lg" icon="account" text="Account" tooltip-timing="never" popovertarget="settings-menu-sm"></nldd-icon-button>
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
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" icon="logout" @click="logout"></nldd-menu-item>
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

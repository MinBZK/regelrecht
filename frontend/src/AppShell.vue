<script setup>
import { computed, ref, onMounted, onBeforeUnmount } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import TrajectMenu from './components/TrajectMenu.vue';
import TrajectDocuments from './components/TrajectDocuments.vue';
import MobileTrajectSheet from './components/MobileTrajectSheet.vue';
import { useAuth } from './composables/useAuth.js';
import { useGithubAuth } from './composables/useGithubAuth.js';
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
// switching between them swaps only the nested router-view - the shell
// instance is reused, so the chrome never rebuilds (no refresh flash).

const { authenticated, loading: authLoading, oidcConfigured, person, hasAnyRole, login, logout } = useAuth();
// GitHub user-OAuth (spike): let a user link their own GitHub account so
// traject writes go out under their credential. `status` is reactive and may
// be null until loaded; the template guards on `githubStatus?.configured`.
const {
  status: githubStatus,
  connect: connectGithub,
  disconnect: disconnectGithub,
} = useGithubAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();

// Roles that may reach the harvester-admin "Corpusinwinning" section. Any harvester-*
// tier (reader/writer/admin) or the spanning regelrecht-admin sees the menu
// item; write actions inside the section are still enforced server-side by
// the harvester-admin API. Composite-role expansion means a higher tier
// already carries the lower ones, but we list all four so a directly-assigned
// role can never be missed.
const HARVESTER_ROLES = [
  'harvester-reader',
  'harvester-writer',
  'harvester-admin',
  'regelrecht-admin',
];
const canViewHarvesting = computed(
  () => authenticated.value && hasAnyRole(HARVESTER_ROLES),
);
function goToHarvesting() {
  router.push("/harvesting");
}
const { colorScheme, setColorScheme } = useColorScheme();
const { activeTrajectRef } = useTrajects();

const colorSchemeOptions = [
  ['auto', 'Systeem'],
  ['light', 'Licht'],
  ['dark', 'Donker'],
];

// Editor panel feature flags, toggled from the settings menu of either
// section (the menu lives in the shell now, so the full editor set is in one
// place - the library previously listed only the first four). Toggling from
// the library affects the editor the next time its panes render.
const editorPanelFlags = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
  // A read-only article-text pane with resolved note highlights + inline note
  // authoring. Kept separate from the Tekst editor so both can be shown side by
  // side for comparison.
  ['panel.notes', 'Tekst viewer + notities'],
];

// The "Functies" menu group: the panel flags, plus the GitHub-koppeling
// toggle when this deployment has a GitHub OAuth App configured. The flag
// (off by default) hides the Koppel/Ontkoppel items below so the spike is
// opt-in per user.
const functieFlags = computed(() =>
  githubStatus.value?.configured
    ? [...editorPanelFlags, ['github.user_oauth', 'GitHub-koppeling']]
    : editorPanelFlags,
);

const route = useRoute();
const router = useRouter();

// Which top-level section is active, derived from the route. Both tabs are
// always rendered; the active one shows `selected` and does not navigate,
// the other carries the cross-section target (traject re-stamped via
// sectionTarget) - matching the previous per-app behaviour.
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

// The editor requires login. Rather than letting the route guard bounce an
// unauthenticated user straight to the SSO screen (an unannounced surprise),
// intercept the Editor tab and first show a small login-warning popover anchored
// to the clicked tab. Authenticated users navigate as before.
const loginWarning = ref(null);
function onEditorTab(e) {
  if (!authenticated.value) {
    if (loginWarning.value) {
      loginWarning.value.anchorElement = e.currentTarget;
      loginWarning.value.show();
    }
    return;
  }
  if (isLibraryRoute.value) router.push(editorTabTarget.value);
}

// View-specific toolbar bits published by the active view.
const { lastSavedPr, documentTabs, activeDocumentTab, tabActions, editorChanges, editorActions, libraryEmpty } = useAppChrome();

// Just-in-time coach-mark on the toolbar search affordance: shown only while the
// library is empty (nothing curated yet) and never dismissable - the app drives
// it, and it disappears by itself once the library has content. Each breakpoint
// renders its search control in a different bar (sm icon-button, md text button,
// lg search field), each in a pane that is display:none off-breakpoint. So we
// resolve the active breakpoint and activate only the coach-mark whose control
// is actually visible, never anchoring a popover to a hidden control.
const viewport = ref('lg'); // 'sm' | 'md' | 'lg', aligned with the DS bar breakpoints
let mdQuery = null;
let lgQuery = null;
// DS bar breakpoints, mirrored here for matchMedia. Keep in sync with
// @nldd/design-system (src/assets/styles/breakpoints.ts): md >= 641px, lg >= 1008px.
// If the DS shifts these thresholds, update them here too - otherwise the
// coach-mark can anchor to a control that is hidden at the current breakpoint.
const DS_MD_MIN = '(min-width: 641px)';
const DS_LG_MIN = '(min-width: 1008px)';
function updateViewport() {
  viewport.value = lgQuery?.matches ? 'lg' : mdQuery?.matches ? 'md' : 'sm';
}
onMounted(() => {
  mdQuery = window.matchMedia?.(DS_MD_MIN) || null;
  lgQuery = window.matchMedia?.(DS_LG_MIN) || null;
  updateViewport();
  mdQuery?.addEventListener?.('change', updateViewport);
  lgQuery?.addEventListener?.('change', updateViewport);
});
onBeforeUnmount(() => {
  mdQuery?.removeEventListener?.('change', updateViewport);
  lgQuery?.removeEventListener?.('change', updateViewport);
});
const showSearchHintSm = computed(() => libraryEmpty.value && viewport.value === 'sm');
const showSearchHintMd = computed(() => libraryEmpty.value && viewport.value === 'md');
const showSearchHintLg = computed(() => libraryEmpty.value && viewport.value === 'lg');

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
      <!-- Primary Bar: md only - search and settings as buttons. The bar-split-
           view draws the divider automatically where the bar group meets main:
           on the library it sits under this bar; on the editor the document-tab-
           bar sits between, so toolbar + tabs read as one group above the single
           main divider. -->
      <nldd-split-view-pane slot="primary-bar-md" only="md">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item v-if="lastSavedPr" slot="end">
              <!-- Federated write-back indicator (editor only). New tab so the
                   editor state isn't lost. -->
              <nldd-button size="md" start-icon="external-link" :text="`PR #${lastSavedPr.number}`" :href="lastSavedPr.url" target="_blank" rel="noopener"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-just-in-time-education
                placement="bottom"
                arrow-length="160px"
                text="Zoek een wet om te openen"
                supporting-text="Markeer een wet als favoriet om die later snel terug te vinden."
                :active="showSearchHintMd || undefined"
              >
                <nldd-button size="md" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
              </nldd-just-in-time-education>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <TrajectMenu id-suffix="md" />
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-md" size="md" icon="account" text="Account" tooltip-timing="never" expandable popovertarget="settings-menu-md"></nldd-icon-button>
              <nldd-menu id="settings-menu-md" anchor="settings-menu-btn-md">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <nldd-menu-item v-if="canViewHarvesting" text="Harvester" icon="gear" @click.stop="goToHarvesting"></nldd-menu-item>
                <nldd-menu-divider v-if="canViewHarvesting"></nldd-menu-divider>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in functieFlags"
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
                <nldd-menu-item v-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth') && githubStatus?.connected" :text="'GitHub ontkoppelen (' + githubStatus.github_login + ')'" icon="dismiss" @click="disconnectGithub"></nldd-menu-item>
                <nldd-menu-item v-else-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth')" text="Koppel GitHub-account" icon="external-link" @click="connectGithub()"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" icon="logout" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Primary Bar: lg+ - search as input field in center slot -->
      <nldd-split-view-pane slot="primary-bar-lg" above="lg">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="center" min-width="240px" width="33%" max-width="480px">
              <nldd-just-in-time-education
                placement="bottom"
                arrow-length="160px"
                text="Zoek een wet om te openen"
                supporting-text="Markeer een wet als favoriet om die later snel terug te vinden."
                :active="showSearchHintLg || undefined"
              >
                <nldd-search-field
                  size="md"
                  placeholder="Zoeken"
                  @click="openSearch"
                  @keydown="onBarSearchKeydown"
                ></nldd-search-field>
              </nldd-just-in-time-education>
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
                <nldd-menu-item v-if="canViewHarvesting" text="Harvester" icon="gear" @click.stop="goToHarvesting"></nldd-menu-item>
                <nldd-menu-divider v-if="canViewHarvesting"></nldd-menu-divider>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in functieFlags"
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
                <nldd-menu-item v-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth') && githubStatus?.connected" :text="'GitHub ontkoppelen (' + githubStatus.github_login + ')'" icon="dismiss" @click="disconnectGithub"></nldd-menu-item>
                <nldd-menu-item v-else-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth')" text="Koppel GitHub-account" icon="external-link" @click="connectGithub()"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" icon="logout" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Document Tab Bar (editor only, md+). Hidden on sm - there the tabs
           live in the MobileTrajectSheet opened from the traject row. Rendered
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

      <!-- Main content area - the active section's body. -->
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
                tooltip-timing="never"
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

      <!-- Mobile Bar (sm only): the combined MobileTrajectSheet button on top
           (traject + open articles in one sheet), then the tab bar + search +
           account row below. -->
      <nldd-split-view-pane slot="mobile-bar" only="sm">
        <nldd-container padding="8" padding-bottom="0">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start" width="100%">
              <MobileTrajectSheet />
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
        <nldd-container padding="8">
          <nldd-toolbar size="lg">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" icon="books" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" icon="edit" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-just-in-time-education
                placement="top"
                arrow-length="160px"
                text="Zoek een wet om te openen"
                supporting-text="Markeer een wet als favoriet om die later snel terug te vinden."
                :active="showSearchHintSm || undefined"
              >
                <nldd-icon-button size="lg" icon="search" text="Zoeken" @click="openSearch"></nldd-icon-button>
              </nldd-just-in-time-education>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-sm" size="lg" icon="account" text="Account" tooltip-timing="never" popovertarget="settings-menu-sm"></nldd-icon-button>
              <nldd-menu id="settings-menu-sm" anchor="settings-menu-btn-sm">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <nldd-menu-item v-if="canViewHarvesting" text="Harvester" icon="gear" @click.stop="goToHarvesting"></nldd-menu-item>
                <nldd-menu-divider v-if="canViewHarvesting"></nldd-menu-divider>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in functieFlags"
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
                <nldd-menu-item v-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth') && githubStatus?.connected" :text="'GitHub ontkoppelen (' + githubStatus.github_login + ')'" icon="dismiss" @click="disconnectGithub"></nldd-menu-item>
                <nldd-menu-item v-else-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth')" text="Koppel GitHub-account" icon="external-link" @click="connectGithub()"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" icon="logout" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
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

  <!-- Editor requires login: a heads-up popover anchored to the clicked Editor
       tab (sm/md/lg) so the SSO screen never appears unannounced. -->
  <nldd-popover ref="loginWarning" accessible-label="Inloggen" width="320px">
    <nldd-container padding="16">
      <nldd-inline-dialog
        icon="login"
        text="Log in om de editor te gebruiken"
        supporting-text="Zodra je bent ingelogd kies je een traject en kun je aan de slag."
      >
        <nldd-button slot="actions" variant="primary" text="Inloggen" @click="login(editorTabHref)"></nldd-button>
      </nldd-inline-dialog>
    </nldd-container>
  </nldd-popover>
</template>

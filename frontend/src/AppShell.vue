<script setup>
import { computed, ref, nextTick, onMounted, onBeforeUnmount, provide } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import TrajectMenu from './components/TrajectMenu.vue';
import MobileTrajectSheet from './components/MobileTrajectSheet.vue';
import AboutSheet from './components/AboutSheet.vue';
import SupportSheet from './components/SupportSheet.vue';
import { useAuth } from './composables/useAuth.js';
import { useGithubAuth } from './composables/useGithubAuth.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useColorScheme } from './composables/useColorScheme.js';
import { useTrajects } from './composables/useTrajects.js';
import {
  lastHomePath,
  lastEditorPath,
  sectionTarget,
  isHomeSection,
  rememberHarvesterOrigin,
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
  // Remember where we came from so the harvester's back button returns here.
  rememberHarvesterOrigin(route.fullPath);
  router.push("/harvesting");
}
const { colorScheme, setColorScheme } = useColorScheme();
const { activeTrajectRef } = useTrajects();

// "Over RegelRecht" about sheet, opened from the account menu.
const aboutSheet = ref(null);
function openAbout() {
  // Let the account menu popover close first, then raise the sheet.
  nextTick(() => aboutSheet.value?.show?.());
}

// "Ondersteuning" support sheet, opened from the account menu.
const supportSheet = ref(null);
function openSupport() {
  // Let the account menu popover close first, then raise the sheet.
  nextTick(() => supportSheet.value?.show?.());
}

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
  ['panel.notes', 'Notities'],
];

// The "Functies" menu group: the panel flags, plus the GitHub-koppeling
// toggle when this deployment has a GitHub OAuth App configured. The flag
// (off by default, deployment-wide like all flags) is one switch with two
// effects: it shows the Koppel/Ontkoppel items below AND makes the backend
// require the acting user's own GitHub token for traject writes (a save
// without a linked token then 428s, which apiAuthGuard.js turns into a
// redirect through the connect flow).
const functieFlags = computed(() => [
  ...editorPanelFlags,
  ...(githubStatus.value?.configured ? [['github.user_oauth', 'GitHub-koppeling']] : []),
]);

const route = useRoute();
const router = useRouter();

// Which top-level section is active, derived from the route. Both tabs are
// always rendered; the active one shows `selected` and does not navigate,
// the other carries the cross-section target (traject re-stamped via
// sectionTarget) - matching the previous per-app behaviour.
// True on the Home section (public landing, a public law, the traject landing,
// or a traject law). Kept named isLibraryRoute to limit this refactor's blast
// radius.
const isLibraryRoute = computed(() => isHomeSection(route.name));
// Home tab restores the last home path verbatim (its own scope), like the
// harvester return - so you continue exactly where you were. The Editor tab
// keeps re-stamping the active traject (it carries the editor's traject logic:
// chooser + law-as-query when no traject is active).
const libraryTabTarget = computed(() => lastHomePath.value);
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
// Where the "Inloggen" button returns after SSO. Defaults to the editor tab;
// callers (e.g. the Bibliotheek "Bewerken" button) point it at a specific
// article so login lands straight on the page being edited.
const loginRedirect = ref(null);

// The login-warning popover is popover=auto: a re-tap on the trigger that opened
// it light-dismisses it on pointerdown, but showLoginWarning would then reopen
// it right away. Snapshot the open state at pointerdown (capture, before the
// dismiss) so a re-tap toggles it closed. Every login trigger (the Editor tabs,
// the Bibliotheek "Bewerken" button) wires @pointerdown.capture to this.
let loginWarningWasOpen = false;
function onLoginTriggerPointerdown() {
  loginWarningWasOpen = loginWarning.value?.open ?? false;
}
provide('onLoginTriggerPointerdown', onLoginTriggerPointerdown);

// Show the login-warning popover anchored to `anchorEl`. Provided to the nested
// views so every editor entry point (the Editor tab, the Bibliotheek
// "Bewerken" button) shows the same heads-up instead of bouncing to SSO.
function showLoginWarning(anchorEl, redirectHref) {
  if (!loginWarning.value) return;
  // Consume the pointerdown snapshot so a later programmatic call (no pointerdown,
  // e.g. gating on navigation) still shows instead of inheriting a stale flag.
  const wasOpen = loginWarningWasOpen;
  loginWarningWasOpen = false;
  // Re-tap on the trigger that opened it: close instead of reopening. hide() is a
  // no-op if native light-dismiss already closed it on pointerdown.
  if (wasOpen) {
    loginWarning.value.hide();
    return;
  }
  loginRedirect.value = redirectHref ?? editorTabHref.value;
  loginWarning.value.anchorElement = anchorEl;
  loginWarning.value.show();
}
provide('showLoginWarning', showLoginWarning);

// Secondary action on the login popover: to the public "Account aanvragen"
// page. Close the popover first so it isn't left hanging over the new page.
const accountRequestHref = computed(() => router.resolve({ name: 'account-aanvragen' }).href);
function goToAccountRequest() {
  loginWarning.value?.hide();
  router.push({ name: 'account-aanvragen' });
}

function onEditorTab(e) {
  if (!authenticated.value) {
    showLoginWarning(e.currentTarget);
    return;
  }
  if (isLibraryRoute.value) router.push(editorTabTarget.value);
}

// Enabling `github.user_oauth` is not a personal display preference like the
// panel flags listed around it: the flag is deployment-wide AND doubles as the
// backend's write-enforcement switch (`write_requires_user_token`), so turning
// it on makes every editor-writer's next traject save require a linked
// personal GitHub account (an unlinked user's save 428s into the connect
// flow). Intercept the enable with an explicit confirmation popover - same
// pattern as the login warning above. Disabling restores the pre-existing
// service-token behaviour and stays a plain toggle.
const enforcementConfirm = ref(null);
function onFunctieFlagSelect(key, e) {
  if (key === 'github.user_oauth' && !isEnabled(key)) {
    if (enforcementConfirm.value) {
      // Anchor to the settings button of whichever breakpoint menu fired the
      // select: the menu itself closes on select, and a popover must never be
      // anchored to a hidden element.
      const anchorId = e.currentTarget.closest('nldd-menu')?.getAttribute('anchor');
      enforcementConfirm.value.anchorElement =
        (anchorId && document.getElementById(anchorId)) || e.currentTarget;
      enforcementConfirm.value.show();
    }
    return;
  }
  toggleFlag(key);
}
function confirmUserOauthEnforcement() {
  enforcementConfirm.value?.hide();
  toggleFlag('github.user_oauth');
}
// Release the anchor when the popover closes (confirm, cancel, or light
// dismiss). nldd-popover toggles itself on EVERY subsequent click on its
// anchor element (popover.js `_handleDocumentClick`) - leaving the settings
// button as anchor would hijack it: the next account-menu click opens this
// popover and (auto-popover exclusivity) closes the menu.
function onEnforcementConfirmClose() {
  if (enforcementConfirm.value) enforcementConfirm.value.anchorElement = null;
}

// View-specific toolbar bits published by the active view.
const { lastSavedPr, documentTabs, activeDocumentTab, tabActions, editorChanges, editorActions, libraryEmpty } = useAppChrome();

// Just-in-time coach-mark on the toolbar search affordance: shown while the
// library is empty (nothing curated yet). In the bare corpus it's app-driven and
// non-dismissable (it disappears once there's content); inside a traject it's
// dismissable, since there are other functions to discover. Each breakpoint
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
// Inside a traject the coach-mark is dismissable (other functions to discover -
// Instellingen, Werkdocumenten): the dismiss button persists so it won't nag
// again; a click outside hides it for the session. In the bare corpus it stays
// non-dismissable until content appears.
const JIT_DISMISS_KEY = 'regelrecht:jit-traject-search-dismissed';
function loadJitDismissed() {
  try { return localStorage.getItem(JIT_DISMISS_KEY) === '1'; } catch { return false; }
}
const trajectActive = computed(() => !!activeTrajectRef.value);
const jitDismissed = ref(loadJitDismissed());
const jitHiddenSession = ref(false);
const searchHintActive = computed(
  () => libraryEmpty.value && !(trajectActive.value && (jitDismissed.value || jitHiddenSession.value)),
);
const showSearchHintSm = computed(() => searchHintActive.value && viewport.value === 'sm');
const showSearchHintMd = computed(() => searchHintActive.value && viewport.value === 'md');
const showSearchHintLg = computed(() => searchHintActive.value && viewport.value === 'lg');
function onSearchHintClose(e) {
  jitHiddenSession.value = true;
  if (e?.detail?.reason === 'dismissed') {
    jitDismissed.value = true;
    try { localStorage.setItem(JIT_DISMISS_KEY, '1'); } catch { /* ignore */ }
  }
}

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
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" text="Home"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" @pointerdown.capture="onLoginTriggerPointerdown" text="Editor"></nldd-tab-bar-item>
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
                :dismissable="trajectActive || undefined"
                @nldd-close="onSearchHintClose"
              >
                <nldd-button size="md" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
              </nldd-just-in-time-education>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button-bar size="md">
                <TrajectMenu id-suffix="md" />
                <nldd-button-bar-divider></nldd-button-bar-divider>
                <nldd-icon-button id="settings-menu-btn-md" size="md" icon="account" text="Account" tooltip-timing="never" expandable popovertarget="settings-menu-md"></nldd-icon-button>
              </nldd-button-bar>
              <nldd-menu id="settings-menu-md" anchor="settings-menu-btn-md">
                <nldd-menu-item v-if="!authLoading && oidcConfigured && !authenticated" text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && oidcConfigured && !authenticated" text="Account aanvragen" icon="new-account" @click="goToAccountRequest"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
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
                <nldd-menu-item v-if="canViewHarvesting" text="Harvester" icon="harvest" @click.stop="goToHarvesting"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Feature flags" icon="flag">
                  <nldd-menu>
                    <nldd-menu-item
                      v-for="[key, label] in functieFlags"
                      :key="key"
                      type="checkbox"
                      :selected="isEnabled(key) || undefined"
                      :text="label"
                      @select="onFunctieFlagSelect(key, $event)"
                    ></nldd-menu-item>
                  </nldd-menu>
                </nldd-menu-item>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item text="Over RegelRecht" icon="info" @click="openAbout"></nldd-menu-item>
                <nldd-menu-item text="Ondersteuning" icon="support" @click="openSupport"></nldd-menu-item>
                <nldd-menu-item v-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth') && githubStatus?.connected" :text="'GitHub ontkoppelen (' + githubStatus.github_login + ')'" icon="dismiss" @click="disconnectGithub"></nldd-menu-item>
                <nldd-menu-item v-else-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth')" text="Koppel GitHub-account" icon="external-link" @click="connectGithub()"></nldd-menu-item>
                <template v-if="!authLoading && authenticated">
                  <nldd-menu-divider></nldd-menu-divider>
                  <nldd-menu-item text="Log uit" icon="logout" @click="logout"></nldd-menu-item>
                </template>
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
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" text="Home"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" @pointerdown.capture="onLoginTriggerPointerdown" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="center" min-width="240px" width="33%" max-width="480px">
              <nldd-just-in-time-education
                placement="bottom"
                arrow-length="160px"
                text="Zoek een wet om te openen"
                supporting-text="Markeer een wet als favoriet om die later snel terug te vinden."
                :active="showSearchHintLg || undefined"
                :dismissable="trajectActive || undefined"
                @nldd-close="onSearchHintClose"
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
              <nldd-button-bar size="md">
                <TrajectMenu id-suffix="lg" />
                <nldd-button-bar-divider></nldd-button-bar-divider>
                <nldd-icon-button id="settings-menu-btn-lg" size="md" icon="account" text="Account" tooltip-timing="never" expandable popovertarget="settings-menu-lg"></nldd-icon-button>
              </nldd-button-bar>
              <nldd-menu id="settings-menu-lg" anchor="settings-menu-btn-lg">
                <nldd-menu-item v-if="!authLoading && oidcConfigured && !authenticated" text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && oidcConfigured && !authenticated" text="Account aanvragen" icon="new-account" @click="goToAccountRequest"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
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
                <nldd-menu-item v-if="canViewHarvesting" text="Harvester" icon="harvest" @click.stop="goToHarvesting"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Feature flags" icon="flag">
                  <nldd-menu>
                    <nldd-menu-item
                      v-for="[key, label] in functieFlags"
                      :key="key"
                      type="checkbox"
                      :selected="isEnabled(key) || undefined"
                      :text="label"
                      @select="onFunctieFlagSelect(key, $event)"
                    ></nldd-menu-item>
                  </nldd-menu>
                </nldd-menu-item>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item text="Over RegelRecht" icon="info" @click="openAbout"></nldd-menu-item>
                <nldd-menu-item text="Ondersteuning" icon="support" @click="openSupport"></nldd-menu-item>
                <nldd-menu-item v-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth') && githubStatus?.connected" :text="'GitHub ontkoppelen (' + githubStatus.github_login + ')'" icon="dismiss" @click="disconnectGithub"></nldd-menu-item>
                <nldd-menu-item v-else-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth')" text="Koppel GitHub-account" icon="external-link" @click="connectGithub()"></nldd-menu-item>
                <template v-if="!authLoading && authenticated">
                  <nldd-menu-divider></nldd-menu-divider>
                  <nldd-menu-item text="Log uit" icon="logout" @click="logout"></nldd-menu-item>
                </template>
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
                text="Opslaan"
                :loading="editorChanges.saving || undefined"
                @click="editorActions?.save?.()"
              ></nldd-button>
              <nldd-menu-item
                slot="overflow"
                icon="save"
                text="Opslaan"
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
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" icon="home" text="Home"></nldd-tab-bar-item>
                <nldd-tab-bar-item :selected="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" @pointerdown.capture="onLoginTriggerPointerdown" icon="edit" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-just-in-time-education
                placement="top"
                arrow-length="160px"
                text="Zoek een wet om te openen"
                supporting-text="Markeer een wet als favoriet om die later snel terug te vinden."
                :active="showSearchHintSm || undefined"
                :dismissable="trajectActive || undefined"
                @nldd-close="onSearchHintClose"
              >
                <nldd-icon-button size="lg" icon="search" text="Zoeken" @click="openSearch"></nldd-icon-button>
              </nldd-just-in-time-education>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-sm" size="lg" icon="account" text="Account" tooltip-timing="never" popovertarget="settings-menu-sm"></nldd-icon-button>
              <nldd-menu id="settings-menu-sm" anchor="settings-menu-btn-sm">
                <nldd-menu-item v-if="!authLoading && oidcConfigured && !authenticated" text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && oidcConfigured && !authenticated" text="Account aanvragen" icon="new-account" @click="goToAccountRequest"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
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
                <nldd-menu-item v-if="canViewHarvesting" text="Harvester" icon="harvest" @click.stop="goToHarvesting"></nldd-menu-item>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Feature flags" icon="flag">
                  <nldd-menu>
                    <nldd-menu-item
                      v-for="[key, label] in functieFlags"
                      :key="key"
                      type="checkbox"
                      :selected="isEnabled(key) || undefined"
                      :text="label"
                      @select="onFunctieFlagSelect(key, $event)"
                    ></nldd-menu-item>
                  </nldd-menu>
                </nldd-menu-item>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item text="Over RegelRecht" icon="info" @click="openAbout"></nldd-menu-item>
                <nldd-menu-item text="Ondersteuning" icon="support" @click="openSupport"></nldd-menu-item>
                <nldd-menu-item v-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth') && githubStatus?.connected" :text="'GitHub ontkoppelen (' + githubStatus.github_login + ')'" icon="dismiss" @click="disconnectGithub"></nldd-menu-item>
                <nldd-menu-item v-else-if="authenticated && githubStatus?.configured && isEnabled('github.user_oauth')" text="Koppel GitHub-account" icon="external-link" @click="connectGithub()"></nldd-menu-item>
                <template v-if="!authLoading && authenticated">
                  <nldd-menu-divider></nldd-menu-divider>
                  <nldd-menu-item text="Log uit" icon="logout" @click="logout"></nldd-menu-item>
                </template>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>
    </nldd-bar-split-view>

    <AboutSheet ref="aboutSheet"></AboutSheet>
    <SupportSheet ref="supportSheet"></SupportSheet>
  </nldd-app-view>

  <!-- Editor requires login: a heads-up popover anchored to the clicked Editor
       tab (sm/md/lg) so the SSO screen never appears unannounced. -->
  <nldd-popover ref="loginWarning" accessible-label="Inloggen" width="320px">
    <nldd-container padding="16">
      <nldd-inline-dialog
        icon="login"
        text="Log in om de editor te gebruiken"
        supporting-text="Zodra je bent ingelogd kies je een traject en kun je aan de slag."
      >
        <nldd-button slot="actions" variant="primary" text="Inloggen" @click="login(loginRedirect || editorTabHref)"></nldd-button>
        <nldd-button slot="actions" variant="secondary" text="Account aanvragen" :href="accountRequestHref" @click.prevent="goToAccountRequest"></nldd-button>
      </nldd-inline-dialog>
    </nldd-container>
  </nldd-popover>

  <!-- Enabling the GitHub-koppeling flag is a deployment-wide write-path
       switch, not a personal preference like its neighbours in the Functies
       list - confirm before every writer's saves start requiring a linked
       GitHub account (see onFunctieFlagSelect). -->
  <nldd-popover ref="enforcementConfirm" accessible-label="GitHub-koppeling inschakelen" width="360px" @close="onEnforcementConfirmClose">
    <nldd-container padding="16">
      <nldd-inline-dialog
        icon="exclamation-triangle"
        text="GitHub-koppeling voor iedereen inschakelen?"
        supporting-text="Dit geldt voor de hele omgeving, niet alleen voor jou: opslaan in een traject vereist daarna voor elke gebruiker een gekoppeld GitHub-account. Wie nog niet gekoppeld heeft, wordt bij de eerstvolgende opslag naar de koppel-flow geleid."
      >
        <nldd-button slot="actions" variant="primary" text="Inschakelen" @click="confirmUserOauthEnforcement"></nldd-button>
        <nldd-button slot="actions" text="Annuleren" @click="enforcementConfirm?.hide()"></nldd-button>
      </nldd-inline-dialog>
    </nldd-container>
  </nldd-popover>
</template>

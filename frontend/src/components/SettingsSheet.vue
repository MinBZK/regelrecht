<script setup>
// "Instellingen" — the account settings sheet, opened from the account menu.
//
// Collects what used to be submenus of that menu (Weergave, Functies) plus the
// GitHub link. The link is a connection, not an action: it belongs with the
// other account state rather than between Help and Log uit, where its label
// also changed identity depending on whether you were connected.
//
// Settings are not a form — nothing is submitted, every control applies at
// once — so these are list rows under section titles, not fields in a
// fieldset.
//
// The composables backing this are singletons, so the sheet reads them
// directly instead of taking the whole set as props.
import { computed, ref } from 'vue';
import { useAuth } from '../composables/useAuth.js';
import { useColorScheme } from '../composables/useColorScheme.js';
import { useFeatureFlags } from '../composables/useFeatureFlags.js';
import { useGithubAuth } from '../composables/useGithubAuth.js';

const sheetEl = ref(null);

function show() {
  sheetEl.value?.show?.();
}
function hide() {
  sheetEl.value?.hide?.();
}

defineExpose({ show, hide });

const { authenticated, loading: authLoading, hasRole } = useAuth();
const { colorScheme, setColorScheme } = useColorScheme();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();
const {
  status: githubStatus,
  connect: connectGithub,
  disconnect: disconnectGithub,
} = useGithubAuth();

const colorSchemeOptions = [
  ['auto', 'Systeem', 'display'],
  ['light', 'Licht', 'light-mode'],
  ['dark', 'Donker', 'dark-mode'],
];

const editorPanelFlags = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
  ['panel.notes', 'Notities'],
];

const signedIn = computed(() => !authLoading.value && authenticated.value);
// Feature flags are deployment-wide and the PUT is behind `editor-admin`
// (main.rs, "Admin routes"). Showing the switches to anyone else would give
// them controls that silently revert on the 403.
const isAdmin = computed(() => signedIn.value && hasRole('editor-admin'));
// `configured: false` means this deployment has no GitHub OAuth App wired up,
// so the whole section stays hidden.
const githubConfigured = computed(() => signedIn.value && !!githubStatus.value?.configured);
// Two different questions, deliberately not the same computed:
//   `required` — does the write path actually demand a personal token? The
//     backend folds the GITHUB_USER_TOKEN_REQUIRED env var AND the feature flag
//     into this one field, and the env var wins. Use it wherever the UI
//     describes behaviour.
//   `enforced` — is the feature flag on? That is the only half an admin can
//     change here, so the switch binds to this.
// They differ when the deployment forces it through config; then the switch
// would be a control that changes nothing, so it makes way for a plain line.
const githubRequired = computed(() => !!githubStatus.value?.required);
const githubEnforced = computed(() => isEnabled('github.user_oauth'));
const githubForcedByConfig = computed(() => githubRequired.value && !githubEnforced.value);
const githubConnected = computed(() => !!githubStatus.value?.connected);

function onPanelFlagChange(key, event) {
  // Follow the store rather than the switch's own DOM state.
  if (event.detail.checked !== isEnabled(key)) toggleFlag(key);
}

// Enabling `github.user_oauth` is not a personal display preference like the
// panel flags: the flag is deployment-wide AND doubles as the backend's
// write-enforcement switch (`write_requires_user_token`), so turning it on
// makes every editor-writer's next traject save require a linked personal
// GitHub account (an unlinked user's save 428s into the connect flow).
// Intercept the enable with an explicit confirmation. Disabling restores the
// pre-existing service-token behaviour and stays a plain toggle.
const enforcementConfirm = ref(null);
const enforcementSwitch = ref(null);

function onEnforcementChange(event) {
  if (!isEnabled('github.user_oauth')) {
    // Revert the switch: it flipped itself, and the flag only actually turns
    // on once the confirmation is accepted.
    event.target.checked = false;
    if (enforcementConfirm.value) {
      enforcementConfirm.value.anchorElement = enforcementSwitch.value || event.target;
      enforcementConfirm.value.show();
    }
    return;
  }
  toggleFlag('github.user_oauth');
}

function confirmUserOauthEnforcement() {
  enforcementConfirm.value?.hide();
  toggleFlag('github.user_oauth');
}

// Release the anchor when the popover closes (confirm, cancel, or light
// dismiss): nldd-popover toggles itself on every subsequent click on its
// anchor element, so a stale anchor makes the switch reopen it.
function onEnforcementConfirmClose() {
  if (enforcementConfirm.value) enforcementConfirm.value.anchorElement = null;
}

// Disconnecting revokes the token at GitHub and, with the flag on, makes the
// next traject save fail with a 428 — so it asks first instead of firing on
// the click.
const disconnectConfirm = ref(null);

function confirmDisconnect() {
  disconnectConfirm.value?.hide();
  disconnectGithub();
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="right" width="480px" full-height @close="hide">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Instellingen"
          dismiss-text="Sluit"
          @dismiss="hide"
        ></nldd-top-title-bar>

        <nldd-simple-section width="full">
          <nldd-title size="5">
            <h2>Weergave</h2>
          </nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-segmented-control
            variant="icon-and-text"
            width="full"
            accessible-label="Kleurschema"
            :value="colorScheme"
            @change="setColorScheme($event.detail.value)"
          >
            <nldd-segmented-control-item
              v-for="[value, label, icon] in colorSchemeOptions"
              :key="value"
              :value="value"
              :text="label"
              :icon="icon"
            ></nldd-segmented-control-item>
          </nldd-segmented-control>

          <template v-if="githubConfigured && githubRequired">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="5">
              <h2>Koppelingen</h2>
            </nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list variant="box">
              <nldd-list-item>
                <nldd-text-cell
                  text="GitHub"
                  :supporting-text="githubConnected ? githubStatus.github_login : 'Niet gekoppeld'"
                ></nldd-text-cell>
                <nldd-cell>
                  <nldd-button
                    v-if="githubConnected"
                    variant="destructive"
                    text="Ontkoppelen"
                    @click="disconnectConfirm?.show()"
                  ></nldd-button>
                  <nldd-button
                    v-else
                    variant="primary"
                    text="Koppelen"
                    end-icon="external-link"
                    @click="connectGithub()"
                  ></nldd-button>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <template v-if="isAdmin">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="5">
              <h2>Beheer</h2>
              <span slot="subtitle">Geldt voor alle gebruikers en trajecten in deze installatie.</span>
            </nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list variant="box">
              <nldd-list-item v-for="[key, label] in editorPanelFlags" :key="key">
                <nldd-text-cell :text="label"></nldd-text-cell>
                <nldd-cell>
                  <nldd-switch
                    :accessible-label="label"
                    :checked="isEnabled(key) || undefined"
                    @change="onPanelFlagChange(key, $event)"
                  ></nldd-switch>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item v-if="githubConfigured">
                <nldd-text-cell
                  text="Met eigen GitHub-account schrijven"
                  :supporting-text="githubForcedByConfig
                    ? 'Aan, vastgezet in de configuratie van deze omgeving.'
                    : 'Uit: RegelRecht schrijft met één gedeeld account.'"
                ></nldd-text-cell>
                <nldd-cell v-if="!githubForcedByConfig">
                  <nldd-switch
                    ref="enforcementSwitch"
                    accessible-label="Met eigen GitHub-account schrijven"
                    :checked="githubEnforced || undefined"
                    @change="onEnforcementChange"
                  ></nldd-switch>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>

    <nldd-popover
      ref="enforcementConfirm"
      accessible-label="GitHub-koppeling inschakelen"
      width="360px"
      @close="onEnforcementConfirmClose"
    >
      <nldd-container padding="16">
        <nldd-inline-dialog
          icon="exclamation-triangle"
          text="GitHub-koppeling voor iedereen inschakelen?"
          supporting-text="Dit geldt voor de hele omgeving, niet alleen voor jou: opslaan in een traject vereist daarna voor elke gebruiker een gekoppeld GitHub-account. Wie nog niet gekoppeld heeft, wordt bij de eerstvolgende opslag naar de koppel-flow geleid."
        >
          <nldd-button
            slot="actions"
            variant="primary"
            text="Inschakelen"
            @click="confirmUserOauthEnforcement"
          ></nldd-button>
          <nldd-button
            slot="actions"
            text="Annuleren"
            @click="enforcementConfirm?.hide()"
          ></nldd-button>
        </nldd-inline-dialog>
      </nldd-container>
    </nldd-popover>

    <nldd-modal-dialog
      ref="disconnectConfirm"
      variant="alert"
      icon="exclamation-triangle"
      text="GitHub-account ontkoppelen?"
      supporting-text="Het token wordt ingetrokken bij GitHub. Zolang schrijven met een eigen account vereist is, mislukt je eerstvolgende opslag in een traject tot je opnieuw koppelt."
    >
      <nldd-button
        slot="actions"
        variant="primary"
        text="Behoud koppeling"
        @click="disconnectConfirm?.hide()"
      ></nldd-button>
      <nldd-button
        slot="actions"
        variant="destructive"
        text="Ontkoppel"
        @click="confirmDisconnect"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>

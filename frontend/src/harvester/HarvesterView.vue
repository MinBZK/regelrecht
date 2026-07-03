<script setup>
/**
 * HarvesterView — the harvester-admin "Corpusinwinning" section, merged into the editor.
 *
 * Top-level route (sibling of AppShell, not nested) so it carries its own
 * compact chrome instead of the editor's app chrome — mirroring the other
 * standalone views (WerkdocumentenView, TrajectChooserView). It hosts the two
 * sub-screens (Law Entries / Jobs) via a nested <router-view>, matching the
 * original standalone admin dashboard's App shell.
 *
 * Reads/writes go to `/api/harvest-admin/*`, which editor-api proxies to the
 * standalone harvester-admin service (forwarding the session cookie). Access is
 * gated at the route (meta.requiresRole) and the menu item; write actions are
 * enforced server-side by the harvester-admin API.
 */
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useAuth } from '../composables/useAuth.js';
import { useColorScheme } from '../composables/useColorScheme.js';
import { usePlatformInfo } from './composables/usePlatformInfo.js';
import { useNewHarvestJob } from './composables/useNewHarvestJob.js';
import NewHarvestJobSheet from './components/NewHarvestJobSheet.vue';
import './harvester.css';

const route = useRoute();
const router = useRouter();
const { authenticated, oidcConfigured, person, loading: authLoading, login, logout } = useAuth();
const { colorScheme, setColorScheme } = useColorScheme();
const { info } = usePlatformInfo();
const { open: openNewHarvestJob } = useNewHarvestJob();

const colorSchemeOptions = [
  ['auto', 'Systeem'],
  ['light', 'Licht'],
  ['dark', 'Donker'],
];

// Show the deployment name (e.g. a prN preview) as a badge, but not for the
// canonical production deployment.
const deploymentName = computed(() =>
  info.value?.deployment_name && info.value.deployment_name !== 'regelrecht'
    ? info.value.deployment_name
    : null,
);

const tabs = [
  { key: 'law-entries', label: 'Wetten', route: '/harvesting/law-entries' },
  { key: 'jobs', label: 'Taken', route: '/harvesting/jobs' },
];
const activeTab = computed(() => route.name);

function goToLibrary() {
  router.push('/library');
}
</script>

<template>
  <nldd-app-view>
    <div class="harvesting-view">
      <nldd-container padding="8" padding-left="16">
        <nldd-toolbar size="md">
          <nldd-toolbar-item slot="start">
            <nldd-icon-button
              icon="arrow-left"
              text="Terug naar bibliotheek"
              tooltip-timing="never"
              variant="neutral-tinted"
              @click="goToLibrary"
            ></nldd-icon-button>
          </nldd-toolbar-item>
          <nldd-toolbar-title
            slot="start"
            text="Corpusinwinning"
            supporting-text="RegelRecht"
            min-width="fit-content"
          ></nldd-toolbar-title>
          <nldd-toolbar-item slot="center">
            <nldd-tab-bar>
              <nldd-tab-bar-item
                v-for="tab in tabs"
                :key="tab.key"
                :text="tab.label"
                :selected="activeTab === tab.key ? '' : undefined"
                @click="router.push(tab.route)"
              ></nldd-tab-bar-item>
            </nldd-tab-bar>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-tag
              v-if="deploymentName"
              color="warning"
              size="sm"
              :text="deploymentName"
            ></nldd-tag>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="plus-small"
              text="Nieuwe harvest-job"
              tooltip-timing="never"
              variant="neutral-tinted"
              @click="openNewHarvestJob"
            ></nldd-icon-button>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              id="harvesting-account-btn"
              size="md"
              icon="account"
              text="Account"
              tooltip-timing="never"
              expandable
              popovertarget="harvesting-account-menu"
            ></nldd-icon-button>
            <nldd-menu id="harvesting-account-menu" anchor="harvesting-account-btn">
              <nldd-menu-item
                v-if="!authLoading && authenticated"
                :text="person?.name || person?.email"
                disabled
              ></nldd-menu-item>
              <nldd-menu-group text="Thema">
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
              </nldd-menu-group>
              <nldd-menu-divider></nldd-menu-divider>
              <nldd-menu-item
                v-if="!authLoading && authenticated"
                text="Uitloggen"
                icon="logout"
                @click="logout"
              ></nldd-menu-item>
              <nldd-menu-item
                v-else-if="!authLoading && oidcConfigured"
                text="Inloggen"
                icon="login"
                @click="login()"
              ></nldd-menu-item>
            </nldd-menu>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>
      <nldd-divider></nldd-divider>

      <nldd-page>
        <router-view></router-view>
      </nldd-page>
    </div>
    <NewHarvestJobSheet />
  </nldd-app-view>
</template>

<script setup>
import { computed, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useAuth } from './composables/useAuth.js';
import { usePlatformInfo } from './composables/usePlatformInfo.js';
import { useNewHarvestJob } from './composables/useNewHarvestJob.js';
import { useColorScheme } from '@regelrecht/frontend-shared';
import NewHarvestJobSheet from './components/NewHarvestJobSheet.vue';

const route = useRoute();
const router = useRouter();
const { authenticated, oidcConfigured, loading: authLoading, logout, login } = useAuth();
const { info } = usePlatformInfo();
const { open: openNewHarvestJob } = useNewHarvestJob();
const { colorScheme, setColorScheme } = useColorScheme();

const themeOptions = [
  ['auto', 'System'],
  ['dark', 'Dark'],
  ['light', 'Light'],
];

const deploymentName = computed(() =>
  info.value?.deployment_name && info.value.deployment_name !== 'regelrecht'
    ? info.value.deployment_name
    : null,
);

const tabs = [
  { key: 'law-entries', label: 'Law Entries', route: '/law-entries' },
  { key: 'jobs', label: 'Jobs', route: '/jobs' },
];

const activeTab = computed(() => route.name);

// Redirect to login if OIDC configured but not authenticated
watch([authLoading, oidcConfigured, authenticated], ([loading, oidc, auth]) => {
  if (!loading && oidc && !auth) {
    login();
  }
});
</script>

<template>
  <div v-if="authLoading" />
  <template v-else>
    <nldd-tag
      v-if="deploymentName"
      class="env-badge"
      color="warning"
      size="sm"
      :text="deploymentName"
    />
    <nldd-bar-split-view>
      <nldd-container slot="toolbar" padding="8" padding-left="16">
        <nldd-toolbar size="md">
          <nldd-toolbar-title
            slot="start"
            text="Admin"
            supporting-text="RegelRecht"
            min-width="fit-content"
          />
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
            <nldd-icon-button
              icon="plus-small"
              text="Add"
              tooltip-timing="never"
              variant="neutral-tinted"
              @click="openNewHarvestJob"
            />
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              id="account-menu-trigger"
              icon="person-circle"
              text="Account"
              expandable
              tooltip-timing="never"
              variant="neutral-tinted"
            />
            <nldd-menu anchor="account-menu-trigger">
              <nldd-menu-group text="Theme">
                <nldd-menu-item
                  v-for="[value, label] in themeOptions"
                  :key="value"
                  type="radio"
                  :text="label"
                  :selected="colorScheme === value || undefined"
                  @click.stop="setColorScheme(value)"
                />
              </nldd-menu-group>
              <nldd-menu-divider />
              <nldd-menu-item
                text="Logout"
                icon="logout"
                @click.stop="logout"
              />
            </nldd-menu>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>
      <nldd-page slot="main">
        <router-view />
      </nldd-page>
    </nldd-bar-split-view>
    <NewHarvestJobSheet />
  </template>
</template>

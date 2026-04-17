<script setup>
import { computed, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useAuth } from './composables/useAuth.js';
import { usePlatformInfo } from './composables/usePlatformInfo.js';

const route = useRoute();
const router = useRouter();
const { authenticated, person, oidcConfigured, loading: authLoading, logout, redirectToLogin } = useAuth();
const { info } = usePlatformInfo();

const deploymentName = computed(() =>
  info.value?.deployment_name && info.value.deployment_name !== 'regelrecht'
    ? info.value.deployment_name
    : null,
);

const accountLabel = computed(() =>
  person.value?.name || person.value?.email || 'Admin',
);

const tabs = [
  { key: 'law-entries', label: 'Law Entries', route: '/law-entries' },
  { key: 'jobs', label: 'Jobs', route: '/jobs' },
];

const activeTab = computed(() => route.name);

// Redirect to OIDC login if configured but not authenticated.
// The ?test-sso param is handled in main.js before Vue loads.
watch(authLoading, (loading) => {
  if (loading || authenticated.value) return;
  if (oidcConfigured.value) {
    redirectToLogin();
  }
});

function onAccountClick() {
  logout();
}
</script>

<template>
  <div v-if="authLoading" />
  <template v-else>
    <span v-if="deploymentName" class="env-badge">{{ deploymentName }}</span>
    <ndd-app-view>
      <ndd-bar-split-view>
        <ndd-page slot="toolbar">
          <ndd-top-navigation-bar
            title="RegelRecht admin"
            no-logo
            no-menu
            utility-no-language-switch
            :utility-account-label="accountLabel"
            @account-click="onAccountClick"
          />
          <ndd-container padding="8">
            <ndd-toolbar size="md">
              <ndd-toolbar-item slot="start">
                <ndd-tab-bar>
                  <ndd-tab-bar-item
                    v-for="tab in tabs"
                    :key="tab.key"
                    :text="tab.label"
                    :selected="activeTab === tab.key ? '' : undefined"
                    @click="router.push(tab.route)"
                  ></ndd-tab-bar-item>
                </ndd-tab-bar>
              </ndd-toolbar-item>
              <ndd-toolbar-item id="view-toggle-target" slot="end" />
            </ndd-toolbar>
          </ndd-container>
        </ndd-page>
        <ndd-page slot="main">
          <router-view />
        </ndd-page>
      </ndd-bar-split-view>
    </ndd-app-view>
  </template>
</template>

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

// Redirect to login if OIDC configured but not authenticated
watch([authLoading, oidcConfigured, authenticated], ([loading, oidc, auth]) => {
  if (!loading && oidc && !auth) {
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
    <ndd-page sticky-header>
      <div slot="header">
        <ndd-top-navigation-bar
          title="RegelRecht admin"
          no-logo
          no-menu
          utility-no-language-switch
          :utility-account-label="accountLabel"
          @account-click="onAccountClick"
        />
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
          <ndd-toolbar-item id="pagination-target" slot="end" />
        </ndd-toolbar>
      </div>
      <router-view />
    </ndd-page>
  </template>
</template>

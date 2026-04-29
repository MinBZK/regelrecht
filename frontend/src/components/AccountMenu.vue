<script setup>
import { useAuth } from '../composables/useAuth.js';
import { useFeatureFlags } from '../composables/useFeatureFlags.js';
import { useUserSettings } from '../composables/useUserSettings.js';

const { authenticated, loading: authLoading, oidcConfigured, person, login, logout } = useAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();
const { theme, toggleTheme } = useUserSettings();

const editorPanelFlags = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
  ['panel.law_graph', 'Wettengraaf'],
];
</script>

<template>
  <nldd-icon-button
    id="account-menu-btn"
    size="md"
    icon="person-circle"
    expandable
    :title="person?.name || 'Account'"
    popovertarget="account-menu"
  ></nldd-icon-button>
  <nldd-menu id="account-menu" anchor="account-menu-btn">
    <template v-if="!authLoading && authenticated">
      <nldd-menu-item :text="person?.name || person?.email" disabled></nldd-menu-item>
      <nldd-menu-divider></nldd-menu-divider>
    </template>
    <nldd-menu-item
      v-for="[key, label] in editorPanelFlags"
      :key="key"
      type="checkbox"
      :selected="isEnabled(key) || undefined"
      :text="label"
      @select="toggleFlag(key)"
    ></nldd-menu-item>
    <nldd-menu-item
      type="checkbox"
      :selected="theme === 'dark' || undefined"
      text="Donkere modus"
      @select="toggleTheme"
    ></nldd-menu-item>
    <nldd-menu-divider></nldd-menu-divider>
    <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
    <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login"></nldd-menu-item>
  </nldd-menu>
</template>

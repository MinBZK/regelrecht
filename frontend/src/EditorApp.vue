<script setup>
import { ref, computed, watch } from 'vue';
import yaml from 'js-yaml';
import { useLaw, fetchLaw } from './composables/useLaw.js';
import { useEngine } from './composables/useEngine.js';
import ArticleText from './components/ArticleText.vue';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import ExecutionTraceView from './components/ExecutionTraceView.vue';

// --- Initial law load (from URL params) ---
const { law, lawId, rawYaml, articles, lawName, selectedArticle, selectedArticleNumber, switchLaw, loading, error } = useLaw();

const middlePaneView = ref('form');

// --- Multi-law tab state (persisted in localStorage) ---
const TABS_STORAGE_KEY = 'regelrecht-open-tabs';

function loadSavedTabs() {
  try {
    const saved = localStorage.getItem(TABS_STORAGE_KEY);
    return saved ? JSON.parse(saved) : [];
  } catch { return []; }
}

function saveTabs(tabs) {
  localStorage.setItem(TABS_STORAGE_KEY, JSON.stringify(tabs));
}

const openTabs = ref(loadSavedTabs());

// Cache for law names (populated on fetch)
const lawNames = ref({});

// Active tab tracks which tab is selected
const activeTab = ref(null);

function tabKey(tab) {
  return `${tab.lawId}:${tab.articleNumber}`;
}

function findTab(lawIdVal, articleNumber) {
  return openTabs.value.find(t => t.lawId === lawIdVal && t.articleNumber === String(articleNumber));
}

// Add tab when initial law loads
watch([() => lawId.value, selectedArticle], ([id, article]) => {
  if (!id || !article) return;
  const num = String(article.number);
  if (!findTab(id, num)) {
    const MAX_TABS = 20;
    const tabs = [...openTabs.value, { lawId: id, articleNumber: num }];
    openTabs.value = tabs.length > MAX_TABS ? tabs.slice(-MAX_TABS) : tabs;
    saveTabs(openTabs.value);
  }
  activeTab.value = { lawId: id, articleNumber: num };
  if (lawName.value) lawNames.value = { ...lawNames.value, [id]: lawName.value };
});

// Also populate lawNames when lawName resolves
watch(lawName, (name) => {
  if (name && lawId.value) {
    lawNames.value = { ...lawNames.value, [lawId.value]: name };
  }
});

async function selectTab(tab) {
  activeTab.value = tab;
  if (tab.lawId === lawId.value) {
    selectedArticleNumber.value = tab.articleNumber;
  } else {
    await switchLaw(tab.lawId, tab.articleNumber);
    lawNames.value = { ...lawNames.value, [tab.lawId]: lawName.value };
  }
}

function closeTab(tab) {
  openTabs.value = openTabs.value.filter(t => tabKey(t) !== tabKey(tab));
  saveTabs(openTabs.value);
  if (activeTab.value && tabKey(activeTab.value) === tabKey(tab)) {
    const remaining = openTabs.value;
    if (remaining.length > 0) {
      selectTab(remaining[remaining.length - 1]).catch(console.warn);
    } else {
      activeTab.value = null;
    }
  }
}

function tabDisplayName(tab) {
  return lawNames.value[tab.lawId] || tab.lawId;
}

// Load lawNames for persisted tabs on startup
(async () => {
  for (const tab of openTabs.value) {
    if (!lawNames.value[tab.lawId]) {
      try {
        const entry = await fetchLaw(tab.lawId);
        lawNames.value = { ...lawNames.value, [tab.lawId]: entry.lawName };
      } catch { /* ignore */ }
    }
  }
})();

function onMiddlePaneChange(event) {
  const value = event.target?.value ?? event.detail?.[0];
  if (value) middlePaneView.value = value;
}

// --- Engine ---
const { ready: engineReady, initError: engineInitError, initEngine, getEngine } = useEngine();
initEngine().catch(() => {});

// Load current law into engine when YAML is available
watch(
  [() => rawYaml.value, engineReady],
  ([lawYaml, isReady]) => {
    if (!isReady || !lawYaml) return;
    const engine = getEngine();
    try {
      if (engine.hasLaw(lawId.value)) {
        engine.unloadLaw(lawId.value);
      }
      engine.loadLaw(lawYaml);
    } catch (e) {
      console.warn(`Failed to load law '${lawId.value}' into engine:`, e);
    }
  },
  { immediate: true },
);

// --- Trace state (receives trace from last executed scenario) ---
const lastTraceText = ref(null);
const lastResult = ref(null);
const lastError = ref(null);
const lastExpectations = ref({});

function handleScenarioExecuted({ result, traceText, error, expectations }) {
  lastResult.value = result;
  lastTraceText.value = traceText;
  lastError.value = error || null;
  lastExpectations.value = expectations || {};
}

// --- Editor state ---
const activeAction = ref(null);
const activeEditItem = ref(null);
const parseError = ref(null);

const machineReadable = ref(null);
const yamlSource = ref('');

const dumpOpts = { lineWidth: 80, noRefs: true };

watch(selectedArticle, (article) => {
  activeAction.value = null;
  activeEditItem.value = null;
  const mr = article?.machine_readable;
  machineReadable.value = mr ? JSON.parse(JSON.stringify(mr)) : null;
  yamlSource.value = mr ? yaml.dump(mr, dumpOpts) : '';
  parseError.value = null;
}, { immediate: true });

const editedArticle = computed(() => {
  if (!selectedArticle.value) return null;
  return { ...selectedArticle.value, machine_readable: machineReadable.value };
});

function onYamlInput(event) {
  const text = event.target.value;
  yamlSource.value = text;
  try {
    const parsed = yaml.load(text);
    machineReadable.value = parsed != null && typeof parsed === 'object' ? parsed : null;
    parseError.value = null;
  } catch (e) {
    parseError.value = e.message;
  }
}

function handleSave({ section, key, newKey, index, data }) {
  const mr = machineReadable.value
    ? JSON.parse(JSON.stringify(machineReadable.value))
    : {};

  if (!mr.definitions) mr.definitions = {};
  if (!mr.execution) mr.execution = {};
  if (!mr.execution.parameters) mr.execution.parameters = [];
  if (!mr.execution.input) mr.execution.input = [];
  if (!mr.execution.output) mr.execution.output = [];

  if (section === 'definition') {
    if (newKey && newKey !== key) delete mr.definitions[key];
    mr.definitions[newKey || key] = data;
  } else if (section === 'add-definition') {
    mr.definitions[key] = data;
  } else if (section === 'parameter') {
    mr.execution.parameters[index] = data;
  } else if (section === 'add-parameter') {
    mr.execution.parameters.push(data);
  } else if (section === 'input') {
    mr.execution.input[index] = data;
  } else if (section === 'add-input') {
    mr.execution.input.push(data);
  } else if (section === 'output') {
    mr.execution.output[index] = data;
  } else if (section === 'add-output') {
    mr.execution.output.push(data);
  }

  machineReadable.value = mr;
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}
</script>

<template>
  <ndd-app-view>
    <ndd-bar-split-view>
      <!-- Primary Bar: App Toolbar + Document Tabs -->
      <ndd-split-view-pane slot="primary-bar">
      <ndd-container padding="8">
          <ndd-toolbar size="md">
            <ndd-toolbar-item slot="start">
              <ndd-tab-bar size="md">
                <ndd-tab-bar-item href="/library" text="Bibliotheek"></ndd-tab-bar-item>
                <ndd-tab-bar-item selected text="Editor"></ndd-tab-bar-item>
              </ndd-tab-bar>
            </ndd-toolbar-item>
            <ndd-toolbar-item slot="center" min-width="240px" width="40%">
              <ndd-search-field
                size="md"
                placeholder="Zoeken"
                @input="search = $event.target.value"
              ></ndd-search-field>
            </ndd-toolbar-item>
            <ndd-toolbar-item slot="end">
              <ndd-button-bar size="md">
                <ndd-button id="project-menu-btn" size="md" expandable text="RR Project" popovertarget="project-menu"></ndd-button>
                <ndd-menu id="project-menu" anchor="project-menu-btn">
                  <ndd-menu-item text="Instellingen"></ndd-menu-item>
                  <ndd-menu-item text="Leden"></ndd-menu-item>
                  <ndd-menu-divider></ndd-menu-divider>
                  <ndd-menu-item text="Nieuw project"></ndd-menu-item>
                </ndd-menu>
                <ndd-button-bar-divider></ndd-button-bar-divider>
                <ndd-icon-button id="account-menu-btn" size="md" icon="person-circle" expandable title="Account" popovertarget="account-menu">
                </ndd-icon-button>
                <ndd-menu id="account-menu" anchor="account-menu-btn">
                  <ndd-menu-item text="Profiel"></ndd-menu-item>
                  <ndd-menu-item text="Voorkeuren"></ndd-menu-item>
                  <ndd-menu-divider></ndd-menu-divider>
                  <ndd-menu-item text="Uitloggen"></ndd-menu-item>
                </ndd-menu>
              </ndd-button-bar>
            </ndd-toolbar-item>
          </ndd-toolbar>

          <ndd-spacer size="8"></ndd-spacer>

          <!-- Document Tab Bar -->
          <ndd-document-tab-bar v-if="!loading && !error && openTabs.length > 0">
            <ndd-document-tab-bar-item
              v-for="tab in openTabs"
              :key="tabKey(tab)"
              :text="`Artikel ${tab.articleNumber}`"
              :supporting-text="tabDisplayName(tab)"
              :short-text="`Art. ${tab.articleNumber}`"
              :short-supporting-text="tabDisplayName(tab)"
              :selected="activeTab && tabKey(activeTab) === tabKey(tab) || undefined"
              has-dismiss-button
              @click="selectTab(tab)"
              @dismiss="closeTab(tab)"
            >
            </ndd-document-tab-bar-item>
          </ndd-document-tab-bar>
        </ndd-container>
      </ndd-split-view-pane>

      <!-- Main content area -->
      <ndd-split-view-pane slot="main">
        <!-- Error state -->
        <ndd-page v-if="error">
          <ndd-simple-section align="center">
            <ndd-inline-dialog variant="alert" text="Kon de wet niet laden" :supporting-text="error.message"></ndd-inline-dialog>
          </ndd-simple-section>
        </ndd-page>

        <!-- 3-column equal layout: Text | Form | Result -->
        <ndd-side-by-side-split-view v-else panes="3">
          <!-- Left: Article Text -->
          <ndd-split-view-pane slot="pane-1" background="tinted">
            <ndd-page sticky-header>
              <ndd-top-title-bar slot="header" text="Tekst"></ndd-top-title-bar>
              <ArticleText :article="selectedArticle" />
            </ndd-page>
          </ndd-split-view-pane>

          <!-- Middle: Form or YAML -->
          <ndd-split-view-pane slot="pane-2">
            <ndd-page sticky-header>
              <ndd-top-title-bar slot="header" text="Scenario's">
                <ndd-segmented-control slot="toolbar" size="md" :value="middlePaneView" @change="onMiddlePaneChange">
                  <ndd-segmented-control-item value="form" text="Scenario's"></ndd-segmented-control-item>
                  <ndd-segmented-control-item value="yaml" text="YAML"></ndd-segmented-control-item>
                </ndd-segmented-control>
                <span v-if="middlePaneView === 'yaml' && parseError" slot="toolbar" class="editor-parse-error">YAML parse error</span>
              </ndd-top-title-bar>

              <!-- Form view: engine error -->
              <ndd-simple-section v-if="middlePaneView === 'form' && engineInitError" align="center">
                <ndd-inline-dialog variant="alert" text="WASM engine niet geladen" :supporting-text="`${engineInitError.message} — voer 'just wasm-build' uit om de WASM module te bouwen.`"></ndd-inline-dialog>
              </ndd-simple-section>

              <!-- Form view: scenario builder -->
              <ScenarioBuilder
                v-else-if="middlePaneView === 'form'"
                :law-id="lawId"
                :law-yaml="rawYaml"
                :engine="getEngine()"
                :ready="engineReady"
                :articles="articles"
                @executed="handleScenarioExecuted"
              />

              <!-- YAML view -->
              <ndd-simple-section v-if="middlePaneView === 'yaml'">
                <div class="editor-yaml-wrap">
                  <textarea
                    :value="yamlSource"
                    @input="onYamlInput"
                    class="editor-yaml-textarea"
                    spellcheck="false"
                    autocomplete="off"
                    autocorrect="off"
                    autocapitalize="off"
                  ></textarea>
                  <div v-if="parseError" class="editor-parse-error-detail">{{ parseError }}</div>
                </div>
              </ndd-simple-section>
            </ndd-page>
          </ndd-split-view-pane>

          <!-- Right: Execution Result -->
          <ndd-split-view-pane slot="pane-3">
            <ndd-page sticky-header>
              <ndd-top-title-bar slot="header" text="Resultaat"></ndd-top-title-bar>

              <ExecutionTraceView
                :result="lastResult"
                :trace-text="lastTraceText"
                :error="lastError"
                :expectations="lastExpectations"
              />
            </ndd-page>
          </ndd-split-view-pane>
        </ndd-side-by-side-split-view>
      </ndd-split-view-pane>
    </ndd-bar-split-view>
  </ndd-app-view>

  <ActionSheet :action="activeAction" :article="editedArticle" @close="activeAction = null" />
  <EditSheet :item="activeEditItem" @save="handleSave" @close="activeEditItem = null" />
</template>

<style>
.editor-engine-error {
  padding: 12px 16px;
  background: #fee;
  color: #c00;
  font-size: 13px;
}

.editor-engine-error-hint {
  margin-top: 4px;
  font-size: 12px;
  color: #999;
}

.editor-engine-error-hint code {
  background: #eee;
  padding: 1px 4px;
  border-radius: 3px;
}

.editor-yaml-wrap {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.editor-yaml-textarea {
  flex: 1;
  width: 100%;
  min-height: 0;
  height: calc(100vh - 160px);
  background: #1e1e2e;
  color: #cdd6f4;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', 'JetBrains Mono', monospace;
  font-size: 13px;
  line-height: 1.6;
  padding: 16px;
  border: none;
  outline: none;
  resize: none;
  tab-size: 2;
  white-space: pre;
  overflow: auto;
}

.editor-yaml-textarea::selection {
  background: #45475a;
}

.editor-parse-error {
  font-size: 12px;
  font-weight: 600;
  color: #c00;
  background: #fee;
  padding: 2px 8px;
  border-radius: 6px;
}

.editor-parse-error-detail {
  background: #2a1a1a;
  color: #f38ba8;
  font-family: 'SF Mono', monospace;
  font-size: 12px;
  padding: 8px 16px;
  border-top: 1px solid #45475a;
}
</style>

<script setup>
import { ref, computed, watch } from 'vue';
import yaml from 'js-yaml';
import { useLaw, fetchLaw } from './composables/useLaw.js';
import { useEngine } from './composables/useEngine.js';
import { useAuth } from './composables/useAuth.js';
import ArticleText from './components/ArticleText.vue';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import MachineReadable from './components/MachineReadable.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import ExecutionTraceView from './components/ExecutionTraceView.vue';

const { authenticated, loading: authLoading, oidcConfigured, person, logout } = useAuth();

// Redirect to login when OIDC is configured but user is not authenticated.
watch([authLoading, oidcConfigured, authenticated], ([isLoading, oidc, authed]) => {
  if (!isLoading && oidc && !authed) {
    window.location.href = '/auth/login';
  }
});

// All edit operations are gated behind SSO. When OIDC is configured the user
// must be authenticated; when OIDC is disabled the editor is fully open.
const canEdit = computed(() => !oidcConfigured.value || authenticated.value);

// --- Initial law load (from URL params) ---
const { law, lawId, rawYaml, articles, lawName, selectedArticle, selectedArticleNumber, switchLaw, loading, error } = useLaw();

const middlePaneView = ref('form');
const rightPaneView = ref('result');

const middlePaneTitle = computed(() => middlePaneView.value === 'yaml' ? 'YAML' : 'Scenario\u2019s');
const rightPaneTitle = computed(() => rightPaneView.value === 'machine' ? 'Machine' : 'Resultaat');

function onRightPaneChange(event) {
  const value = event.target?.value ?? event.detail?.[0];
  if (value) rightPaneView.value = value;
}

// --- Multi-law tab state (persisted in localStorage) ---
const TABS_STORAGE_KEY = 'regelrecht-open-tabs';

function loadSavedTabs() {
  try {
    const saved = localStorage.getItem(TABS_STORAGE_KEY);
    const parsed = saved ? JSON.parse(saved) : [];
    return Array.isArray(parsed) ? parsed : [];
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

let switchGeneration = 0;

async function selectTab(tab) {
  const gen = ++switchGeneration;
  activeTab.value = tab;
  // Restore snapshot if the user is mid-edit, otherwise the partial mutations
  // would persist into the new tab's view.
  if (activeAction.value) {
    handleActionClose();
  }
  if (tab.lawId === lawId.value) {
    selectedArticleNumber.value = tab.articleNumber;
  } else {
    await switchLaw(tab.lawId, tab.articleNumber);
    if (gen !== switchGeneration) return; // stale, another switch started
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

// Load lawNames for persisted tabs on startup (parallel, deduplicated)
const uniqueLawIds = [...new Set(openTabs.value.map(t => t.lawId))];
Promise.all(uniqueLawIds.map(async (id) => {
  try {
    const entry = await fetchLaw(id);
    lawNames.value = { ...lawNames.value, [id]: entry.lawName };
  } catch { /* ignore */ }
}));

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

// Initialize empty machine_readable scaffold
function handleInitMr() {
  machineReadable.value = {
    definitions: {},
    execution: {
      parameters: [],
      input: [],
      output: [],
      actions: [],
    },
  };
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}

// Add a new action and open ActionSheet
let actionSnapshot = null;

function handleAddAction() {
  // Snapshot BEFORE any mutations so cancel restores the exact original state
  actionSnapshot = JSON.stringify(machineReadable.value);
  const mr = machineReadable.value || {};
  if (!mr.execution) mr.execution = {};
  if (!mr.execution.actions) mr.execution.actions = [];
  const newAction = { output: '', value: '' };
  mr.execution.actions.push(newAction);
  machineReadable.value = { ...mr };
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
  activeAction.value = newAction;
}

function handleOpenAction(action) {
  actionSnapshot = JSON.stringify(machineReadable.value);
  activeAction.value = action;
  // Clear any stale parse error from a previous failed save
  parseError.value = null;
}

// Restore model from snapshot when ActionSheet is cancelled
function handleActionClose() {
  if (actionSnapshot) {
    machineReadable.value = JSON.parse(actionSnapshot);
    yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
    actionSnapshot = null;
  }
  activeAction.value = null;
  // Clear any stale parse error from a failed save attempt
  parseError.value = null;
}

const COMPARISON_OPS_SET = new Set([
  'EQUALS', 'NOT_EQUALS', 'GREATER_THAN', 'GREATER_THAN_OR_EQUAL',
  'LESS_THAN', 'LESS_THAN_OR_EQUAL', 'NOT_NULL', 'IN', 'NOT_IN',
]);

// Walk a value tree and report the first incomplete operation (e.g. a stub
// `{ operation: 'ADD', values: [] }` that the user inserted via "Voeg operatie
// toe" but never filled in). Returns null when the tree is structurally valid.
function findIncompleteOperation(value) {
  if (value == null || typeof value !== 'object') return null;
  if (!value.operation) return null;
  const op = value.operation;
  // Arithmetic / logical ops need a non-empty values or conditions array
  if (Array.isArray(value.values) && value.values.length === 0) return op;
  if (Array.isArray(value.conditions) && value.conditions.length === 0) return op;
  // IF/SWITCH need at least one case
  if ((op === 'IF' || op === 'SWITCH') && (!Array.isArray(value.cases) || value.cases.length === 0)) return op;
  // Comparison ops need a non-empty subject (and value, except for NOT_NULL).
  // changeOperationType / addNestedOperation seed these as empty strings, so
  // we must reject the stub before persisting. IN/NOT_IN accept either a
  // variable reference (e.g. "$list") or a literal non-empty array; both
  // are non-empty by the same value !== '' / array.length > 0 check.
  if (COMPARISON_OPS_SET.has(op)) {
    if ((value.subject ?? '') === '') return op;
    if (op !== 'NOT_NULL') {
      const v = value.value;
      if (v == null || v === '') return op;
      if (Array.isArray(v) && v.length === 0) return op;
    }
  }
  // NOT wraps a single value/operation; reject the empty-string stub created
  // when transitioning from arithmetic ops via changeOperationType.
  if (op === 'NOT' && (value.value ?? '') === '') return op;
  // Recurse into structural slots
  for (const child of [value.subject, value.value, value.default]) {
    const inner = findIncompleteOperation(child);
    if (inner) return inner;
  }
  if (Array.isArray(value.values)) {
    for (const v of value.values) {
      const inner = findIncompleteOperation(v);
      if (inner) return inner;
    }
  }
  if (Array.isArray(value.conditions)) {
    for (const c of value.conditions) {
      const inner = findIncompleteOperation(c);
      if (inner) return inner;
    }
  }
  if (Array.isArray(value.cases)) {
    for (const c of value.cases) {
      const inner = findIncompleteOperation(c?.when) || findIncompleteOperation(c?.then);
      if (inner) return inner;
    }
  }
  return null;
}

// Sync YAML when ActionSheet saves (mutations happened in-place)
function handleActionSave() {
  const action = activeAction.value;
  if (action) {
    // Output is required by the schema and the engine cannot load a law
    // with an action that has an empty output name.
    if (action.output == null || String(action.output).trim() === '') {
      parseError.value = 'Output mag niet leeg zijn';
      return;
    }
    // Reject incomplete nested operations (e.g. ADD with empty values[]) that
    // the user inserted via "Voeg operatie toe" but never filled in.
    // Note: a literal empty-string `value` is permitted at this layer — the
    // schema validator on save handles type-specific validation; rejecting it
    // here would block the legitimate "set output now, fill value via YAML
    // pane later" workflow used by the test suite and the editor's manual
    // YAML escape hatch.
    const incomplete = findIncompleteOperation(action.value);
    if (incomplete) {
      parseError.value = `Operatie '${incomplete}' is nog niet ingevuld`;
      return;
    }
  }
  actionSnapshot = null;
  activeAction.value = null;
  // Re-assign to trigger reactivity + re-dump YAML
  machineReadable.value = JSON.parse(JSON.stringify(machineReadable.value));
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
                <ndd-icon-button id="account-menu-btn" size="md" icon="person-circle" expandable :title="person?.name || 'Account'" popovertarget="account-menu">
                </ndd-icon-button>
                <ndd-menu id="account-menu" anchor="account-menu-btn">
                  <template v-if="!authLoading && authenticated">
                    <ndd-menu-item :text="person?.name || person?.email" disabled></ndd-menu-item>
                    <ndd-menu-divider></ndd-menu-divider>
                    <ndd-menu-item text="Uitloggen" @click="logout"></ndd-menu-item>
                  </template>
                  <template v-else-if="!authLoading && oidcConfigured">
                    <ndd-menu-item text="Inloggen" @click="() => window.location.href = '/auth/login'"></ndd-menu-item>
                  </template>
                </ndd-menu>
              </ndd-button-bar>
            </ndd-toolbar-item>
          </ndd-toolbar>

          <ndd-spacer size="8"></ndd-spacer>

          <!-- Document Tab Bar -->
          <ndd-document-tab-bar v-if="openTabs.length > 0">
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
        <!-- Empty state: no tabs open -->
        <ndd-page v-if="!activeTab">
          <ndd-simple-section align="center">
            <ndd-inline-dialog text="Open een artikel vanuit de bibliotheek om te bewerken"></ndd-inline-dialog>
          </ndd-simple-section>
        </ndd-page>

        <!-- Error state -->
        <ndd-page v-else-if="error">
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
              <ndd-top-title-bar slot="header" :text="middlePaneTitle">
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

          <!-- Right: Execution Result or Machine Readable -->
          <ndd-split-view-pane slot="pane-3">
            <ndd-page sticky-header>
              <ndd-top-title-bar slot="header" :text="rightPaneTitle">
                <ndd-segmented-control slot="toolbar" size="md" :value="rightPaneView" @change="onRightPaneChange">
                  <ndd-segmented-control-item value="result" text="Resultaat"></ndd-segmented-control-item>
                  <ndd-segmented-control-item value="machine" text="Machine"></ndd-segmented-control-item>
                </ndd-segmented-control>
              </ndd-top-title-bar>

              <ExecutionTraceView
                v-if="rightPaneView === 'result'"
                :result="lastResult"
                :trace-text="lastTraceText"
                :error="lastError"
                :expectations="lastExpectations"
              />

              <!-- Machine view: structured editor -->
              <ndd-simple-section v-else-if="rightPaneView === 'machine'">
                <MachineReadable
                  :article="editedArticle"
                  :editable="canEdit"
                  @open-action="handleOpenAction"
                  @open-edit="activeEditItem = $event"
                  @init-mr="handleInitMr"
                  @add-action="handleAddAction"
                />
              </ndd-simple-section>
            </ndd-page>
          </ndd-split-view-pane>
        </ndd-side-by-side-split-view>
      </ndd-split-view-pane>
    </ndd-bar-split-view>
  </ndd-app-view>

  <ActionSheet :action="activeAction" :article="editedArticle" :editable="canEdit" @close="handleActionClose" @save="handleActionSave" />
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

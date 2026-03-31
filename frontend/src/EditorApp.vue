<script setup>
import { ref, computed, watch } from 'vue';
import yaml from 'js-yaml';
import { useLaw } from './composables/useLaw.js';
import { useEngine } from './composables/useEngine.js';
import { useExecution } from './composables/useExecution.js';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import ExecutionTraceView from './components/ExecutionTraceView.vue';

const { law, lawId, rawYaml, articles, lawName, selectedArticle, selectedArticleNumber, loading, error } = useLaw();

const rightPaneView = ref('machine');
const isTestMode = computed(() => rightPaneView.value === 'test');

function onRightPaneChange(event) {
  const value = event.target?.value ?? event.detail?.[0];
  if (value) rightPaneView.value = value;
}

// --- Engine (shared for test mode) ---
const { ready: engineReady, initError: engineInitError, initEngine, getEngine } = useEngine();
initEngine().catch(() => {});

// Load current law into engine when YAML is available
watch(
  [() => rawYaml.value, engineReady],
  async ([lawYaml, isReady]) => {
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

// --- Execution state (shared between form and trace panels) ---
const {
  result: execResult,
  trace: execTrace,
  traceText: execTraceText,
  running: execRunning,
  error: execError,
  expectations: execExpectations,
  execute: execExecute,
  reset: execReset,
} = useExecution();

function handleFormExecute(payload) {
  const engine = getEngine();
  if (!engine) return;
  execExecute(engine, payload);
}

// Reset execution state when switching away from test mode
watch(isTestMode, (isTest) => {
  if (!isTest) execReset();
});

// --- Editor state ---
const activeAction = ref(null);
const activeEditItem = ref(null);
const parseError = ref(null);

// ── Reactive data model (single source of truth) ──
const machineReadable = ref(null);
const yamlSource = ref('');

const dumpOpts = { lineWidth: 80, noRefs: true };

// Initialize from article
watch(selectedArticle, (article) => {
  activeAction.value = null;
  activeEditItem.value = null;
  const mr = article?.machine_readable;
  machineReadable.value = mr ? JSON.parse(JSON.stringify(mr)) : null;
  yamlSource.value = mr ? yaml.dump(mr, dumpOpts) : '';
  parseError.value = null;
}, { immediate: true });

// Virtual article for components (reads from machineReadable)
const editedArticle = computed(() => {
  if (!selectedArticle.value) return null;
  return { ...selectedArticle.value, machine_readable: machineReadable.value };
});

// YAML textarea input → parse to model
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

// Right-panel save → update model → re-dump YAML
function handleSave({ section, key, newKey, index, data }) {
  const mr = machineReadable.value
    ? JSON.parse(JSON.stringify(machineReadable.value))
    : {};

  // Ensure structure exists for adds
  if (!mr.definitions) mr.definitions = {};
  if (!mr.execution) mr.execution = {};
  if (!mr.execution.parameters) mr.execution.parameters = [];
  if (!mr.execution.input) mr.execution.input = [];
  if (!mr.execution.output) mr.execution.output = [];

  if (section === 'definition') {
    if (newKey && newKey !== key) {
      delete mr.definitions[key];
    }
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

  // Trigger reactivity + sync YAML
  machineReadable.value = mr;
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}

function selectArticle(number) {
  activeAction.value = null;
  selectedArticleNumber.value = String(number);
}
</script>

<template>
  <rr-app-view>
    <rr-bar-split-view>
      <!-- Primary Bar: App Toolbar + Document Tabs -->
      <rr-split-view-pane slot="primary-bar">
        <rr-container>
          <rr-toolbar size="md">
            <rr-toolbar-start-area>
              <rr-toolbar-item>
                <rr-tab-bar size="md">
                  <rr-tab-bar-item href="/library">Bibliotheek</rr-tab-bar-item>
                  <rr-tab-bar-item selected>Editor</rr-tab-bar-item>
                </rr-tab-bar>
              </rr-toolbar-item>
            </rr-toolbar-start-area>
            <rr-toolbar-center-area>
              <rr-toolbar-item>
                <rr-segmented-control size="md" :value="rightPaneView" @change="onRightPaneChange">
                  <rr-segmented-control-item value="machine">Machine Readable</rr-segmented-control-item>
                  <rr-segmented-control-item value="test">Test</rr-segmented-control-item>
                </rr-segmented-control>
              </rr-toolbar-item>
            </rr-toolbar-center-area>
            <rr-toolbar-end-area>
              <rr-toolbar-item>
                <rr-icon-button variant="neutral-tinted" size="m" icon="inbox" title="Notificaties">
                </rr-icon-button>
              </rr-toolbar-item>
              <rr-toolbar-item>
                <rr-button-bar size="md">
                  <rr-button variant="neutral-tinted" size="md" is-picker>RR Project</rr-button>
                  <rr-icon-button variant="neutral-tinted" size="m" icon="person-circle" has-menu title="Account">
                  </rr-icon-button>
                </rr-button-bar>
              </rr-toolbar-item>
            </rr-toolbar-end-area>
          </rr-toolbar>

          <!-- Document Tab Bar -->
          <rr-document-tab-bar v-if="!loading && !error">
            <rr-document-tab-bar-item
              v-for="article in articles"
              :key="article.number"
              :subtitle="lawName"
              :selected="String(article.number) === String(selectedArticleNumber) || undefined"
              has-dismiss-button
              @click="selectArticle(article.number)"
            >
              Artikel {{ article.number }}
            </rr-document-tab-bar-item>
          </rr-document-tab-bar>
        </rr-container>
      </rr-split-view-pane>

      <!-- Main content area -->
      <rr-split-view-pane slot="main">
        <!-- Error state -->
        <div v-if="error" style="padding: 32px; color: #c00; text-align: center;">
          Kon de wet niet laden: {{ error.message }}
        </div>

        <!-- Machine Readable mode: original 3-column navigation layout -->
        <rr-navigation-split-view v-else-if="!isTestMode">

          <!-- Sidebar: Text -->
          <rr-split-view-pane slot="sidebar" has-content>
            <rr-page header-sticky>
              <rr-toolbar slot="header" size="md">
                <rr-toolbar-start-area>
                  <rr-toolbar-item>
                    <rr-button variant="neutral-tinted" size="md" expandable>
                      Tekst
                    </rr-button>
                  </rr-toolbar-item>
                </rr-toolbar-start-area>
                <rr-toolbar-end-area>
                  <rr-toolbar-item>
                    <rr-segmented-control size="md" content-type="icons">
                      <rr-segmented-control-item value="bold" title="Bold">
                        <rr-icon name="bold"></rr-icon>
                      </rr-segmented-control-item>
                      <rr-segmented-control-item value="italic" title="Italic">
                        <rr-icon name="italic"></rr-icon>
                      </rr-segmented-control-item>
                    </rr-segmented-control>
                  </rr-toolbar-item>
                  <rr-toolbar-item>
                    <rr-segmented-control size="md" content-type="icons">
                      <rr-segmented-control-item value="hr" title="Horizontale lijn">
                        <rr-icon name="minus"></rr-icon>
                      </rr-segmented-control-item>
                      <rr-segmented-control-item value="ul" title="Bullet list">
                        <rr-icon name="bullet-list"></rr-icon>
                      </rr-segmented-control-item>
                      <rr-segmented-control-item value="ol" title="Numbered list">
                        <rr-icon name="numbered-list"></rr-icon>
                      </rr-segmented-control-item>
                    </rr-segmented-control>
                  </rr-toolbar-item>
                </rr-toolbar-end-area>
              </rr-toolbar>

              <rr-simple-section>
                <ArticleText :article="selectedArticle" />
              </rr-simple-section>
            </rr-page>
          </rr-split-view-pane>

          <!-- Secondary Sidebar: YAML -->
          <rr-split-view-pane slot="secondary-sidebar" has-content>
            <rr-page header-sticky>
              <rr-toolbar slot="header" size="md">
                <rr-toolbar-start-area>
                  <rr-toolbar-item>
                    <rr-title-bar size="5">YAML</rr-title-bar>
                  </rr-toolbar-item>
                </rr-toolbar-start-area>
                <rr-toolbar-end-area>
                  <rr-toolbar-item v-if="parseError">
                    <span class="editor-parse-error">YAML parse error</span>
                  </rr-toolbar-item>
                </rr-toolbar-end-area>
              </rr-toolbar>

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
            </rr-page>
          </rr-split-view-pane>

          <!-- Main: Machine Readable -->
          <rr-split-view-pane slot="main" has-content>
            <rr-page header-sticky>
              <rr-simple-section>
                <MachineReadable
                  :article="editedArticle"
                  :editable="true"
                  @open-edit="activeEditItem = $event"
                  @open-action="activeAction = $event"
                />
              </rr-simple-section>
            </rr-page>
          </rr-split-view-pane>

        </rr-navigation-split-view>

        <!-- Test mode: equal 3-column layout -->
        <div v-else class="test-mode-layout">
          <!-- Left: Article Text -->
          <div class="test-pane">
            <rr-page header-sticky>
              <rr-toolbar slot="header" size="md">
                <rr-toolbar-start-area>
                  <rr-toolbar-item>
                    <rr-title-bar size="5">Tekst</rr-title-bar>
                  </rr-toolbar-item>
                </rr-toolbar-start-area>
              </rr-toolbar>
              <rr-simple-section>
                <ArticleText :article="selectedArticle" />
              </rr-simple-section>
            </rr-page>
          </div>

          <!-- Middle: Form -->
          <div class="test-pane">
            <rr-page header-sticky>
              <rr-toolbar slot="header" size="md">
                <rr-toolbar-start-area>
                  <rr-toolbar-item>
                    <rr-title-bar size="5">Formulier</rr-title-bar>
                  </rr-toolbar-item>
                </rr-toolbar-start-area>
              </rr-toolbar>

              <!-- Engine error -->
              <div v-if="engineInitError" class="test-engine-error">
                WASM engine failed to load: {{ engineInitError.message }}
                <div class="test-engine-error-hint">
                  Run <code>just wasm-build</code> to build the WASM module.
                </div>
              </div>

              <ScenarioBuilder
                v-else
                :law-id="lawId"
                :law-yaml="rawYaml"
                :engine="getEngine()"
                :ready="engineReady"
                :running="execRunning"
                @execute="handleFormExecute"
              />
            </rr-page>
          </div>

          <!-- Right: Execution Trace -->
          <div class="test-pane">
            <rr-page header-sticky>
              <rr-toolbar slot="header" size="md">
                <rr-toolbar-start-area>
                  <rr-toolbar-item>
                    <rr-title-bar size="5">Resultaat</rr-title-bar>
                  </rr-toolbar-item>
                </rr-toolbar-start-area>
              </rr-toolbar>

              <ExecutionTraceView
                :result="execResult"
                :trace="execTrace"
                :trace-text="execTraceText"
                :expectations="execExpectations"
                :error="execError"
                :running="execRunning"
              />
            </rr-page>
          </div>
        </div>
      </rr-split-view-pane>
    </rr-bar-split-view>
  </rr-app-view>

  <ActionSheet :action="activeAction" :article="editedArticle" @close="activeAction = null" />
  <EditSheet :item="activeEditItem" @save="handleSave" @close="activeEditItem = null" />
</template>

<style>
/* Test mode: 3-column equal-width layout */
.test-mode-layout {
  display: flex;
  flex: 1;
  min-height: 0;
  height: 100%;
}

.test-pane {
  flex: 1 1 0;
  min-width: 0;
  overflow: hidden;
  border-right: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.test-pane:last-child {
  border-right: none;
}

.test-engine-error {
  padding: 12px 16px;
  background: #fee;
  color: #c00;
  font-size: 13px;
}

.test-engine-error-hint {
  margin-top: 4px;
  font-size: 12px;
  color: #999;
}

.test-engine-error-hint code {
  background: #eee;
  padding: 1px 4px;
  border-radius: 3px;
}

/* Editor YAML styles */
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

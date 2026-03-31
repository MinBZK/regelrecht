<script setup>
import { ref, computed, watch } from 'vue';
import yaml from 'js-yaml';
import { useLaw } from './composables/useLaw.js';
import { useEngine } from './composables/useEngine.js';
import { useExecution } from './composables/useExecution.js';
import ArticleText from './components/ArticleText.vue';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import ExecutionTraceView from './components/ExecutionTraceView.vue';

const { law, lawId, rawYaml, articles, lawName, selectedArticle, selectedArticleNumber, loading, error } = useLaw();

const middlePaneView = ref('form');

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

// --- Execution state (shared between form and result panels) ---
const {
  result: execResult,
  trace: execTrace,
  traceText: execTraceText,
  running: execRunning,
  error: execError,
  expectations: execExpectations,
  execute: execExecute,
} = useExecution();

function handleFormExecute(payload) {
  const engine = getEngine();
  if (!engine) return;
  execExecute(engine, payload);
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
            <rr-toolbar-end-area>
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

        <!-- 3-column equal layout: Text | Form | Result -->
        <div v-else class="editor-layout">
          <!-- Left: Article Text -->
          <div class="editor-pane">
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

          <!-- Middle: Form or YAML -->
          <div class="editor-pane">
            <rr-page header-sticky>
              <rr-toolbar slot="header" size="md">
                <rr-toolbar-start-area>
                  <rr-toolbar-item>
                    <rr-segmented-control size="md" :value="middlePaneView" @change="onMiddlePaneChange">
                      <rr-segmented-control-item value="form">Formulier</rr-segmented-control-item>
                      <rr-segmented-control-item value="yaml">YAML</rr-segmented-control-item>
                    </rr-segmented-control>
                  </rr-toolbar-item>
                </rr-toolbar-start-area>
                <rr-toolbar-end-area>
                  <rr-toolbar-item v-if="middlePaneView === 'yaml' && parseError">
                    <span class="editor-parse-error">YAML parse error</span>
                  </rr-toolbar-item>
                </rr-toolbar-end-area>
              </rr-toolbar>

              <!-- Form view -->
              <div v-if="middlePaneView === 'form'">
                <div v-if="engineInitError" class="editor-engine-error">
                  WASM engine failed to load: {{ engineInitError.message }}
                  <div class="editor-engine-error-hint">
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
              </div>

              <!-- YAML view -->
              <div v-if="middlePaneView === 'yaml'" class="editor-yaml-wrap">
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
          </div>

          <!-- Right: Execution Result -->
          <div class="editor-pane">
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
.editor-layout {
  display: flex;
  flex: 1;
  min-height: 0;
  height: 100%;
}

.editor-pane {
  flex: 1 1 0;
  min-width: 0;
  overflow: hidden;
  border-right: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.editor-pane:last-child {
  border-right: none;
}

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

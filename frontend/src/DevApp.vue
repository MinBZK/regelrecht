<script setup>
import { ref, computed, watch } from 'vue';
import yaml from 'js-yaml';
import { useLaw } from './composables/useLaw.js';
import MachineReadable from './components/MachineReadable.vue';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';

const { articles, lawName, selectedArticle, selectedArticleNumber, loading, error } = useLaw();

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
    machineReadable.value = yaml.load(text);
    parseError.value = null;
  } catch (e) {
    parseError.value = e.message;
  }
}

// Right-panel save → update model → re-dump YAML
function handleSave({ section, key, newKey, index, data }) {
  if (!machineReadable.value) return;
  const mr = JSON.parse(JSON.stringify(machineReadable.value));

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
  selectedArticleNumber.value = String(number);
}
</script>

<template>
  <rr-page header-sticky>
    <!-- Header -->
    <rr-toolbar slot="header" size="md">
      <rr-toolbar-start-area>
        <rr-toolbar-item>
          <rr-tab-bar size="md">
            <rr-tab-bar-item href="index.html">Bibliotheek</rr-tab-bar-item>
            <rr-tab-bar-item href="editor.html">Editor</rr-tab-bar-item>
            <rr-tab-bar-item selected>Dev</rr-tab-bar-item>
          </rr-tab-bar>
        </rr-toolbar-item>
      </rr-toolbar-start-area>
      <rr-toolbar-center-area>
        <rr-toolbar-item>
          <span class="dev-badge">Development</span>
        </rr-toolbar-item>
      </rr-toolbar-center-area>
    </rr-toolbar>

    <!-- Error state -->
    <div v-if="error" style="padding: 32px; color: #c00; text-align: center;">
      Kon de wet niet laden: {{ error.message }}
    </div>

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

    <!-- Main: Side-by-side split view -->
    <rr-side-by-side-split-view>

      <!-- Left Pane: Editable YAML -->
      <div slot="pane-1" class="dev-yaml-pane">
        <rr-page header-sticky>
          <rr-toolbar slot="header" size="md">
            <rr-toolbar-start-area>
              <rr-toolbar-item>
                <span class="dev-pane-title">machine_readable YAML</span>
              </rr-toolbar-item>
            </rr-toolbar-start-area>
            <rr-toolbar-end-area>
              <rr-toolbar-item v-if="parseError">
                <span class="dev-parse-error">YAML parse error</span>
              </rr-toolbar-item>
            </rr-toolbar-end-area>
          </rr-toolbar>

          <div class="dev-yaml-editor-wrap">
            <textarea
              :value="yamlSource"
              @input="onYamlInput"
              class="dev-yaml-editor"
              spellcheck="false"
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
            ></textarea>
            <div v-if="parseError" class="dev-parse-error-detail">{{ parseError }}</div>
          </div>
        </rr-page>
      </div>

      <!-- Right Pane: Machine Readable -->
      <div slot="pane-2">
        <rr-page header-sticky>
          <rr-toolbar slot="header" size="md">
            <rr-toolbar-start-area>
              <rr-toolbar-item>
                <span class="dev-pane-title">Machine Readable</span>
              </rr-toolbar-item>
            </rr-toolbar-start-area>
          </rr-toolbar>

          <rr-simple-section>
            <MachineReadable
              :article="editedArticle"
              :editable="true"
              @open-edit="activeEditItem = $event"
              @open-action="activeAction = $event"
            />
          </rr-simple-section>
        </rr-page>
      </div>

    </rr-side-by-side-split-view>
  </rr-page>

  <ActionSheet :action="activeAction" :article="editedArticle" @close="activeAction = null" />
  <EditSheet :item="activeEditItem" @save="handleSave" @close="activeEditItem = null" />
</template>

<style>
.dev-badge {
  display: inline-block;
  background: #E8740C;
  color: white;
  font-size: 12px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 10px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.dev-pane-title {
  font-family: var(--rr-font-family-title, 'RijksSansVF', sans-serif);
  font-weight: 550;
  font-size: 14px;
  color: var(--semantics-text-primary-color, #333B44);
}

.dev-yaml-pane {
  background: #1e1e2e;
  height: 100%;
}

.dev-yaml-editor-wrap {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 0;
}

.dev-yaml-editor {
  flex: 1;
  width: 100%;
  min-height: 0;
  height: calc(100vh - 120px);
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

.dev-yaml-editor::selection {
  background: #45475a;
}

.dev-parse-error {
  font-size: 12px;
  font-weight: 600;
  color: #c00;
  background: #fee;
  padding: 2px 8px;
  border-radius: 6px;
}

.dev-parse-error-detail {
  background: #2a1a1a;
  color: #f38ba8;
  font-family: 'SF Mono', monospace;
  font-size: 12px;
  padding: 8px 16px;
  border-top: 1px solid #45475a;
}
</style>

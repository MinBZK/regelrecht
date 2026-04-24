<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import yaml from 'js-yaml';
import { useLaw, fetchLaw } from './composables/useLaw.js';
import { useEngine } from './composables/useEngine.js';
import { useAuth } from './composables/useAuth.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import ArticleText from './components/ArticleText.vue';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import SearchWindow from './components/SearchWindow.vue';
import MachineReadable from './components/MachineReadable.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import ExecutionTraceView from './components/ExecutionTraceView.vue';

const { authenticated, loading: authLoading, oidcConfigured, person, login, logout } = useAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();

const editorPanelFlags = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
];

const showTextPane = computed(() => isEnabled('panel.article_text'));
const showFormPane = computed(() => isEnabled('panel.scenario_form'));
const showYamlPane = computed(() => isEnabled('panel.yaml_editor'));
const showMachinePane = computed(() => isEnabled('panel.machine_readable'));

// Compute visible pane count and slot assignments for split view.
const visiblePanes = computed(() => {
  const panes = [];
  if (showTextPane.value) panes.push('text');
  if (showMachinePane.value) panes.push('machine');
  if (showFormPane.value) panes.push('form');
  if (showYamlPane.value) panes.push('yaml');
  return panes.length > 0 ? panes : ['text', 'machine', 'form', 'yaml'];
});
const paneSlot = (name) => {
  const idx = visiblePanes.value.indexOf(name);
  return idx >= 0 ? `pane-${idx + 1}` : undefined;
};

// All edit operations are gated behind SSO. When OIDC is configured the user
// must be authenticated; when OIDC is disabled the editor is fully open.
// In practice the `requiresAuth` router guard already awaits the auth-check
// and blocks unauthenticated users before this component mounts, so canEdit
// is always true here — the computed remains as a safety net.
const canEdit = computed(() => !oidcConfigured.value || authenticated.value);

const route = useRoute();
const router = useRouter();

// --- Initial law load (from route params) ---
const {
  law,
  lawId,
  rawYaml,
  articles,
  lawName,
  selectedArticle,
  selectedArticleNumber,
  switchLaw,
  loading,
  error,
  saving: lawSaving,
  saveError: lawSaveError,
  saveLaw,
} = useLaw(route.params.lawId, route.params.articleNumber);

const resultSheetOpen = ref(false);

// --- Corpus search (reuse LibraryApp's SearchWindow) ---
const corpusLaws = ref([]);
const searchOpen = ref(false);

async function loadCorpusLaws() {
  try {
    const res = await fetch('/api/corpus/laws?limit=1000');
    if (!res.ok) return;
    const list = await res.json();
    corpusLaws.value = list.sort((a, b) => a.law_id.localeCompare(b.law_id));
  } catch { /* ignore — search is a convenience */ }
}
loadCorpusLaws();

function openSearch() {
  searchOpen.value = true;
}

function onSearchSelectLaw(lawId) {
  router.push(`/editor/${encodeURIComponent(lawId)}`);
}

async function onSearchHarvestAvailable(slug) {
  await fetch('/api/corpus/reload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ law_ids: [slug] }),
  }).catch(() => {});
  await loadCorpusLaws();
  router.push(`/editor/${encodeURIComponent(slug)}`);
}
const resultSheetEl = ref(null);
watch(resultSheetOpen, async (open) => {
  await nextTick();
  if (open) resultSheetEl.value?.show();
  else resultSheetEl.value?.hide();
});

// --- Multi-law tab state (persisted in localStorage) ---
const TABS_STORAGE_KEY = 'regelrecht-open-tabs';
const ACTIVE_TAB_STORAGE_KEY = 'regelrecht-active-tab';

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

function loadSavedActiveTab() {
  try {
    const saved = localStorage.getItem(ACTIVE_TAB_STORAGE_KEY);
    return saved ? JSON.parse(saved) : null;
  } catch { return null; }
}

function saveActiveTab(tab) {
  if (!tab) localStorage.removeItem(ACTIVE_TAB_STORAGE_KEY);
  else localStorage.setItem(ACTIVE_TAB_STORAGE_KEY, JSON.stringify(tab));
}

// If the user lands on /editor without a lawId, restore the last tab
// they had open before the refresh.
if (!route.params.lawId) {
  const last = loadSavedActiveTab();
  if (last?.lawId) {
    router.replace({
      name: 'editor',
      params: { lawId: last.lawId, articleNumber: last.articleNumber || undefined },
    });
  }
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
  saveActiveTab(activeTab.value);
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

// --- Engine ---
const { ready: engineReady, initError: engineInitError, initEngine, getEngine } = useEngine();
initEngine().catch(() => {});

// The engine-loading watch lives below, next to `currentLawYaml`, so it
// observes in-memory edits rather than only the persisted `rawYaml`.

// --- Trace state (receives trace from last executed scenario) ---
const lastTraceText = ref(null);
const lastResult = ref(null);
const lastError = ref(null);
const lastExpectations = ref({});
const lastScenarioName = ref('');

function handleScenarioExecuted({ result, traceText, error, expectations, scenarioName }) {
  lastResult.value = result;
  lastTraceText.value = traceText;
  lastError.value = error || null;
  lastExpectations.value = expectations || {};
  lastScenarioName.value = scenarioName || '';
  resultSheetOpen.value = true;
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

// Parse rawYaml once per law load into a reusable document skeleton. The
// computed below splices in the currently edited article's
// machine_readable on every reactive change, so without this cache each
// keystroke in the YAML textarea would re-parse the whole ~25-200 KiB law
// on the main thread. Hoisting the parse to a computed keyed only on
// rawYaml drops that cost to one parse per load.
const parsedRawLaw = computed(() => {
  if (!rawYaml.value) return null;
  try {
    return yaml.load(rawYaml.value);
  } catch {
    return null;
  }
});

// Reactive "edited" law YAML: rawYaml with the currently selected article's
// machine_readable substituted in. This is what flows into the engine and
// into ScenarioBuilder, so in-memory edits re-execute scenarios without a
// round-trip through the backend.
//
// Only the currently selected article's machine_readable is swapped — edits
// on other articles are not tracked across tab switches (existing behavior
// of the editor state model).
//
// KNOWN LIMITATION: when this value is sent to `saveLaw` (via the Machine
// panel save button), the body is the `yaml.dump` output of the
// reconstructed document — which strips YAML comments and may reorder
// top-level keys compared to `rawYaml`. The YAML-pane edit path preserves
// the user's exact text via `yamlSource`, so it does not have this drift.
// Today's corpus is harvester-generated and comment-free, so the impact is
// zero in practice; revisit if hand-annotated laws are introduced (e.g.
// keep an "as-typed" base alongside `rawYaml` and only re-dump the edited
// article).
const currentLawYaml = computed(() => {
  if (!rawYaml.value) return null;
  if (!selectedArticle.value || machineReadable.value == null) {
    return rawYaml.value;
  }
  const base = parsedRawLaw.value;
  if (!base) return rawYaml.value;
  try {
    // Shallow-clone the doc and the articles array so our splice doesn't
    // mutate the memoized `parsedRawLaw` value — Vue would consider the
    // computed still fresh but the next read would see our substituted
    // article instead of the original.
    const doc = { ...base };
    const docArticles = Array.isArray(base.articles) ? [...base.articles] : null;
    if (!docArticles) return rawYaml.value;
    const idx = docArticles.findIndex(
      (a) => String(a.number) === String(selectedArticleNumber.value),
    );
    if (idx < 0) return rawYaml.value;
    docArticles[idx] = {
      ...docArticles[idx],
      machine_readable: machineReadable.value,
    };
    doc.articles = docArticles;
    return yaml.dump(doc, dumpOpts);
  } catch {
    return rawYaml.value;
  }
});

// Load current law into engine. Reacts to currentLawYaml so in-memory edits
// are immediately visible to scenarios.
watch(
  [currentLawYaml, engineReady],
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

// Dirty state: the selected article's in-memory machine_readable differs
// from the article's saved copy. `machineReadable.value` starts as a deep
// JSON clone of `selectedArticle.machine_readable` (see the `watch` above),
// so for field-based edits the two share the same key order and
// `JSON.stringify` is a cheap, accurate structural comparison.
//
// Note: the YAML-pane edit path (`onYamlInput`) replaces `machineReadable`
// with a fresh `yaml.load(text)` object whose key order comes from the
// textarea, so a no-op round-trip can flip this flag to `true` even when
// the semantic content is unchanged. That's a conservative false positive
// — the worst case is an enabled save button — so we accept it rather
// than pay for a canonical YAML dump on every keystroke.
const isMachineReadableDirty = computed(() => {
  if (!selectedArticle.value) return false;
  const saved = selectedArticle.value.machine_readable ?? null;
  const current = machineReadable.value ?? null;
  if (saved == null && current == null) return false;
  try {
    return JSON.stringify(saved) !== JSON.stringify(current);
  } catch {
    return true;
  }
});

async function handleMachineReadableSave() {
  const lawYaml = currentLawYaml.value;
  if (!lawYaml) return;
  // Snapshot the law id before the await. saveLaw itself guards its own
  // reactive writes with the same check, but the post-save cleanup below
  // runs in the EditorApp scope and would happily overwrite the new law's
  // in-progress machine_readable with its pristine article data if the
  // user switched laws mid-flight.
  const savedLawId = lawId.value;
  try {
    await saveLaw(lawYaml);
    if (lawId.value !== savedLawId) return; // law switched mid-PUT
    // After save, `rawYaml` is the saved text and `selectedArticle` now
    // points at the re-parsed article. We could rely on the `watch`
    // further up to re-sync `machineReadable` from the new selectedArticle,
    // but that watcher fires on the next microtask — leaving a window
    // where `isMachineReadableDirty` still sees the pre-save object and
    // the save button stays enabled, enabling a double-save click. Reset
    // `machineReadable` explicitly from the freshly-parsed article so the
    // dirty flag clears synchronously with the save.
    const fresh = selectedArticle.value?.machine_readable ?? null;
    machineReadable.value = fresh ? JSON.parse(JSON.stringify(fresh)) : null;
    yamlSource.value = fresh ? yaml.dump(fresh, dumpOpts) : '';
  } catch (e) {
    // saveError is surfaced via lawSaveError; log for dev visibility.
    console.warn('saveLaw failed:', e);
  }
}

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

// Delete an item from the machine_readable. Mirrors handleSave's section
// dispatch but removes the entry instead of replacing it. Definitions are
// keyed by name; parameters / inputs / outputs / actions are keyed by
// array index. Out-of-range indices and missing keys are no-ops so a
// stale event from the UI can never crash.
function handleDelete({ section, key, index }) {
  const mr = machineReadable.value
    ? JSON.parse(JSON.stringify(machineReadable.value))
    : null;
  if (!mr) return;

  if (section === 'definition') {
    if (mr.definitions && key != null && key in mr.definitions) {
      delete mr.definitions[key];
    }
  } else if (section === 'parameter') {
    if (mr.execution?.parameters && index >= 0 && index < mr.execution.parameters.length) {
      mr.execution.parameters.splice(index, 1);
    }
  } else if (section === 'input') {
    if (mr.execution?.input && index >= 0 && index < mr.execution.input.length) {
      mr.execution.input.splice(index, 1);
    }
  } else if (section === 'output') {
    if (mr.execution?.output && index >= 0 && index < mr.execution.output.length) {
      mr.execution.output.splice(index, 1);
    }
  } else if (section === 'action') {
    if (mr.execution?.actions && index >= 0 && index < mr.execution.actions.length) {
      mr.execution.actions.splice(index, 1);
    }
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
  // Seed the new action with an EQUALS stub instead of an empty literal so
  // OperationSettings has an operation tree to render and the user can
  // immediately reach the type dropdown to switch to AGE / AND / etc.
  // The findIncompleteOperation guard rejects unfilled stubs on save, so
  // a half-configured action still can't be persisted.
  const newAction = {
    output: '',
    value: { operation: 'EQUALS', subject: '', value: '' },
  };
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
  // AGE has two structural slots — both must be filled. Empty strings are
  // the seed values from changeOperationType('AGE'); reject them so the
  // user can't save a stub.
  if (op === 'AGE') {
    if ((value.date_of_birth ?? '') === '') return op;
    if ((value.reference_date ?? '') === '') return op;
  }
  // Recurse into structural slots
  for (const child of [value.subject, value.value, value.default, value.date_of_birth, value.reference_date]) {
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
  <nldd-app-view>
    <nldd-bar-split-view>
      <!-- Primary Bar: App Toolbar + Document Tabs -->
      <nldd-split-view-pane slot="primary-bar">
      <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md">
                <nldd-tab-bar-item href="/library" @click.prevent="router.push('/library')" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="center" min-width="240px" width="40%">
              <nldd-search-field
                size="md"
                placeholder="Zoeken"
                @focus="openSearch"
                @click="openSearch"
              ></nldd-search-field>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button-bar size="md">
                <nldd-button id="project-menu-btn" size="md" expandable text="RR Project" popovertarget="project-menu"></nldd-button>
                <nldd-menu id="project-menu" anchor="project-menu-btn">
                  <nldd-menu-item text="Instellingen"></nldd-menu-item>
                  <nldd-menu-item text="Leden"></nldd-menu-item>
                  <nldd-menu-divider></nldd-menu-divider>
                  <nldd-menu-item text="Nieuw project"></nldd-menu-item>
                </nldd-menu>
                <nldd-button-bar-divider></nldd-button-bar-divider>
                <nldd-icon-button id="account-menu-btn" size="md" icon="person-circle" expandable :title="person?.name || 'Account'" popovertarget="account-menu">
                </nldd-icon-button>
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
                  <nldd-menu-divider></nldd-menu-divider>
                  <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
                  <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login"></nldd-menu-item>
                </nldd-menu>
              </nldd-button-bar>
            </nldd-toolbar-item>
          </nldd-toolbar>

          <nldd-spacer size="8"></nldd-spacer>

          <!-- Document Tab Bar -->
          <nldd-document-tab-bar v-if="openTabs.length > 0">
            <nldd-document-tab-bar-item
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
            </nldd-document-tab-bar-item>
          </nldd-document-tab-bar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Main content area -->
      <nldd-split-view-pane slot="main">
        <!-- Empty state: no tabs open -->
        <nldd-page v-if="!activeTab">
          <nldd-simple-section align="center">
            <nldd-inline-dialog text="Open een artikel vanuit de bibliotheek om te bewerken"></nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- Error state -->
        <nldd-page v-else-if="error">
          <nldd-simple-section align="center">
            <nldd-inline-dialog variant="alert" text="Kon de wet niet laden" :supporting-text="error.message"></nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- Dynamic column layout based on feature flags -->
        <nldd-side-by-side-split-view v-else :panes="String(visiblePanes.length)">
          <!-- Left: Article Text -->
          <nldd-split-view-pane v-if="showTextPane" :slot="paneSlot('text')">
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" text="Tekst"></nldd-top-title-bar>
              <nldd-simple-section :align="selectedArticle ? undefined : 'center'">
                <ArticleText :article="selectedArticle" raw />
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Scenarios -->
          <nldd-split-view-pane v-if="showFormPane" :slot="paneSlot('form')">
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" text="Scenario's"></nldd-top-title-bar>

              <nldd-simple-section v-if="engineInitError" align="center">
                <nldd-inline-dialog variant="alert" text="WASM engine niet geladen" :supporting-text="`${engineInitError.message} — voer 'just wasm-build' uit om de WASM module te bouwen.`"></nldd-inline-dialog>
              </nldd-simple-section>

              <ScenarioBuilder
                v-else
                :law-id="lawId"
                :law-yaml="currentLawYaml"
                :engine="getEngine()"
                :ready="engineReady"
                :articles="articles"
                @executed="handleScenarioExecuted"
              />
            </nldd-page>
          </nldd-split-view-pane>

          <!-- YAML -->
          <nldd-split-view-pane v-if="showYamlPane" :slot="paneSlot('yaml')">
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" text="YAML">
                <span v-if="parseError" slot="toolbar" class="editor-parse-error">YAML parse error</span>
              </nldd-top-title-bar>

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
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Machine Readable -->
          <nldd-split-view-pane v-if="showMachinePane" :slot="paneSlot('machine')">
            <nldd-page sticky-header :sticky-footer="canEdit && (isMachineReadableDirty || lawSaving) || undefined">
              <nldd-top-title-bar slot="header" text="Machine"></nldd-top-title-bar>
              <nldd-simple-section>
                <MachineReadable
                  :article="editedArticle"
                  :editable="canEdit"
                  :dirty="isMachineReadableDirty"
                  :saving="lawSaving"
                  :save-error="lawSaveError"
                  @open-action="handleOpenAction"
                  @open-edit="activeEditItem = $event"
                  @init-mr="handleInitMr"
                  @add-action="handleAddAction"
                  @save="handleMachineReadableSave"
                  @delete="handleDelete"
                />
              </nldd-simple-section>
              <nldd-container v-if="canEdit && (isMachineReadableDirty || lawSaving)" slot="footer" padding="16">
                <nldd-button
                  variant="primary"
                  size="md"
                  full-width
                  data-testid="save-mr-btn"
                  :disabled="lawSaving || undefined"
                  :text="lawSaving ? 'Opslaan…' : 'Opslaan'"
                  @click="handleMachineReadableSave"
                ></nldd-button>
              </nldd-container>
            </nldd-page>
          </nldd-split-view-pane>
        </nldd-side-by-side-split-view>
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-app-view>

  <ActionSheet :action="activeAction" :article="editedArticle" :editable="canEdit" @close="handleActionClose" @save="handleActionSave" />
  <EditSheet :item="activeEditItem" :article="editedArticle" @save="handleSave" @close="activeEditItem = null" />
  <SearchWindow
    v-model="searchOpen"
    :laws="corpusLaws"
    @select-law="onSearchSelectLaw"
    @harvest-available="onSearchHarvestAvailable"
  />

  <!-- Result of the most recently executed scenario, opened as a bottom sheet. -->
  <nldd-sheet
    ref="resultSheetEl"
    placement="bottom"
    @close="resultSheetOpen = false"
  >
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" text="Resultaat" dismiss-text="Sluit" @dismiss="resultSheetOpen = false"></nldd-top-title-bar>
      <ExecutionTraceView
        :result="lastResult"
        :trace-text="lastTraceText"
        :error="lastError"
        :expectations="lastExpectations"
        :scenario-name="lastScenarioName"
      />
    </nldd-page>
  </nldd-sheet>
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
  /* Fill the pane body. nldd-page's body is the only ancestor between us
   * and the viewport, so anchoring on viewport height minus the toolbar
   * + tab strip height gives a stable tall area regardless of how many
   * scenarios are loaded next door. */
  height: calc(100vh - 180px);
  padding: 16px;
  box-sizing: border-box;
}

.editor-yaml-textarea {
  flex: 1;
  width: 100%;
  min-height: 0;
  /* Match the library/zorgtoeslagwet/2 YamlView look: tinted background,
   * rounded corners, monospace, comfortable padding. The library version
   * is read-only <pre><code>; this is the editable counterpart with the
   * same skin so the eye doesn't have to context-switch. */
  background: var(--semantics-surfaces-tinted-background-color, #F4F6F9);
  color: var(--semantics-text-default-color, #1F2937);
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', 'JetBrains Mono', monospace;
  font-size: 13px;
  line-height: 1.5;
  padding: 16px;
  border: 1px solid var(--semantics-borders-default-color, #DDE0E4);
  border-radius: 12px;
  outline: none;
  resize: none;
  tab-size: 2;
  white-space: pre;
  overflow: auto;
}

.editor-yaml-textarea:focus {
  border-color: var(--semantics-borders-focus-color, #007BC7);
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
  margin-top: 8px;
  background: #fef2f2;
  color: #b91c1c;
  font-family: 'SF Mono', monospace;
  font-size: 12px;
  padding: 8px 12px;
  border: 1px solid #fecaca;
  border-radius: 6px;
}
</style>

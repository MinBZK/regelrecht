<script setup>
import { ref, computed, reactive, watch, watchEffect, nextTick } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import yaml from 'js-yaml';
import { useLaw, fetchLaw } from './composables/useLaw.js';
import { useEngine } from './composables/useEngine.js';
import { useAuth } from './composables/useAuth.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useColorScheme } from './composables/useColorScheme.js';
import { lastLibraryPath } from './composables/useLastVisitedRoute.js';
import { SUPPORT_EMAIL } from './constants.js';
import ArticleText from './components/ArticleText.vue';
import ArticleTextEditor from './components/ArticleTextEditor.vue';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import SearchPopover from './components/SearchPopover.vue';
import MachineReadable from './components/MachineReadable.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import ExecutionTraceView from './components/ExecutionTraceView.vue';
import LawGraphView from './components/LawGraphView.vue';

const { authenticated, loading: authLoading, oidcConfigured, person, login, logout } = useAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();
const { colorScheme, setColorScheme } = useColorScheme();

const colorSchemeOptions = [
  ['auto', 'Systeem'],
  ['light', 'Licht'],
  ['dark', 'Donker'],
];

const editorPanelFlags = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
  ['editor.article_text_edit', 'Tekst bewerken'],
];

// Per-pane view selection. Each pane independently picks one of the
// available views (Tekst, Machine, Scenario's, YAML). Same view can be
// in multiple panes (e.g. two YAML panes for diff). Layout always has
// one pane per view; nldd-side-by-side-split-view auto-hides panes
// from the right when the viewport is too narrow, so left = highest
// priority. Visibility flags via panel.* feature flags filter what's
// pickable in the menu.
const VIEW_DEFINITIONS = [
  { id: 'text', flag: 'panel.article_text', label: 'Tekst' },
  { id: 'machine', flag: 'panel.machine_readable', label: 'Machine' },
  { id: 'scenario', flag: 'panel.scenario_form', label: "Scenario's" },
  { id: 'yaml', flag: 'panel.yaml_editor', label: 'YAML' },
];

const availableViews = computed(() => VIEW_DEFINITIONS.filter(v => isEnabled(v.flag)));

function viewLabel(viewId) {
  return VIEW_DEFINITIONS.find(v => v.id === viewId)?.label ?? viewId;
}

const PANE_VIEWS_KEY = 'regelrecht-pane-views';
const paneViews = ref(loadPaneViews());

function loadPaneViews() {
  try {
    const stored = JSON.parse(localStorage.getItem(PANE_VIEWS_KEY) ?? 'null');
    // Only accept entries we still recognise — a stale value left over
    // from a removed view (e.g. 'form' before scenario was renamed) or
    // an externally injected string would otherwise produce a pane with
    // no v-if branch matching it, briefly flashing an empty body before
    // the availableViews watcher corrects it on the next tick.
    const knownIds = new Set(VIEW_DEFINITIONS.map(v => v.id));
    if (
      Array.isArray(stored) &&
      stored.length > 0 &&
      stored.every(v => typeof v === 'string' && knownIds.has(v))
    ) {
      return stored;
    }
  } catch {
    /* fall through to default */
  }
  return VIEW_DEFINITIONS.map(v => v.id);
}

// Every paneViews mutation goes through `paneViews.value = next`
// (setPaneView, the availableViews sync watcher), so the top-level ref
// identity always changes. No deep:true needed; in-place mutations are
// not used and shouldn't be relied on.
watch(paneViews, (val) => {
  localStorage.setItem(PANE_VIEWS_KEY, JSON.stringify(val));
});

// Sync paneViews with availableViews when a flag flips:
// - Drop panes whose view is no longer available (flag off → pane gone)
// - Append a pane for any view that was JUST enabled by this change
//   (in current available, not in previous) — so re-enabling a flag
//   brings the pane back instead of silently leaving the user without
//   it. The previous version compared "missing from paneViews" against
//   the full available set, which spuriously appended every available
//   view a user happened not to have open whenever any unrelated flag
//   flipped.
// When everything is filtered out, reset to one pane per available view
// as the safe default.
watch(availableViews, (views, oldViews) => {
  const allowedIds = new Set(views.map(v => v.id));
  const filtered = paneViews.value.filter(v => allowedIds.has(v));
  // On the very first run (immediate: true) oldViews is undefined; treat
  // it as "no diff" so we don't append every available view on top of
  // whatever the user had restored from localStorage.
  const previousIds = new Set((oldViews ?? []).map(v => v.id));
  const newlyEnabled = views
    .filter(v => oldViews !== undefined && !previousIds.has(v.id))
    .map(v => v.id);
  const next = [...filtered, ...newlyEnabled];
  if (next.length === 0 && views.length > 0) {
    paneViews.value = views.map(v => v.id);
    return;
  }
  if (
    next.length !== paneViews.value.length ||
    next.some((v, i) => v !== paneViews.value[i])
  ) {
    paneViews.value = next;
  }
}, { immediate: true });

function setPaneView(idx, viewId) {
  const next = [...paneViews.value];
  next[idx] = viewId;
  paneViews.value = next;
}

// All edit operations are gated behind SSO. When OIDC is configured the user
// must be authenticated; when OIDC is disabled the editor is fully open.
// In practice the `requiresAuth` router guard already awaits the auth-check
// and blocks unauthenticated users before this component mounts, so canEdit
// is always true here — the computed remains as a safety net.
const canEdit = computed(() => !oidcConfigured.value || authenticated.value);
// Tekst-pane is only editable when the user has write access AND the
// `editor.article_text_edit` flag is on. Visibility of the pane is
// controlled separately by `panel.article_text`.
const canEditArticleText = computed(() => canEdit.value && isEnabled('editor.article_text_edit'));

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
  lastSavedPr,
} = useLaw(route.params.lawId, route.params.articleNumber);

const resultSheetOpen = ref(false);
const graphSheetOpen = ref(false);

// --- Corpus search (reuse LibraryApp's SearchPopover) ---
const corpusLaws = ref([]);
const searchPopoverRef = ref(null);

async function loadCorpusLaws() {
  try {
    const res = await fetch('/api/corpus/laws?limit=1000');
    if (!res.ok) return;
    const list = await res.json();
    corpusLaws.value = list.sort((a, b) => a.law_id.localeCompare(b.law_id));
  } catch { /* ignore — search is a convenience */ }
}
loadCorpusLaws();

function openSearch(e, initialSearch = '') {
  searchPopoverRef.value?.show(e?.currentTarget, initialSearch);
}

/**
 * Display name for the failed law on the error inline-dialog. Tries the
 * corpus index (loaded for the search popover) first; falls back to the
 * URL slug so the user always sees a concrete identifier.
 */
const failedLawName = computed(() => {
  const id = lawId.value;
  if (!id) return '';
  return corpusLaws.value.find(l => l.law_id === id)?.name || id;
});

/**
 * Retry the failed law fetch. switchLaw clears `error` and re-runs the
 * fetch; failed responses don't enter the cache so a retry actually hits
 * the network again.
 */
function retryLoadLaw() {
  if (!lawId.value) return;
  switchLaw(lawId.value, selectedArticleNumber.value);
}

/**
 * Spotlight-style: any printable single-character keystroke on the bar's
 * search-field opens the popover with that character as the initial query.
 * preventDefault keeps the character out of the bar's own input — popover
 * shows it instead. Modifier-combos (Ctrl-A, Cmd-V, etc.), Tab, Enter,
 * arrows etc. fall through (length !== 1).
 */
function onBarSearchKeydown(e) {
  if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
    e.preventDefault();
    openSearch(e, e.key);
  }
}

function onSearchSelectLaw(lawIdVal) {
  // Open in the library — search currently only matches law names. As
  // soon as article-level search lands, we can route directly into the
  // editor (with the chosen article as the active tab).
  router.push(`/library/${encodeURIComponent(lawIdVal)}`);
}

async function onSearchHarvestAvailable(slug) {
  await fetch('/api/corpus/reload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ law_ids: [slug] }),
  }).catch(() => {});
  await loadCorpusLaws();
  router.push(`/library/${encodeURIComponent(slug)}`);
}
const resultSheetEl = ref(null);
watch(resultSheetOpen, async (open) => {
  await nextTick();
  if (open) resultSheetEl.value?.show();
  else resultSheetEl.value?.hide();
});
const graphSheetEl = ref(null);
watch(graphSheetOpen, async (open) => {
  await nextTick();
  if (open) graphSheetEl.value?.show();
  else graphSheetEl.value?.hide();
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
  // Sync the URL so deep-linking and browser back/forward stay in step.
  // `replace` (not `push`) keeps history clean — a tab switch isn't
  // navigation the user wants to undo with the back button.
  router.replace({
    name: 'editor',
    params: { lawId: tab.lawId, articleNumber: tab.articleNumber },
  });
}

// Browser back/forward (or any external navigation) — pull state from URL.
// Local mutations from selectTab already match the destination, so the
// guards below short-circuit; the work only happens for true URL changes.
onBeforeRouteUpdate(async (to) => {
  const newLawId = to.params.lawId;
  const newArticle = to.params.articleNumber;
  if (!newLawId) return;
  if (newLawId !== lawId.value) {
    const gen = ++switchGeneration;
    await switchLaw(newLawId, newArticle);
    if (gen !== switchGeneration) return;
    lawNames.value = { ...lawNames.value, [newLawId]: lawName.value };
  } else if (newArticle && String(newArticle) !== String(selectedArticleNumber.value)) {
    selectedArticleNumber.value = String(newArticle);
  }
});

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
// The scenario's entry output (e.g. "is_rechthebbende"). The graph view
// uses this to pin its "▶ start" marker to the right leaf.
const lastOutputName = ref(null);
// Loading state of the last scenario and a bound re-run callback, so the
// result sheet can show "running…" / an error with a reload action.
const lastRunning = ref(false);
const lastReload = ref(null);

function handleScenarioExecuted({ result, traceText, error, running, expectations, scenarioName, outputName, reload, view }) {
  lastResult.value = result;
  lastTraceText.value = traceText;
  lastError.value = error || null;
  lastRunning.value = !!running;
  lastReload.value = typeof reload === 'function' ? reload : null;
  lastExpectations.value = expectations || {};
  lastScenarioName.value = scenarioName || '';
  lastOutputName.value = outputName || null;
  // Open exactly one of the two sheets — opening the second on top of
  // the first would leave the previous one as a stale layer that
  // re-appears when the user dismisses the foreground sheet.
  if (view === 'graph') {
    resultSheetOpen.value = false;
    graphSheetOpen.value = true;
  } else {
    graphSheetOpen.value = false;
    resultSheetOpen.value = true;
  }
}

// Clear the captured trace whenever the active law changes — otherwise
// LawGraphView would re-flatten the old trace under the new lawId,
// misattribute every step to the new law, and pin the "▶ start" badge
// to a leaf that just happens to share the previous output's name.
// The trace and graph sheets close along with the trace they were
// showing: leaving them open over an empty graph or a fresh law the
// user just navigated into is more confusing than auto-dismissing.
watch(lawId, () => {
  lastResult.value = null;
  lastTraceText.value = null;
  lastError.value = null;
  lastRunning.value = false;
  lastReload.value = null;
  lastExpectations.value = {};
  lastScenarioName.value = '';
  lastOutputName.value = null;
  resultSheetOpen.value = false;
  graphSheetOpen.value = false;
});

// --- Editor state ---
const activeAction = ref(null);
// True while the open action sheet is for a freshly added action, so its
// Save button is always offered (no edit required to create it).
const activeActionIsNew = ref(false);
const activeEditItem = ref(null);
const parseError = ref(null);

const machineReadable = ref(null);
const yamlSource = ref('');
// In-memory markdown for the currently selected article's `text` field.
// Seeded on article switch alongside machineReadable so the Tekst and Machine
// panes reset in lockstep when the user tabs to a different article.
const editedText = ref('');

// Per-pane refs to the ArticleTextEditor instance so the pane-header can
// render the formatting toolbar (Bold/Italic/lists) next to the existing
// pane-view dropdown rather than the editor drawing its own duplicate
// label dropdown inside the body. Functional ref keeps the map in sync as
// panes mount/unmount.
const textEditorRefs = reactive({});
function setTextEditorRef(idx) {
  return (el) => {
    if (el) {
      textEditorRefs[idx] = el;
    } else {
      delete textEditorRefs[idx];
    }
  };
}

const dumpOpts = { lineWidth: 80, noRefs: true };

watch(selectedArticle, (article) => {
  activeAction.value = null;
  activeEditItem.value = null;
  const mr = article?.machine_readable;
  machineReadable.value = mr ? structuredClone(mr) : null;
  yamlSource.value = mr ? yaml.dump(mr, dumpOpts) : '';
  editedText.value = article?.text ?? '';
  parseError.value = null;
}, { immediate: true });

// Reflect navigation depth in the document title:
//   "Editor: Art. 5 · Wet op de zorgtoeslag · RegelRecht"
// Tab name first (the high-level location), then most-specific to least-
// specific so browser tab truncation preserves the article number.
// Always set (no early return) — router.afterEach used to set a static
// fallback but it raced with this effect on tab/article switches.
watchEffect(() => {
  const detail = [];
  if (selectedArticle.value) detail.push(`Art. ${selectedArticle.value.number}`);
  if (lawName.value) detail.push(lawName.value);
  document.title = detail.length > 0
    ? `Editor: ${detail.join(' · ')} · RegelRecht`
    : 'Editor · RegelRecht';
});

const editedArticle = computed(() => {
  if (!selectedArticle.value) return null;
  return {
    ...selectedArticle.value,
    text: editedText.value,
    machine_readable: machineReadable.value,
  };
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
  if (!selectedArticle.value) return rawYaml.value;
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
    // Short-circuit when neither pane has been touched: a `yaml.dump`
    // round-trip can cosmetically reformat the parsed doc (key ordering,
    // string quoting), and `handleActionSave` calls `saveLaw(currentLawYaml)`
    // without checking dirty flags, so a no-op action-modal save on a
    // pristine article would otherwise produce a reformat-only commit.
    const baseArticle = docArticles[idx];
    const textPristine = editedText.value === (baseArticle.text ?? '');
    const baselineMr = baseArticle.machine_readable ?? null;
    const currentMr = machineReadable.value ?? null;
    const mrPristine = baselineMr === currentMr
      || JSON.stringify(baselineMr) === JSON.stringify(currentMr);
    if (textPristine && mrPristine) return rawYaml.value;
    // Only splice fields that have diverged from the base — passing
    // `machineReadable.value` verbatim when it's null would erase the
    // article's machine_readable from the serialized doc, and similarly
    // for text. The dirty computeds below drive this same contract.
    const patched = { ...baseArticle };
    if (!textPristine) {
      patched.text = editedText.value;
    }
    if (machineReadable.value != null) {
      patched.machine_readable = machineReadable.value;
    } else if (baselineMr != null) {
      // The user opened an article that had machine_readable and cleared the
      // YAML editor. The spread above carried the original key over; we have
      // to drop it explicitly so the save persists the deletion rather than
      // silently round-tripping the original content.
      delete patched.machine_readable;
    }
    docArticles[idx] = patched;
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

const isArticleTextDirty = computed(() => {
  if (!selectedArticle.value) return false;
  return (selectedArticle.value.text ?? '') !== (editedText.value ?? '');
});

// Tracks which pane(s) had dirty edits at the time of the most recent save
// attempt. Used to scope `lawSaveError` to the pane that actually triggered
// the save — without this, a save initiated from the machine pane that
// fails would surface the "Opslaan mislukt" dialog inside the text-pane
// body too, blaming a pane the user didn't touch.
const lastSaveTouchedText = ref(false);
const lastSaveTouchedMachine = ref(false);

// Single save handler shared by the Tekst and Machine panes. The PUT writes
// the whole law YAML, so one click persists every in-memory edit for the
// selected article regardless of which pane surfaced the button.
async function handleLawSave() {
  const lawYaml = currentLawYaml.value;
  if (!lawYaml) return;
  // Snapshot the law id before the await. saveLaw itself guards its own
  // reactive writes with the same check, but the post-save cleanup below
  // runs in the EditorApp scope and would happily overwrite the new law's
  // in-progress edits with its pristine article data if the user switched
  // laws mid-flight.
  const savedLawId = lawId.value;
  lastSaveTouchedText.value = isArticleTextDirty.value;
  lastSaveTouchedMachine.value = isMachineReadableDirty.value;
  try {
    await saveLaw(lawYaml);
    if (lawId.value !== savedLawId) return; // law switched mid-PUT
    // points at the re-parsed article. The `watch(selectedArticle)` above
    // fires on the next microtask — leaving a window where the dirty
    // computeds still see the pre-save values and the save button stays
    // enabled, enabling a double-save click. Reset local state explicitly
    // from the freshly-parsed article so both dirty flags clear
    // synchronously with the save.
    const fresh = selectedArticle.value;
    const freshMr = fresh?.machine_readable ?? null;
    machineReadable.value = freshMr ? structuredClone(freshMr) : null;
    yamlSource.value = freshMr ? yaml.dump(freshMr, dumpOpts) : '';
    editedText.value = fresh?.text ?? '';
    // Successful save — the dialog flags drop back to false.
    lastSaveTouchedText.value = false;
    lastSaveTouchedMachine.value = false;
  } catch (e) {
    // saveError is surfaced via lawSaveError; log for dev visibility.
    console.warn('saveLaw failed:', e);
  }
}

// Per-pane scoped views of lawSaveError. The error is only visible in the
// pane that contributed to the failing save, even though the underlying
// failure (and lawSaveError itself) is the same for both panes.
const articleTextSaveError = computed(() =>
  lastSaveTouchedText.value ? lawSaveError.value : null,
);
const machineReadableSaveError = computed(() =>
  lastSaveTouchedMachine.value ? lawSaveError.value : null,
);

// Alias kept to minimise template churn; both panes ultimately call the
// same whole-law save.
const handleMachineReadableSave = handleLawSave;

function onYamlInput(event) {
  // nldd-code-editor dispatches a CustomEvent with the new value in
  // event.detail.value (see the design-system 0.8.41 component). The
  // host's `value` property is updated before dispatch so
  // event.target.value would also work, but reading from detail keeps
  // the contract explicit and matches how the storybook docs the API.
  // `??` skips only null/undefined — a deliberate empty string passes
  // through as a valid "user cleared the editor" input.
  const text = event.detail?.value ?? event.target?.value;
  if (text == null) {
    // Structurally broken event (no detail, no value on target). Don't
    // touch yamlSource — silently overwriting with the previous value
    // would swallow keystrokes a user can see in the textarea but
    // never sees committed to the reactive source. Surface it loudly
    // instead so the regression is obvious in dev console.
    console.warn('onYamlInput: unexpected event shape, ignoring', event);
    return;
  }
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
  } else if (section === 'produces') {
    if (!mr.execution.produces) mr.execution.produces = {};
    if (data == null) delete mr.execution.produces[key];
    else mr.execution.produces[key] = data;
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
  activeActionIsNew.value = true;
  activeAction.value = newAction;
}

function handleOpenAction(action) {
  actionSnapshot = JSON.stringify(machineReadable.value);
  activeActionIsNew.value = false;
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
async function handleActionSave() {
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
  // Commit the in-place mutations, then actually persist the law. The
  // sheet's "Opslaan" is a real save, so the Machine pane won't show a
  // separate dirty/save affordance for sheet edits. On a failed PUT the
  // edits stay in the model and the Machine pane's normal dirty/save
  // affordance is the fallback — no data loss either way.
  machineReadable.value = JSON.parse(JSON.stringify(machineReadable.value));
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
  // Close the sheet unconditionally: the mutations are already committed
  // above, so even if handleLawSave() throws the sheet must not stay open
  // showing a now-clean (isDirty === false) state. A failed PUT falls back
  // to the Machine pane's normal dirty/save affordance — no data loss.
  try {
    await handleLawSave();
  } finally {
    actionSnapshot = null;
    activeAction.value = null;
  }
}

</script>

<template>
  <nldd-app-view>
    <nldd-bar-split-view>
      <!-- Primary Bar: App Toolbar + Document Tabs (md+) -->
      <!-- Primary Bar: md only — search and settings as buttons -->
      <nldd-split-view-pane slot="primary-bar-md" only="md" no-divider>
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md">
                <nldd-tab-bar-item :href="lastLibraryPath" @click.prevent="router.push(lastLibraryPath)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item v-if="lastSavedPr" slot="end">
              <!-- Federated write-back indicator. Stays visible across pane
                   switches so the user always knows where their edits are
                   accumulating. New tab so the editor state isn't lost. -->
              <nldd-button
                size="md"
                start-icon="external-link"
                :text="`PR #${lastSavedPr.number}`"
                :href="lastSavedPr.url"
                target="_blank"
                rel="noopener"
              ></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="md" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button id="settings-menu-btn-md" size="md" start-icon="global-settings" text="Instellingen" expandable popovertarget="settings-menu-md"></nldd-button>
              <nldd-menu id="settings-menu-md" anchor="settings-menu-btn-md">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in editorPanelFlags"
                  :key="key"
                  type="checkbox"
                  :selected="isEnabled(key) || undefined"
                  :text="label"
                  @select="toggleFlag(key)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-group text="Thema">
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-md-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Primary Bar: lg+ — search as input, settings as button -->
      <nldd-split-view-pane slot="primary-bar-lg" above="lg" no-divider>
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md">
                <nldd-tab-bar-item :href="lastLibraryPath" @click.prevent="router.push(lastLibraryPath)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="center" min-width="240px" width="33%">
              <nldd-search-field
                size="md"
                placeholder="Zoeken"
                @click="openSearch"
                @keydown="onBarSearchKeydown"
              ></nldd-search-field>
            </nldd-toolbar-item>
            <nldd-toolbar-item v-if="lastSavedPr" slot="end">
              <!-- Same federated write-back indicator as the md toolbar. -->
              <nldd-button
                size="md"
                start-icon="external-link"
                :text="`PR #${lastSavedPr.number}`"
                :href="lastSavedPr.url"
                target="_blank"
                rel="noopener"
              ></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button id="settings-menu-btn-lg" size="md" start-icon="global-settings" text="Instellingen" expandable popovertarget="settings-menu-lg"></nldd-button>
              <nldd-menu id="settings-menu-lg" anchor="settings-menu-btn-lg">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in editorPanelFlags"
                  :key="key"
                  type="checkbox"
                  :selected="isEnabled(key) || undefined"
                  :text="label"
                  @select="toggleFlag(key)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-group text="Thema">
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-lg-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Document Tab Bar (md+) -->
      <nldd-split-view-pane slot="document-tabs" sm-order="2">
        <nldd-container padding-inline="8" padding-top="4" padding-bottom="8" sm-padding-top="8" sm-padding-bottom="0">
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
        <!-- Empty state: no tabs open. The CTA points back to the library
             since that's the only way to create new tabs; mention the tab
             bar too because closed tabs may still be visible alongside this
             empty state on the next pane. -->
        <nldd-page v-if="!activeTab">
          <nldd-simple-section width="full">
            <nldd-inline-dialog text="Open een artikel vanuit de tabbalk of de bibliotheek om te bewerken.">
              <nldd-button slot="actions" variant="secondary" text="Ga naar bibliotheek" :href="lastLibraryPath" @click.prevent="router.push(lastLibraryPath)"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- Error state — mirrors the library's law-load failure pattern. -->
        <nldd-page v-else-if="error">
          <nldd-simple-section width="full">
            <nldd-inline-dialog
              variant="alert"
              :text="`${failedLawName} is niet geladen`"
              supporting-text="De gegevens konden niet worden opgehaald."
            >
              <nldd-button slot="actions" variant="primary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
              <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- All editor flags off — paneViews is empty so the
             side-by-side view would render zero pane slots. Surface an
             explicit empty-state with a CTA to the settings menu so
             the user understands the editor isn't broken. -->
        <nldd-page v-else-if="paneViews.length === 0">
          <nldd-simple-section width="full">
            <nldd-inline-dialog text="Geen editors actief. Schakel ten minste één editor in via Instellingen."></nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- One pane per entry in `paneViews`. Each pane independently
             picks its view via the dropdown in its header. The split-view
             auto-hides panes from the right when the viewport is too narrow.
             Hidden panes stay in the DOM so state is preserved when the
             viewport widens. -->
        <nldd-side-by-side-split-view v-else :panes="String(paneViews.length)">
          <!-- Compound key: when a flag flip shifts which view sits at a
               given index, Vue would otherwise patch the existing pane in
               place — leaking ScenarioBuilder form state and engine
               results into a different view. Re-keying on the view id
               forces an unmount + remount on identity change, at the
               (acceptable) cost of losing pane scroll position. -->
          <nldd-split-view-pane
            v-for="(view, idx) in paneViews"
            :key="`${view}-${idx}`"
            :slot="`pane-${idx + 1}`"
          >
            <nldd-page
              sticky-header
              :background="view === 'scenario' ? 'tinted' : undefined"
              :sticky-footer="(view === 'machine' && canEdit && !activeAction && (isMachineReadableDirty || lawSaving) && paneViews.indexOf('machine') === idx) || (view === 'text' && canEditArticleText && (isArticleTextDirty || lawSaving) && paneViews.indexOf('text') === idx)"
            >
              <div slot="header" class="pane-header">
                <nldd-button
                  :id="`pane-view-btn-${idx}`"
                  size="md"
                  expandable
                  :text="viewLabel(view)"
                  :popovertarget="`pane-view-menu-${idx}`"
                ></nldd-button>
                <nldd-menu :id="`pane-view-menu-${idx}`" :anchor="`pane-view-btn-${idx}`">
                  <nldd-menu-item
                    v-for="opt in availableViews"
                    :key="opt.id"
                    type="radio"
                    :selected="view === opt.id || undefined"
                    :text="opt.label"
                    @select="setPaneView(idx, opt.id)"
                  ></nldd-menu-item>
                </nldd-menu>
                <!-- Formatting toolbar lives in the pane-header so it sits in
                     line with the pane-view dropdown rather than below it.
                     Wired to the ArticleTextEditor instance via textEditorRefs
                     so the active-format chips update in lockstep with the
                     editor's selection. -->
                <div
                  v-if="view === 'text' && selectedArticle && textEditorRefs[idx]"
                  class="fmt-group"
                  data-testid="article-text-fmt-group"
                >
                  <span class="fmt-btn" :class="{ 'is-active': textEditorRefs[idx].activeFormats.bold }">
                    <nldd-icon-button
                      icon="bold"
                      size="md"
                      accessible-label="Vet"
                      data-testid="fmt-bold"
                      :disabled="!canEditArticleText || undefined"
                      @click="textEditorRefs[idx].toggleBold()"
                    ></nldd-icon-button>
                  </span>
                  <span class="fmt-btn" :class="{ 'is-active': textEditorRefs[idx].activeFormats.italic }">
                    <nldd-icon-button
                      icon="italic"
                      size="md"
                      accessible-label="Schuin"
                      data-testid="fmt-italic"
                      :disabled="!canEditArticleText || undefined"
                      @click="textEditorRefs[idx].toggleItalic()"
                    ></nldd-icon-button>
                  </span>
                  <span class="fmt-divider" role="separator" aria-orientation="vertical"></span>
                  <span class="fmt-btn" :class="{ 'is-active': textEditorRefs[idx].activeFormats.bulletList }">
                    <nldd-icon-button
                      icon="bullet-list"
                      size="md"
                      accessible-label="Opsomming"
                      data-testid="fmt-bullet-list"
                      :disabled="!canEditArticleText || undefined"
                      @click="textEditorRefs[idx].toggleBulletList()"
                    ></nldd-icon-button>
                  </span>
                  <span class="fmt-btn" :class="{ 'is-active': textEditorRefs[idx].activeFormats.orderedList }">
                    <nldd-icon-button
                      icon="numbered-list"
                      size="md"
                      accessible-label="Genummerde lijst"
                      data-testid="fmt-ordered-list"
                      :disabled="!canEditArticleText || undefined"
                      @click="textEditorRefs[idx].toggleOrderedList()"
                    ></nldd-icon-button>
                  </span>
                </div>
                <span v-if="view === 'yaml' && parseError" class="editor-parse-error">YAML parse error</span>
              </div>

              <!-- Tekst — WYSIWYG editor when the editor.article_text_edit
                   feature flag is on, otherwise the read-only ArticleText
                   display (matches the pre-#589 look). The toolbar in the
                   pane-header above guards on `textEditorRefs[idx]`, which
                   is only populated by the WYSIWYG component, so it auto-
                   hides when the flag is off. -->
              <nldd-simple-section v-if="view === 'text'" width="full">
                <ArticleTextEditor
                  v-if="canEditArticleText"
                  :ref="setTextEditorRef(idx)"
                  :article="selectedArticle"
                  :editable="canEditArticleText"
                  :save-error="articleTextSaveError"
                  :model-value="editedText"
                  @update:model-value="editedText = $event"
                />
                <ArticleText v-else :article="selectedArticle" />
              </nldd-simple-section>
              <!-- Footer + Save button only on the first text pane (mirrors the machine pattern). -->
              <nldd-container
                v-if="view === 'text' && canEditArticleText && (isArticleTextDirty || lawSaving) && paneViews.indexOf('text') === idx"
                slot="footer"
                padding="16"
              >
                <nldd-button
                  variant="primary"
                  size="md"
                  width="full"
                  data-testid="save-text-btn"
                  :disabled="lawSaving || undefined"
                  :text="lawSaving ? 'Opslaan…' : 'Opslaan'"
                  @click="handleLawSave"
                ></nldd-button>
              </nldd-container>

              <!-- Machine readable -->
              <nldd-simple-section v-else-if="view === 'machine'" width="full">
                <MachineReadable
                  :article="editedArticle"
                  :editable="canEdit"
                  :dirty="isMachineReadableDirty"
                  :saving="lawSaving"
                  :save-error="machineReadableSaveError"
                  @open-action="handleOpenAction"
                  @open-edit="activeEditItem = $event"
                  @init-mr="handleInitMr"
                  @add-action="handleAddAction"
                  @save="handleMachineReadableSave"
                  @patch="handleSave"
                  @delete="handleDelete"
                />
              </nldd-simple-section>
              <!-- Footer + Save button only on the first machine pane.
                   Duplicates would render redundant Save buttons over
                   the same shared dirty state — not broken, just noisy.
                   Hidden while the action sheet is open: the sheet's
                   "Opslaan" is the real save for those in-place edits, so a
                   second Machine-pane Save button behind it just distracts. -->
              <nldd-container
                v-if="view === 'machine' && canEdit && !activeAction && (isMachineReadableDirty || lawSaving) && paneViews.indexOf('machine') === idx"
                slot="footer"
                padding="16"
              >
                <nldd-button
                  variant="primary"
                  size="md"
                  width="full"
                  data-testid="save-mr-btn"
                  :disabled="lawSaving || undefined"
                  :text="lawSaving ? 'Opslaan…' : 'Opslaan'"
                  @click="handleMachineReadableSave"
                ></nldd-button>
              </nldd-container>

              <!-- Scenario builder -->
              <template v-else-if="view === 'scenario'">
                <nldd-simple-section v-if="engineInitError" width="full">
                  <nldd-inline-dialog
                    variant="alert"
                    text="WASM engine niet geladen"
                    :supporting-text="`${engineInitError.message} — voer 'just wasm-build' uit om de WASM module te bouwen.`"
                  ></nldd-inline-dialog>
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
              </template>

              <!-- YAML -->
              <nldd-simple-section v-else-if="view === 'yaml'" width="full">
                <nldd-code-editor
                  resize="none"
                  accessible-label="YAML"
                  :value="yamlSource"
                  @input="onYamlInput"
                ></nldd-code-editor>
                <div v-if="parseError" class="editor-parse-error-detail">{{ parseError }}</div>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>
        </nldd-side-by-side-split-view>
      </nldd-split-view-pane>

      <!-- Mobile Bar (sm only): tab bar + icon-buttons for search and settings -->
      <nldd-split-view-pane slot="mobile-bar" only="sm">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar compact>
                <nldd-tab-bar-item :href="lastLibraryPath" @click.prevent="router.push(lastLibraryPath)" icon="books" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected icon="edit" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <span>
                <nldd-icon-button size="lg" icon="search" text="Zoeken" @click="openSearch"></nldd-icon-button>
              </span>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <span>
                <nldd-icon-button id="settings-menu-btn-sm" size="lg" icon="global-settings" text="Instellingen" popovertarget="settings-menu-sm"></nldd-icon-button>
              </span>
              <nldd-menu id="settings-menu-sm" anchor="settings-menu-btn-sm">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <nldd-menu-group text="Functies">
                <nldd-menu-item
                  v-for="[key, label] in editorPanelFlags"
                  :key="key"
                  type="checkbox"
                  :selected="isEnabled(key) || undefined"
                  :text="label"
                  @select="toggleFlag(key)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-group text="Thema">
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-sm-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
                <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-app-view>

  <ActionSheet :action="activeAction" :article="editedArticle" :editable="canEdit" :is-new="activeActionIsNew" @close="handleActionClose" @save="handleActionSave" />
  <EditSheet :item="activeEditItem" :article="editedArticle" @save="handleSave" @close="activeEditItem = null" />
  <SearchPopover
    ref="searchPopoverRef"
    :laws="corpusLaws"
    @select-law="onSearchSelectLaw"
    @harvest-available="onSearchHarvestAvailable"
  />

  <!-- Trace sheet — execution trace + expected outcomes for the most
       recently executed scenario. Opened from a scenario card's "Toon
       resultaat" button. -->
  <nldd-sheet
    ref="resultSheetEl"
    placement="bottom"
    @close="resultSheetOpen = false"
  >
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" :text="lastScenarioName ? `Resultaat: ${lastScenarioName}` : 'Resultaat'" collapse-anchor="result-scenario-title" dismiss-text="Sluit" @dismiss="resultSheetOpen = false"></nldd-top-title-bar>
      <nldd-simple-section width="full">
        <nldd-title id="result-scenario-title" size="4"><h3>{{ lastScenarioName ? `Resultaat: ${lastScenarioName}` : 'Resultaat' }}</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>
        <ExecutionTraceView
          :result="lastResult"
          :trace-text="lastTraceText"
          :error="lastError"
          :running="lastRunning"
          :expectations="lastExpectations"
          :can-reload="!!lastReload"
          @reload="lastReload && lastReload()"
        />
      </nldd-simple-section>
    </nldd-page>
  </nldd-sheet>

  <!-- Graph sheet — visual law graph with the scenario's trace overlay.
       Opened from a scenario card's "Graaf" button. -->
  <nldd-sheet
    ref="graphSheetEl"
    placement="bottom"
    @close="graphSheetOpen = false"
  >
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" :text="lastScenarioName ? `Graaf: ${lastScenarioName}` : 'Graaf'" dismiss-text="Sluit" @dismiss="graphSheetOpen = false"></nldd-top-title-bar>
      <!-- Lazy-mount: building the Vue Flow graph for laws with many
           cross-law references is non-trivial and the sheet is hidden
           by default. Render only while the sheet is open; the remount
           cost on each subsequent open is acceptable next to dragging
           an unused Vue Flow tree behind every editor session. -->
      <LawGraphView
        v-if="graphSheetOpen"
        :law-id="lawId"
        :result="lastResult"
        :output-name="lastOutputName"
        :expectations="lastExpectations"
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

/* Per-pane header: view-picker dropdown sits where the title would
   normally be, with the YAML parse-error pill floating after it.
   Mirrors nldd-top-title-bar's compact spacing so the row height
   matches other panes' headers. */
.pane-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  min-height: 56px;
  box-sizing: border-box;
}

/* Formatting buttons embedded in the text-pane header. The nldd-icon-button
 * library doesn't carry a pressed state of its own, so the wrapper span
 * paints the active background locally — same approach the previous
 * in-component toolbar used. */
.fmt-group {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.fmt-btn {
  display: inline-flex;
  border-radius: 8px;
  transition: background-color 120ms ease;
}

.fmt-btn.is-active {
  background-color: var(--semantics-surfaces-accent-tinted-background-color, rgba(0, 123, 199, 0.14));
}

.fmt-divider {
  display: inline-block;
  width: 1px;
  height: 20px;
  margin: 0 4px;
  background-color: var(--semantics-borders-default-color, #DDE0E4);
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

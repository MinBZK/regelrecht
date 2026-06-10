<script setup>
import { ref, computed, reactive, watch, watchEffect, nextTick, onBeforeUnmount } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import yaml from 'js-yaml';
import { useLaw, fetchLaw } from './composables/useLaw.js';
import { lawsListUrl } from './composables/corpusUrls.js';
import { useEngine } from './composables/useEngine.js';
import { useAuth } from './composables/useAuth.js';
import { useTrajects } from './composables/useTrajects.js';
import TrajectMenu from './components/TrajectMenu.vue';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useNotes, useResolvedDraftNotes } from './composables/useNotes.js';
import { useDraftNotes } from './composables/useDraftNotes.js';
import { useColorScheme } from './composables/useColorScheme.js';
import { lastLibraryPath, sectionTarget } from './composables/useLastVisitedRoute.js';
import { SUPPORT_EMAIL } from './constants.js';
import ArticleText from './components/ArticleText.vue';
import AnnotatedText from './components/AnnotatedText.vue';
import ArticleTextEditor from './components/ArticleTextEditor.vue';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import SearchPopover from './components/SearchPopover.vue';
import MachineReadable from './components/MachineReadable.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import TrajectDocuments from './components/TrajectDocuments.vue';
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
  // Capability gate: when on, the Tekst pane offers a "Notities"
  // toggle that overlays resolved notes on the article text. Not a
  // separate pane (notes are a layer over the text, not other content).
  ['panel.notes', 'Notities'],
  // Note authoring (RFC-018 write path). Separate gate from panel.notes so
  // notes can be shown read-only without exposing the (MVP, local-only)
  // creation + export flow.
  ['notes.create', 'Notities aanmaken'],
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

// All edit operations are gated behind SSO + an active traject. The
// editor renders in two shapes:
//   - `/editor/{lawId?}/{articleNumber?}`         → read-only view,
//     `canEdit` is false; save buttons disabled.
//   - `/editor/{trajectRef}/{lawId?}/{article?}`  → full edit mode,
//     `canEdit` is true; writes land in that traject's branch.
// Pick a traject in the TrajectMenu to flip from the first shape to
// the second.
const { activeTrajectRef } = useTrajects();
const canEdit = computed(
  () => (!oidcConfigured.value || authenticated.value) && activeTrajectRef.value !== null,
);
// Tekst-pane is only editable when the user has write access AND the
// `editor.article_text_edit` flag is on. Visibility of the pane is
// controlled separately by `panel.article_text`.
const canEditArticleText = computed(() => canEdit.value && isEnabled('editor.article_text_edit'));

const route = useRoute();
const router = useRouter();

// Bibliotheek tab / "naar bibliotheek" buttons: restore the last library
// position but re-stamp it with the currently active traject, so the
// traject survives the Editor→Bibliotheek switch (it lives in the URL).
const libraryTabTarget = computed(() =>
  sectionTarget(router, lastLibraryPath.value, activeTrajectRef.value),
);
const libraryTabHref = computed(() => router.resolve(libraryTabTarget.value).href);

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
} = useLaw(route.params.lawId, route.params.articleNumber, route.params.trajectRef);

// When the active traject changes (router.push to /editor/{otherRef}/…)
// the URL stays on the same component; refresh the corpus index and
// re-fetch the open law through the new traject's backends. `switchLaw`
// crosses trajects too via its third argument so the law cache key
// stays correct.
//
// Also flush the WASM engine: it caches loaded laws by id only, so
// without this a scenario run after a traject switch would evaluate
// the open law against the *previous* traject's dependencies. The
// dependency walker re-loads on demand on the next run, so a single
// `unloadAllLaws` is enough — no per-dep bookkeeping needed.
watch(activeTrajectRef, (next) => {
  unloadAllLaws();
  loadCorpusLaws();
  if (lawId.value) {
    switchLaw(lawId.value, selectedArticleNumber.value, next);
  }
});

// Notes (RFC-005/RFC-018) for the current law, resolved against its text.
const {
  notesForArticle: committedNotesForArticle,
  issues: noteIssues,
  loading: notesLoading,
  error: notesError,
  reload: reloadNotes,
} = useNotes(lawId, selectedArticle, activeTrajectRef);

// Notes are a layer over the Tekst pane, not a separate pane. This toggle is
// the user's "show notes now" preference; panel.notes (a feature flag) is the
// capability gate that makes the toggle available at all. Persisted so it
// survives navigation within a session.
const NOTES_TOGGLE_KEY = 'regelrecht-show-notes';
const showNotes = ref(localStorage.getItem(NOTES_TOGGLE_KEY) === '1');
watch(showNotes, (v) => {
  try { localStorage.setItem(NOTES_TOGGLE_KEY, v ? '1' : '0'); } catch { /* ignore */ }
});
// Notes overlay only makes sense in read mode: the resolver matches raw text,
// so it cannot align with the markdown render or the editable textarea.
const notesActive = computed(
  () => isEnabled('panel.notes') && showNotes.value && !canEditArticleText.value,
);

// Note authoring (RFC-018 write path). Draft notes live in localStorage per
// law until exported; they resolve and highlight live alongside committed
// ones, so the author sees the new note anchored immediately. The WASM engine
// is handed to NoteCreator for selector-uniqueness checks; useNotes already
// initialises it, this just exposes the instance once ready (null until then,
// NoteCreator guards on it).
const noteEngine = ref(null);
useEngine()
  .initEngine()
  .then((e) => {
    noteEngine.value = e;
  })
  .catch(() => {});
const {
  drafts: draftNotes,
  draftCount,
  addDraft,
  clearDrafts,
  exportYaml,
  saveToRepo,
} = useDraftNotes(lawId, activeTrajectRef);
const { draftNotesForArticle } = useResolvedDraftNotes(
  draftNotes,
  lawId,
  selectedArticle,
  activeTrajectRef,
);
const canCreateNotes = computed(
  () => isEnabled('notes.create') && notesActive.value,
);
// Committed + draft notes share the highlight path. Draft entries already
// carry __draft so the popover can mark them unsaved.
const notesForArticle = computed(() => [
  ...committedNotesForArticle.value,
  ...draftNotesForArticle.value,
]);

function onCreateNote(note) {
  addDraft(note);
}

// Wiping drafts is irreversible (local-only until exported), so it goes
// through a confirm modal. nldd-modal-dialog's API is show()/hide() (there
// is no close()); a flag drives those via a watch, and @close just clears
// the flag — same pattern as MachineReadable's delete confirm, which avoids
// the hide() -> @close -> hide() recursion.
const clearDraftsModalEl = ref(null);
const clearDraftsPending = ref(false);
watch(clearDraftsPending, (open) => {
  const el = clearDraftsModalEl.value;
  if (!el) return;
  if (open && typeof el.show === 'function') el.show();
  else if (!open && typeof el.hide === 'function') el.hide();
});
function askClearDrafts() {
  clearDraftsPending.value = true;
}
function cancelClearDrafts() {
  if (clearDraftsPending.value === false) return; // idempotent: @close + button
  clearDraftsPending.value = false;
}
function confirmClearDrafts() {
  clearDrafts();
  clearDraftsPending.value = false;
}

const exporting = ref(false);
async function exportNotes() {
  if (exporting.value) return; // a second click would download a duplicate
  exporting.value = true;
  let url;
  try {
    const text = await exportYaml();
    const blob = new Blob([text], { type: 'text/yaml' });
    url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'annotations.yaml';
    a.click();
  } finally {
    // Revoke on a later tick: a programmatic anchor download starts
    // asynchronously, and revoking synchronously after click() races the
    // browser fetching the blob (unreliable in Safari/Firefox).
    if (url) setTimeout(() => URL.revokeObjectURL(url), 0);
    exporting.value = false;
  }
}

// Note write-back: PUT the new drafts to editor-api, which appends them to
// the sidecar on the active traject's branch (same write model as law and
// scenario edits since #632). No source picker — the traject's own corpus
// config decides where the notes land. The "PR #N" toolbar badge is driven
// by the shared lastSavedPr ref, so a save that produced a PR lights it up
// with no extra wiring here.
const savingNotes = ref(false);
const notesSaveError = ref(null);
// Explicit success signal: a PR-less / NoChange save must not look like
// the work vanished (the drafts get cleared either way).
const notesSaveStatus = ref(null);
// The save status/error describe the LAST save. A NEW draft appearing
// after that save makes the confirmation stale ("Opslaan gelukt" next to
// "1 concept, nog niet opgeslagen" is contradictory), so clear it then.
// But a successful save itself drains drafts to zero — clearing on a
// DECREASE would wipe the very confirmation `saveNotesToRepo` is about to
// set, a race that previously only worked by microtask-ordering luck.
// Only react to an increase; the count going down is the save completing.
watch(draftCount, (count, prev) => {
  if (count > prev) {
    notesSaveStatus.value = null;
    notesSaveError.value = null;
  }
});
async function saveNotesToRepo() {
  if (savingNotes.value) return;
  savingNotes.value = true;
  notesSaveError.value = null;
  notesSaveStatus.value = null;
  try {
    const { pr, noChange } = await saveToRepo();
    if (noChange) {
      notesSaveStatus.value = 'Notities waren al opgeslagen.';
    } else if (pr) {
      notesSaveStatus.value = `Opgeslagen in PR #${pr.number}.`;
    } else {
      notesSaveStatus.value = 'Notities opgeslagen.';
    }
    // After save, drafts are cleared but useNotes still serves the
    // pre-save cached resolution (typically []). Force a refetch so
    // the just-committed notes show up immediately instead of only
    // after the user navigates away and back. NoChange also refetches
    // — it's cheap and keeps the post-save state consistent.
    await reloadNotes();
  } catch (e) {
    notesSaveError.value = e?.message || 'Opslaan mislukt';
  } finally {
    savingNotes.value = false;
  }
}

const resultSheetOpen = ref(false);
const graphSheetOpen = ref(false);

// --- Corpus search (reuse LibraryApp's SearchPopover) ---
const corpusLaws = ref([]);
const searchPopoverRef = ref(null);

// Generation counter to discard stale responses across rapid traject switches.
let corpusLawsGeneration = 0;

async function loadCorpusLaws() {
  const gen = ++corpusLawsGeneration;
  try {
    const res = await fetch(lawsListUrl(activeTrajectRef.value, 'limit=1000'));
    if (!res.ok) return;
    if (gen !== corpusLawsGeneration) return; // stale response, discard
    const list = await res.json();
    if (gen !== corpusLawsGeneration) return; // stale response, discard
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

// True when the law fetch errored with 404 (law missing or not in active traject's corpus).
const lawErrorIs404 = computed(() => error.value?.status === 404);

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
  router.push(libraryRouteFor(lawIdVal));
}

async function onSearchHarvestAvailable(slug) {
  await fetch('/api/corpus/reload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ law_ids: [slug] }),
  }).catch(() => {});
  await loadCorpusLaws();
  router.push(libraryRouteFor(slug));
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

/**
 * Build a router target for the editor that preserves the current
 * traject scope. With a traject in the URL the user stays in
 * `editor-traject` (full edit mode). Without one — which should not
 * occur while EditorApp is mounted, since the editor requires a
 * traject — fall back to the traject chooser, carrying the law as
 * query so it opens right after a traject is picked.
 */
function editorRouteFor(lawIdVal, articleNumber) {
  const trajectRef = route.params.trajectRef;
  if (trajectRef) {
    return {
      name: 'editor-traject',
      params: { trajectRef, lawId: lawIdVal, articleNumber },
    };
  }
  return {
    name: 'editor',
    query: { law: lawIdVal || undefined, article: articleNumber || undefined },
  };
}

/**
 * Build a router target for the bibliotheek that preserves the current
 * traject scope, mirroring editorRouteFor. Used when leaving the editor
 * for the library (e.g. opening a search result) so the active traject
 * follows the user instead of being dropped.
 */
function libraryRouteFor(lawIdVal) {
  const trajectRef = route.params.trajectRef;
  return trajectRef
    ? { name: 'library-traject', params: { trajectRef, lawId: lawIdVal } }
    : { name: 'library', params: { lawId: lawIdVal } };
}

// If the user lands on the editor without a lawId, restore the last
// tab they had open before the refresh — keeping the same traject
// scope (or lack of it).
if (!route.params.lawId) {
  const last = loadSavedActiveTab();
  if (last?.lawId) {
    router.replace(editorRouteFor(last.lawId, last.articleNumber || undefined));
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
  router.replace(editorRouteFor(tab.lawId, tab.articleNumber));
}

// Fallback: once a load settles with no tab active while the user still has
// open tabs (e.g. the URL points to an article that no longer resolves),
// open the first tab instead of stranding them on the empty state.
watch(loading, async (isLoading) => {
  if (isLoading) return;
  await nextTick();
  if (activeTab.value || openTabs.value.length === 0) return;
  selectTab(openTabs.value[0]).catch(console.warn);
});

// Browser back/forward (or any external navigation) — pull state from
// URL. Local mutations from selectTab already match the destination,
// so the guards below short-circuit; the work only happens for true
// URL changes.
//
// trajectRef-only changes are intentionally NOT handled here: the
// `watch(activeTrajectRef)` above already does `unloadAllLaws` +
// `loadCorpusLaws` + `switchLaw`, and triggering switchLaw twice in
// the same tick would burn an extra fetch (the first await loses the
// switchVersion race, but still hits the network). This guard handles
// the law / article portion only.
onBeforeRouteUpdate(async (to) => {
  const newLawId = to.params.lawId;
  const newArticle = to.params.articleNumber;
  if (!newLawId) return;
  if (newLawId !== lawId.value) {
    const gen = ++switchGeneration;
    await switchLaw(newLawId, newArticle, to.params.trajectRef || null);
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

// Load lawNames for persisted tabs on startup (parallel, deduplicated).
// Reads go through the currently-active traject so tab labels match
// what the editor pane shows after a save.
const uniqueLawIds = [...new Set(openTabs.value.map(t => t.lawId))];
Promise.all(uniqueLawIds.map(async (id) => {
  try {
    const entry = await fetchLaw(activeTrajectRef.value, id);
    lawNames.value = { ...lawNames.value, [id]: entry.lawName };
  } catch { /* ignore */ }
}));

// --- Engine ---
const {
  ready: engineReady,
  initError: engineInitError,
  initEngine,
  getEngine,
  loadLawYaml,
  unloadAllLaws,
} = useEngine();
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

// Load current law into engine. Reacts to currentLawYaml so in-memory
// edits are immediately visible to scenarios. Goes through
// useEngine.loadLawYaml so the engine's scope-tracking map sees this
// load under the right traject — otherwise a later loadDependency call
// for the same law id would treat it as already-current and skip the
// refetch on a traject switch.
// `currentLawYaml` re-dumps on every keystroke, so reacting directly would
// reload the WASM engine per keystroke. Debounce the reload ~300ms after the
// last edit; keep the first load (no previous yaml) synchronous so the initial
// load isn't delayed. Subsequent transitions from an existing yaml — keystroke
// edits, article switches, traject switches — debounce.
let engineLoadDebounce = null;

async function reloadEngineLaw(lawYaml, isReady) {
  if (!isReady || !lawYaml) return;
  try {
    await loadLawYaml(lawYaml, lawId.value, activeTrajectRef.value);
  } catch (e) {
    console.warn(`Failed to load law '${lawId.value}' into engine:`, e);
  }
}

watch(
  [currentLawYaml, engineReady],
  ([lawYaml, isReady], prev) => {
    clearTimeout(engineLoadDebounce);
    // First load (no previous yaml): run immediately so scenarios see the law
    // without delay. Any change from an existing yaml debounces.
    if (!prev || !prev[0]) {
      reloadEngineLaw(lawYaml, isReady);
      return;
    }
    engineLoadDebounce = setTimeout(() => reloadEngineLaw(lawYaml, isReady), 300);
  },
  { immediate: true },
);

onBeforeUnmount(() => clearTimeout(engineLoadDebounce));

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
  // JSON round-trip clone: Vue reactive proxies aren't structuredClone-able
  // in all envs. Safe because law YAML is JSON-plain — but Date/undefined
  // are NOT preserved, which only matters if a future field becomes
  // date-typed (use an explicit serialiser then).
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
    // Drop the container once every field is cleared, otherwise the YAML
    // dump emits a bare `produces: {}` which the schema rejects.
    if (Object.keys(mr.execution.produces).length === 0) {
      delete mr.execution.produces;
    }
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
  // JSON clone — see handleSave: Date/undefined not preserved (fine for
  // JSON-plain law data).
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
  // JSON clone — see handleSave: reactive proxy not structuredClone-able;
  // Date/undefined not preserved (fine for JSON-plain law YAML).
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
                <nldd-tab-bar-item :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)" text="Bibliotheek"></nldd-tab-bar-item>
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
              <TrajectMenu id-suffix="md" />
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-md" size="md" icon="account" text="Account" expandable tooltip-timing="never" popovertarget="settings-menu-md"></nldd-icon-button>
              <nldd-menu id="settings-menu-md" anchor="settings-menu-btn-md">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <template v-else-if="!authLoading && !authenticated">
                  <nldd-menu-item text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                  <nldd-menu-divider></nldd-menu-divider>
                </template>
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
                <nldd-menu-divider v-if="!authLoading && authenticated"></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
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
                <nldd-tab-bar-item :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="center" min-width="240px" width="33%" max-width="480px">
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
              <TrajectMenu id-suffix="lg" />
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-lg" size="md" icon="account" text="Account" expandable tooltip-timing="never" popovertarget="settings-menu-lg"></nldd-icon-button>
              <nldd-menu id="settings-menu-lg" anchor="settings-menu-btn-lg">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <template v-else-if="!authLoading && !authenticated">
                  <nldd-menu-item text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                  <nldd-menu-divider></nldd-menu-divider>
                </template>
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
                <nldd-menu-divider v-if="!authLoading && authenticated"></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
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
              <nldd-button slot="actions" variant="secondary" text="Ga naar bibliotheek" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- Loading takes precedence over `error` to avoid flashing a stale error during a refetch. -->
        <nldd-page v-else-if="loading">
          <nldd-simple-section width="full">
            <nldd-activity-indicator text="Wet laden" show-text></nldd-activity-indicator>
          </nldd-simple-section>
        </nldd-page>

        <!-- Error state — mirrors the library's law-load failure pattern.
             404s typically mean "the law isn't part of the active traject"
             (e.g. after a traject switch); we surface a traject-specific
             message and a quick "Naar bibliotheek" exit. Other failures
             keep the generic copy + retry. -->
        <nldd-page v-else-if="error">
          <nldd-simple-section width="full">
            <nldd-inline-dialog
              v-if="lawErrorIs404"
              variant="alert"
              :text="`${failedLawName} is niet beschikbaar in dit traject`"
              supporting-text="Wissel van traject via het menu rechtsboven of ga terug naar het overzicht."
            >
              <nldd-button slot="actions" variant="primary" text="Naar bibliotheek" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
              <nldd-button slot="actions" variant="secondary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
            </nldd-inline-dialog>
            <nldd-inline-dialog
              v-else
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
                <!-- Notes toggle: only on the Tekst pane, only when the
                     panel.notes capability is enabled, and only in read mode
                     (the overlay needs raw text — it can't align with the
                     editable textarea). Notes are a layer over the text, not
                     a separate pane. -->
                <!-- No start-icon: the design-system has no note/comment
                     glyph (its `comment` icon is an empty SVG). A misleading
                     icon (edit/document) is worse than none; the "Notities"
                     label is clear on its own. Tracked upstream:
                     MinBZK/storybook icon-set request. -->
                <nldd-button
                  v-if="view === 'text' && isEnabled('panel.notes') && !canEditArticleText"
                  size="md"
                  :variant="showNotes ? 'primary' : 'default'"
                  text="Notities"
                  data-testid="notes-toggle"
                  @click="showNotes = !showNotes"
                ></nldd-button>
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
                <!-- Notes overlay (read mode only): same Tekst pane, plain
                     text with resolved highlights instead of the markdown
                     render. Toggled via the header button below. -->
                <template v-else-if="notesActive">
                  <nldd-inline-dialog
                    v-if="notesError"
                    variant="alert"
                    text="Notities niet geladen"
                    :supporting-text="notesError.message"
                  ></nldd-inline-dialog>
                  <nldd-activity-indicator
                    v-else-if="notesLoading"
                    text="Notities laden"
                    show-text
                  ></nldd-activity-indicator>
                  <template v-else>
                    <AnnotatedText
                      :article="selectedArticle"
                      :notes-for-article="notesForArticle"
                      :can-create="canCreateNotes"
                      :law-id="lawId"
                      :engine="noteEngine"
                      :traject-ref="$route.params.trajectRef || ''"
                      @create-note="onCreateNote"
                    />
                    <nldd-inline-dialog
                      v-if="noteIssues.length"
                      variant="warning"
                      :text="`${noteIssues.length} notitie(s) niet verankerd`"
                      :supporting-text="noteIssues.map(i => i.reason).join('; ')"
                    ></nldd-inline-dialog>
                    <!-- Draft notes live in localStorage until written back.
                         "Opslaan naar repo" appends them to the sidecar on
                         the active traject's branch (same write model as
                         law/scenario edits since #632). No source picker —
                         the traject's own corpus config decides the target.
                         Without an active traject the save button is
                         disabled, mirroring the law-edit buttons, so the
                         backend 403 is never a surprise. "Exporteer YAML"
                         stays for the offline / manual-commit case. -->
                    <nldd-inline-dialog
                      v-if="canCreateNotes && draftCount > 0"
                      data-testid="draft-notes-bar"
                      :text="`${draftCount} concept-notitie(s), nog niet opgeslagen`"
                    >
                      <nldd-button
                        slot="actions"
                        size="md"
                        variant="primary"
                        :text="savingNotes ? 'Opslaan…' : 'Opslaan naar repo'"
                        :disabled="savingNotes || !canEdit || null"
                        data-testid="save-notes-btn"
                        @click="saveNotesToRepo"
                      ></nldd-button>
                      <nldd-button
                        slot="actions"
                        size="md"
                        text="Exporteer YAML"
                        data-testid="export-notes-btn"
                        @click="exportNotes"
                      ></nldd-button>
                      <nldd-button
                        slot="actions"
                        size="md"
                        variant="destructive"
                        text="Concepten wissen"
                        data-testid="clear-drafts-btn"
                        @click="askClearDrafts"
                      ></nldd-button>
                    </nldd-inline-dialog>
                    <nldd-inline-dialog
                      v-if="canCreateNotes && draftCount > 0 && !canEdit"
                      variant="warning"
                      data-testid="notes-no-traject"
                      text="Selecteer eerst een traject"
                      supporting-text="Opslaan naar repo werkt pas als er een actief traject is. Exporteer YAML werkt wel."
                    ></nldd-inline-dialog>
                    <nldd-inline-dialog
                      v-if="notesSaveError"
                      variant="alert"
                      data-testid="notes-save-error"
                      text="Notities opslaan mislukt"
                      :supporting-text="notesSaveError"
                    ></nldd-inline-dialog>
                    <nldd-inline-dialog
                      v-if="notesSaveStatus && !notesSaveError"
                      variant="success"
                      data-testid="notes-save-status"
                      text="Opslaan gelukt"
                      :supporting-text="notesSaveStatus"
                    ></nldd-inline-dialog>
                  </template>
                </template>
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
                  :traject-ref="activeTrajectRef"
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
                  :traject-ref="activeTrajectRef"
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
        <nldd-container padding="8" padding-bottom="0">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start" width="100%">
              <TrajectMenu id-suffix="sm" full-width />
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
        <nldd-container padding="8">
          <nldd-toolbar size="lg">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar>
                <nldd-tab-bar-item :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)" icon="books" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item selected icon="edit" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button icon="search" text="Zoeken" @click="openSearch"></nldd-icon-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button id="settings-menu-btn-sm" icon="account" text="Account" popovertarget="settings-menu-sm"></nldd-icon-button>
              <nldd-menu id="settings-menu-sm" anchor="settings-menu-btn-sm">
                <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
                <template v-else-if="!authLoading && !authenticated">
                  <nldd-menu-item text="Inloggen" icon="login" @click="login()"></nldd-menu-item>
                  <nldd-menu-divider></nldd-menu-divider>
                </template>
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
                <nldd-menu-divider v-if="!authLoading && authenticated"></nldd-menu-divider>
                <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" @click="logout"></nldd-menu-item>
              </nldd-menu>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-app-view>

  <ActionSheet :action="activeAction" :article="editedArticle" :editable="canEdit" :is-new="activeActionIsNew" @close="handleActionClose" @save="handleActionSave" />
  <EditSheet :item="activeEditItem" :article="editedArticle" :traject-ref="activeTrajectRef" @save="handleSave" @close="activeEditItem = null" />
  <!-- Traject-documents browser sheet + edit window, opened from TrajectMenu. -->
  <TrajectDocuments />
  <SearchPopover
    ref="searchPopoverRef"
    @select-law="onSearchSelectLaw"
    @harvest-available="onSearchHarvestAvailable"
  />

  <!-- Drafts are local-only until exported (RFC-018: git is the source of
       truth). Wiping them is irreversible and the only copy, so confirm. -->
  <nldd-modal-dialog
    ref="clearDraftsModalEl"
    variant="alert"
    text="Alle concept-notities wissen?"
    supporting-text="Niet-geëxporteerde concepten gaan definitief verloren. Exporteer eerst als je ze wilt bewaren."
    data-testid="clear-drafts-confirm"
    @close="cancelClearDrafts"
  >
    <nldd-button slot="actions" variant="primary" text="Behoud concepten" @click="cancelClearDrafts"></nldd-button>
    <nldd-button slot="actions" variant="destructive" text="Wis alles" data-testid="clear-drafts-confirm-btn" @click="confirmClearDrafts"></nldd-button>
  </nldd-modal-dialog>

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
        :traject-ref="activeTrajectRef"
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
  font-family: var(--primitives-font-family-monospace);
  font-size: 12px;
  padding: 8px 12px;
  border: 1px solid #fecaca;
  border-radius: 6px;
}
</style>

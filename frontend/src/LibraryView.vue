<script setup>
import { ref, computed, shallowRef, nextTick, watch, watchEffect, onBeforeUnmount, inject } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import * as yaml from 'js-yaml';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';
import SearchPopover from './components/SearchPopover.vue';
import { useAuth } from './composables/useAuth.js';
import { lawFetchInit } from './composables/useLaw.js';
import { useTrajects } from './composables/useTrajects.js';
import { lawsListUrl, lawUrl, changedLawsUrl } from './composables/corpusUrls.js';
import { SUPPORT_EMAIL } from './constants.js';
import { registerSearchPopover, setLibraryEmpty } from './composables/useAppChrome.js';
import { homeTarget } from './composables/useLastVisitedRoute.js';
import { humanizeLawId } from './lib/lawName.js';
import { apiFetch, apiFetchJson, ApiError } from './lib/apiFetch.js';
import { useLatest } from './lib/useLatest.js';
import { holdRetryFloor, RETRY_MIN_SPINNER_MS } from './lib/retryFeedback.js';

const { authenticated, login } = useAuth();

// Provided by AppShell: shows the login-warning popover anchored to an element,
// so "Bewerken" gates on login the same way the Editor tab does.
const showLoginWarning = inject('showLoginWarning', null);
// The detail-pane "Bewerken" button, used as the popover anchor for edit
// actions that don't originate from a click on the button itself.
const editButton = ref(null);

// Label of the back-button that returns to the Home sidebar from underlying
// pages. Kept fixed as "Home" even though the sidebar's own heading is now
// traject-aware (see sidebarTitle) — a back-button reads more naturally as
// "Home" than as the traject name.
const LIBRARY_HOME_BACK_TEXT = 'Home';

const route = useRoute();
const router = useRouter();

// Active traject (null = global browse). Derived from the URL via
// `route.params.trajectRef`, so the new `library-traject` route makes the
// bibliotheek traject-aware without any extra plumbing.
const { activeTrajectRef, activeTraject } = useTrajects();

// Primary-sidebar heading: the active traject's name, or 'Corpus juris' for the
// public/global corpus (logged-out, or logged-in without a chosen traject).
// Replaces the old fixed 'RegelRecht' now that Home is traject-aware.
const sidebarTitle = computed(() =>
  activeTrajectRef.value ? activeTraject.value?.name || 'Traject…' : 'Corpus juris',
);

// "Account aanvragen" affordance for the favoriet login popover (mirrors the
// editor/bewerken login popover in AppShell): to the public account-request
// page. /account-aanvragen is a top-level route, so navigating there unmounts
// the popover along with the shell.
const accountRequestHref = computed(() => router.resolve({ name: 'account-aanvragen' }).href);
function goToAccountRequest() {
  router.push({ name: 'account-aanvragen' });
}

// Keep the user's traject scope across in-app navigations. A traject with a law
// stays on `library-traject`, a traject without one on `traject-home`; publicly,
// a law drills into `corpus-juris`, otherwise the bare `home`. See homeTarget.
function libraryRouteFor(params = {}) {
  return homeTarget({
    trajectRef: activeTrajectRef.value || undefined,
    lawId: params.lawId,
    articleNumber: params.articleNumber,
  });
}
function editorRouteFor(lawIdVal, articleNumber) {
  // Without an active traject the editor isn't reachable directly — the
  // editor requires a traject. Send the user to the chooser, carrying the
  // law as query so it opens right after a traject is picked.
  return activeTrajectRef.value
    ? { name: 'editor-traject', params: { trajectRef: activeTrajectRef.value, lawId: lawIdVal, articleNumber } }
    : { name: 'editor', query: { law: lawIdVal || undefined, article: articleNumber || undefined } };
}

const laws = ref([]);
const favorites = ref(null);
// Law ids edited in the active traject (branch-vs-base diff). `null` until
// loaded / when no traject is active; a Set once the endpoint resolves.
const changedLawIds = ref(null);
const loading = ref(true);
const indexError = ref(null);
const searchPopoverRef = ref(null);
// The toolbar search control lives in the AppShell; register our popover so
// the shell's search button/field opens it.
registerSearchPopover(searchPopoverRef);

const selectedLawId = ref(null);
// Which sidebar section the law was opened from, so only that instance is
// highlighted (a law can sit in both Favorieten and Recent bekeken). Cleared on
// non-sidebar selects (search / deep-link); highlightSection then falls back to
// the law's first occurrence.
const selectedSection = ref(null);
const selectedLaw = shallowRef(null);
const selectedLawLoading = ref(false);
const lawError = ref(null);
const selectedArticleNumber = ref(null);

// Recently-viewed laws (most-recent-first), persisted across sessions. Stored
// as { law_id, name } so a law that fails to load in the active traject — and
// therefore never enters the corpus index — still stays reachable + labelled.
const RECENT_LAWS_KEY = 'regelrecht-recent-laws';
const MAX_RECENT_LAWS = 12;
function loadRecentLaws() {
  try {
    const raw = JSON.parse(localStorage.getItem(RECENT_LAWS_KEY) || '[]');
    return Array.isArray(raw) ? raw.filter(r => r && r.law_id) : [];
  } catch {
    return [];
  }
}
const recentLaws = ref(loadRecentLaws());
function recordRecentLaw(lawId, name) {
  if (!lawId) return;
  const entry = { law_id: lawId, name: name || humanizeLawId(lawId) };
  recentLaws.value = [entry, ...recentLaws.value.filter(r => r.law_id !== lawId)].slice(0, MAX_RECENT_LAWS);
  try {
    localStorage.setItem(RECENT_LAWS_KEY, JSON.stringify(recentLaws.value));
  } catch { /* storage unavailable — keep the in-memory list */ }
}
// Detail view (tekst/machine/yaml) is reflected in the URL hash so the
// state is bookmarkable and shareable. English keys in the hash because
// they're stable identifiers, not labels.
const VIEW_TO_HASH = { tekst: '#text', machine: '#machine', yaml: '#yaml' };
const HASH_TO_VIEW = { '#text': 'tekst', '#machine': 'machine', '#yaml': 'yaml' };

const detailView = computed({
  get() {
    return HASH_TO_VIEW[route.hash] ?? 'tekst';
  },
  set(value) {
    // Reject anything we don't recognise rather than silently stripping
    // the hash — every call site today hard-codes a literal, so an
    // unknown value is a programmer error. Warn in dev so a future
    // contributor adding a tab without updating VIEW_TO_HASH catches
    // it immediately; production silently no-ops to avoid console noise.
    const hash = VIEW_TO_HASH[value];
    if (!hash) {
      if (import.meta.env.DEV) {
        console.warn(`[detailView] ignoring unknown value: ${value}`);
      }
      return;
    }
    if (hash !== route.hash) {
      router.replace({ path: route.path, query: route.query, hash });
    }
  },
});
// nldd-tab-bar fires `tabchange` on BOTH pointer click and arrow-key
// activation (content-switching tabs auto-activate on arrow — the ARIA
// pattern). Driving detailView from this single event keeps the keyboard, the
// selected tab, and the visible panel in lockstep; a per-item @click never
// fired on arrow, which is why the highlight moved but the view lagged.
// `:selected` stays the controlled-in binding so a hash-driven detailView
// change (e.g. a deep link to #yaml) still reflects on the tabs.
function onDetailTabChange(e) {
  const view = e.detail?.item?.dataset?.view;
  if (view) detailView.value = view;
}

const activeAction = ref(null);

// Curated sidebar sections (in render order). Each entry is
// `{ key, title, laws }`. Empty sections are never pushed, so the template
// can iterate without per-section emptiness checks.
//
//   - "Bewerkt in dit traject" comes first: it's the small, high-signal,
//     context-specific set, so it sits above favorites.
//     Only present when a traject is active and the diff is non-empty.
//   - "Favorieten": the user's personal favorites.
//
// There is deliberately NO full-corpus fallback: the central corpus is the
// full BWB corpus (thousands of laws), so dumping it into the sidebar isn't
// useful and is exactly the "huge pile" we don't want loaded here. When
// nothing is curated yet, the template shows a search CTA instead — full
// browse lives in the search popover.
const sidebarSections = computed(() => {
  const list = laws.value;
  const sections = [];

  if (activeTrajectRef.value && changedLawIds.value?.size) {
    const changed = list.filter(law => changedLawIds.value.has(law.law_id));
    if (changed.length > 0) {
      sections.push({ key: 'changed', title: 'Bewerkt in dit traject', laws: changed });
    }
  }

  if (favorites.value) {
    const favList = list.filter(law => favorites.value.has(law.law_id));
    if (favList.length > 0) {
      sections.push({ key: 'favorites', title: 'Favorieten', laws: favList });
    }
  }

  // "Recent bekeken" sits below the curated groups and faithfully shows the
  // view history: a law stays here even when it's also a favorite or edited in
  // this traject, so it can appear in both sections (favoriting a law must not
  // remove it from recent). Each id resolves to its richer index entry when
  // available, otherwise to the stored { law_id, name } (e.g. a law not present
  // in the active traject).
  if (recentLaws.value.length > 0) {
    const recent = recentLaws.value
      .map(r => list.find(law => law.law_id === r.law_id) || r);
    sections.push({ key: 'recent', title: 'Recent bekeken', laws: recent });
  }

  return sections;
});

// "No usable content" states, shown full-page (like EditorView's no-content
// states) instead of inside the split-view, so the error/CTA spans the full
// width rather than the narrow sidebar. isInitialLoading covers the first load
// before anything resolves; indexError is handled at the same top level.
const isInitialLoading = computed(
  () => loading.value && !selectedLawId.value && sidebarSections.value.length === 0,
);
const isEmptyLibrary = computed(
  () => !loading.value && !indexError.value && !selectedLawId.value && sidebarSections.value.length === 0,
);

// Tell the shell whether the library is empty so it can show the just-in-time
// search coach-mark on the toolbar field; reset on unmount so it doesn't linger
// on the editor route.
watchEffect(() => setLibraryEmpty(isEmptyLibrary.value));
onBeforeUnmount(() => setLibraryEmpty(false));

const articles = computed(() => selectedLaw.value?.articles ?? []);

const lawName = computed(() => {
  if (!selectedLaw.value) return '';
  const nameRef = selectedLaw.value.name;
  if (typeof nameRef === 'string' && nameRef.startsWith('#')) {
    const outputName = nameRef.slice(1);
    for (const article of articles.value) {
      const actions = article.machine_readable?.execution?.actions;
      if (!actions) continue;
      for (const action of actions) {
        if (action.output === outputName) return action.value;
      }
    }
  }
  if (nameRef) return nameRef;
  return humanizeLawId(selectedLaw.value.$id || selectedLaw.value.law_id || '');
});

// Display name resolved from the index. Used in the load-error state where
// `selectedLaw` is null and `lawName` would be empty.
const indexedLawName = computed(() => {
  if (!selectedLawId.value) return '';
  const law = laws.value.find(l => l.law_id === selectedLawId.value);
  return law ? displayName(law) : humanizeLawId(selectedLawId.value);
});

// Track the active law in "Recent bekeken" — including one that fails to load,
// so the sidebar reflects what the user is looking at even when nothing is
// curated yet. Re-runs as the name resolves to upgrade the label from the
// humanized id to the real name.
watch([selectedLawId, lawName, indexedLawName], () => {
  if (selectedLawId.value) {
    recordRecentLaw(selectedLawId.value, lawName.value || indexedLawName.value);
  }
}, { immediate: true });

const selectedArticle = computed(() => {
  if (!selectedArticleNumber.value) return null;
  return articles.value.find(
    (a) => String(a.number) === String(selectedArticleNumber.value)
  ) ?? null;
});

// True when the URL points at an article that doesn't exist in the
// loaded law. Distinct from "no article selected" (where no article
// number is in the URL).
const articleNotFound = computed(() =>
  !!(selectedLaw.value && selectedArticleNumber.value && !selectedArticle.value)
);

// 404 means the law isn't in the active traject's corpus; the error UI shows a traject-specific message.
const lawErrorIs404 = computed(() => lawError.value?.status === 404);

// Reflect navigation depth in the document title:
//   "Art. 5 · Wet op de zorgtoeslag · 15 juni test · RegelRecht"
// On a traject-scoped browse the active traject name is appended (like the
// editor) so the browser tab and history show which traject you are viewing.
// Most-specific first so browser tab truncation preserves the article number.
// We deliberately omit the "Bibliotheek:" prefix here (unlike the editor) —
// browsing laws is the implicit default, and the law name carries enough
// context. The editor still prefixes because "Wijzig:" disambiguates the
// edit context from the read-only browse.
// Always set (no early return) — router.afterEach used to set a static
// fallback but it raced with this effect on tab/article switches.
watchEffect(() => {
  const detail = [];
  if (selectedArticle.value) detail.push(`Art. ${selectedArticle.value.number}`);
  // Fall back to indexedLawName so the title reflects the URL even when the
  // law itself failed to load.
  const name = lawName.value || indexedLawName.value;
  if (name) detail.push(name);
  // Traject-scoped browse: append the traject name (resolves once the trajects
  // list loads). Null on the public no-traject library, so it drops out there.
  if (activeTraject.value?.name) detail.push(activeTraject.value.name);
  document.title = detail.length > 0
    ? `${detail.join(' · ')} · RegelRecht`
    : 'Home · RegelRecht';
});

function displayName(law) {
  // Prefer the API's resolved `display_name`: laws can have a dynamic
  // `name: "#output_ref"` in YAML that the backend resolves via the
  // matching action output. Without this check we'd render the raw
  // `#output_ref` string for those laws.
  if (law.display_name) return law.display_name;
  if (law.name) return law.name;
  return humanizeLawId(law.law_id);
}

function articleDescription(article) {
  if (!article.text) return '';
  const firstLine = article.text.split('\n')[0].replace(/\*\*/g, '');
  return firstLine.length > 80 ? firstLine.slice(0, 80) + '...' : firstLine;
}

async function loadFavorites() {
  try {
    const favIds = await apiFetchJson('/api/favorites', {
      errorMessage: (status) => `Failed to load favorites: ${status}`,
    });
    favorites.value = new Set(favIds);
  } catch (e) {
    // Not authenticated (401/403) or endpoint unavailable — no favorites.
    // Only server errors are worth a console trace.
    if (e instanceof ApiError && e.status >= 500) console.warn(e.message);
  }
}

// Fetch the set of law ids edited in the active traject. Returns `null`
// when there's no traject (global browse has no "changed" notion) or on
// any failure — the "Bewerkt in dit traject" section then simply stays
// hidden instead of surfacing an error in the sidebar. The backend returns
// an empty array (not an error) when nothing has been saved yet, which maps
// to an empty Set and a hidden section all the same.
async function fetchChangedLawIds(trajectRef) {
  if (!trajectRef) return null;
  try {
    return new Set(await apiFetchJson(changedLawsUrl(trajectRef)));
  } catch {
    // Any failure (HTTP or network) just hides the section — see above.
    return null;
  }
}

const togglingFavorites = ref(new Set());

const favoriteLoginWarning = ref(null);
// Heart button when not logged in: nudge to log in via a popover anchored to the
// button (same pattern as the Editor tab + Trajecten button) instead of silently
// doing nothing.
function onFavoriteClick(e) {
  if (!authenticated.value) {
    if (favoriteLoginWarning.value) {
      favoriteLoginWarning.value.anchorElement = e.currentTarget;
      favoriteLoginWarning.value.show();
    }
    return;
  }
  toggleFavorite(selectedLawId.value);
}

async function toggleFavorite(lawId) {
  if (!authenticated.value || !lawId) return;
  if (togglingFavorites.value.has(lawId)) return;

  togglingFavorites.value.add(lawId);
  const isFav = favorites.value?.has(lawId);

  // Optimistic update
  const updated = new Set(favorites.value || []);
  if (isFav) updated.delete(lawId);
  else updated.add(lawId);
  favorites.value = updated;

  const revert = () => {
    const reverted = new Set(favorites.value);
    if (isFav) reverted.add(lawId);
    else reverted.delete(lawId);
    favorites.value = reverted;
  };

  try {
    const method = isFav ? 'DELETE' : 'PUT';
    await apiFetch(`/api/favorites/${encodeURIComponent(lawId)}`, { method });
    // Re-resolve the sidebar's id-set so a newly-favorited law (whose
    // metadata isn't loaded yet, since we only fetch favorites + edits by
    // id) appears in the Favorieten section without a manual reload.
    loadIndex();
  } catch {
    // HTTP or network failure — roll the optimistic toggle back.
    revert();
  } finally {
    togglingFavorites.value.delete(lawId);
  }
}

const claimLoadIndex = useLatest();

async function loadIndex() {
  const isCurrent = claimLoadIndex();
  // Snapshot the traject so the changed-laws fetch and its assignment below
  // both refer to the scope this run started in.
  const trajectRef = activeTrajectRef.value;
  try {
    // Resolve the small id sets the sidebar actually needs: the user's
    // personal favorites and (in a traject) the laws edited on the traject
    // branch. Both `loadFavorites` and `fetchChangedLawIds` are id-only.
    const [, changedIds] = await Promise.all([
      loadFavorites(),
      fetchChangedLawIds(trajectRef),
    ]);
    if (!isCurrent()) return;
    changedLawIds.value = changedIds;

    // Fetch metadata for just those ids via `?ids=` — never the whole corpus.
    // The central corpus is the full BWB corpus (thousands of laws); loading
    // it here only to filter out a handful would be wasteful and would miss
    // any favorite/edit that sorts past a page cap. Full browse lives in the
    // search popover instead.
    const ids = new Set([...(favorites.value || []), ...(changedIds || [])]);
    if (ids.size === 0) {
      laws.value = [];
      return;
    }
    const query = `ids=${encodeURIComponent([...ids].join(','))}&limit=1000`;
    const res = await apiFetch(lawsListUrl(trajectRef, query), {
      errorMessage: (status) => `Failed to load corpus: ${status}`,
    });
    // Gate before and after json(): skip parsing for stale 200s, and catch races during it.
    if (!isCurrent()) return;
    const corpusLaws = await res.json();
    if (!isCurrent()) return;
    laws.value = corpusLaws.sort((a, b) => a.law_id.localeCompare(b.law_id));
  } catch (e) {
    if (!isCurrent()) return;
    indexError.value = e;
  } finally {
    if (isCurrent()) {
      loading.value = false;
    }
  }
}

const claimLoadLaw = useLatest();

async function loadLaw(lawId, { minLoadingMs = 0 } = {}) {
  const isCurrent = claimLoadLaw();
  const startedAt = Date.now();
  let failed = false;
  try {
    selectedLawLoading.value = true;
    const res = await apiFetch(lawUrl(activeTrajectRef.value, lawId), lawFetchInit);
    // Gate before and after `res.text()`: skip the body read for stale 200s, and catch races during it.
    if (!isCurrent()) return;
    const text = await res.text();
    if (!isCurrent()) return;
    selectedLaw.value = yaml.load(text);
    // selectedArticleNumber is set from the route on initial mount and via
    // onBeforeRouteUpdate; we don't validate here so an invalid number
    // surfaces as the articleNotFound error state instead of being silently
    // stripped from the URL.
  } catch (e) {
    if (!isCurrent()) return;
    failed = true;
    selectedLaw.value = null;
    lawError.value = e;
  } finally {
    if (isCurrent()) {
      // On a failed retry, hold the spinner briefly so the click reads as
      // feedback instead of the error dialog snapping straight back.
      await holdRetryFloor({ startedAt, minMs: minLoadingMs, failed });
      if (isCurrent()) selectedLawLoading.value = false;
    }
  }
}

function retryLoadLaw() {
  if (!selectedLawId.value) return;
  // No explicit selectedLawLoading.value = true here (unlike
  // retryLoadCorpus → loadIndex): loadLaw sets it as the first
  // statement inside its try block, which runs synchronously before
  // any await yields. The next reactivity flush sees both
  // lawError = null and selectedLawLoading = true together, so the
  // template can't briefly fall through to the "Selecteer een wet"
  // empty state.
  lawError.value = null;
  loadLaw(selectedLawId.value, { minLoadingMs: RETRY_MIN_SPINNER_MS });
}

function retryLoadCorpus() {
  indexError.value = null;
  // loadIndex only flips loading back to false in its finally block —
  // it never sets it to true. So after the first failure (loading is
  // false, indexError is truthy) we have to flip the spinner back on
  // here, otherwise the retry shows the error pane until the next
  // round-trip resolves.
  loading.value = true;
  loadIndex();
}

function editInEditor() {
  if (!selectedLawId.value || !selectedArticleNumber.value) return;
  activeAction.value = null;
  if (!authenticated.value) {
    gateEditorLogin(editButton.value);
    return;
  }
  // Carry the active traject so "Bewerken" opens the editable
  // editor-traject view instead of the read-only editor.
  router.push(editorRouteFor(selectedLawId.value, selectedArticleNumber.value));
}

// "Bewerken" button in the detail pane: same traject-aware target as
// editInEditor, exposed as a location + href so the anchor is real (and
// middle-click / open-in-new-tab works) while the click stays SPA.
const editLawTarget = computed(() =>
  editorRouteFor(selectedLawId.value, selectedArticleNumber.value || undefined),
);
const editLawHref = computed(() => router.resolve(editLawTarget.value).href);

// Editor requires login. Instead of letting the route guard bounce an
// unauthenticated user to SSO, show the same login-warning popover the Editor
// tab uses (anchored to `anchorEl`), returning to this article after login.
function gateEditorLogin(anchorEl) {
  showLoginWarning?.(anchorEl, editLawHref.value);
}

function onEditClick(e) {
  if (!authenticated.value) {
    gateEditorLogin(e.currentTarget);
    return;
  }
  router.push(editLawTarget.value);
}

function selectLaw(lawId, focusAfter = false) {
  // Default to no section context (search / programmatic); selectLawFromSection
  // re-sets it after this call for sidebar clicks.
  selectedSection.value = null;
  if (lawId !== selectedLawId.value || lawError.value) {
    selectedLawId.value = lawId;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
    router.push(libraryRouteFor({ lawId }));
    loadLaw(lawId);
  }

  // When triggered from the search popover we want focus to land on the
  // newly-selected sidebar item — not on the popover trigger that
  // popover._returnFocus restores to. Schedule on nextTick so the popover
  // has fully closed (sync) and Vue has rendered the selected state, then
  // walk the list-item shadow DOM to focus its inner button (the host
  // doesn't delegate focus). Scope by section so a law present in two sections
  // focuses the clicked instance (selectedSection is set by the time this runs).
  if (focusAfter) {
    nextTick(() => {
      const sectionSel = selectedSection.value ? `[data-section="${CSS.escape(selectedSection.value)}"]` : '';
      const item = document.querySelector(`${sectionSel}[data-law-id="${CSS.escape(lawId)}"]`);
      const action = item?.shadowRoot?.querySelector('.list-item__action');
      action?.focus?.();
    });
  }
}

// Sidebar click: record which section it came from, then select (keeping the
// focus-restore that recent-reordering needs).
function selectLawFromSection(lawId, sectionKey) {
  selectLaw(lawId, sectionKey === 'recent');
  selectedSection.value = sectionKey;
}

// The single sidebar instance to highlight: the clicked section (when it still
// holds the law), else the law's first occurrence — so exactly one instance is
// selected, including on a deep-link where no section was clicked.
const highlightSection = computed(() => {
  const id = selectedLawId.value;
  if (!id) return null;
  const sections = sidebarSections.value;
  const clicked = sections.find(s => s.key === selectedSection.value);
  if (clicked?.laws.some(l => l.law_id === id)) return clicked.key;
  return sections.find(s => s.laws.some(l => l.law_id === id))?.key ?? null;
});

function selectArticle(number) {
  const articleStr = String(number);
  if (articleStr === selectedArticleNumber.value) return;
  selectedArticleNumber.value = articleStr;
  activeAction.value = null;
  router.replace({
    ...libraryRouteFor({ lawId: selectedLawId.value, articleNumber: articleStr }),
    hash: route.hash,
  });
}

/**
 * Pane back-button handlers — URL-driven so browser back works the same
 * way as clicking the in-pane back button. Pushing the URL one level up
 * lets `onBeforeRouteUpdate` reactively pull the right local state into
 * sync. On sm the navigation-split-view shows the deepest pane with
 * has-content based on those state values.
 *
 * Listening at the nldd-navigation-split-view level (rather than per pane)
 * is more reliable: bubbling always reaches there. We use composedPath
 * to identify which pane the back originated from and route accordingly.
 *
 * `back` is the event fired by nldd-top-title-bar's back-button (not
 * `dismiss` — that's the X-style close button on the right).
 */
function onPaneBack(e) {
  const path = e.composedPath();
  const pane = path.find(el => el.tagName === 'NLDD-SPLIT-VIEW-PANE');
  if (!pane) return;
  const slot = pane.getAttribute('slot');
  // On any error state — corpus load failed (indexError) or this
  // specific law failed (lawError) — back from the main pane should
  // return to the library root, not /library/<lawId>. The latter would
  // route the user back into the same error they just dismissed.
  if (slot === 'main') return (lawError.value || indexError.value) ? goToLibraryRoot() : goToLawRoot();
  if (slot === 'secondary-sidebar') return goToLibraryRoot();
}

function goToLawRoot() {
  if (selectedLawId.value) {
    router.push(libraryRouteFor({ lawId: selectedLawId.value }));
  }
}

function goToLibraryRoot() {
  router.push(libraryRouteFor());
}

function clearRecent() {
  // Deselect only if the open law was reachable *solely* via "Recent bekeken"
  // (not also a favorite / traject edit), so clearing the list doesn't leave a
  // selected-but-invisible law. Then return to the library home.
  const sel = selectedLawId.value;
  const stillShown =
    !!sel && ((favorites.value && favorites.value.has(sel)) ||
              (changedLawIds.value && changedLawIds.value.has(sel)));
  const deselect = !!sel && recentLaws.value.some(r => r.law_id === sel) && !stillShown;
  recentLaws.value = [];
  try { localStorage.removeItem(RECENT_LAWS_KEY); } catch { /* ignore */ }
  if (deselect) goToLibraryRoot();
}

// Handle browser back/forward navigation
onBeforeRouteUpdate((to) => {
  const newLawId = to.params.lawId;
  const newArticle = to.params.articleNumber;

  if (!newLawId) {
    // Navigated to /library with no lawId — clear state. No auto-select:
    // the empty state (Wetten Browser only) is a valid landing view.
    selectedLawId.value = null;
    selectedLaw.value = null;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
  } else if (newLawId !== selectedLawId.value) {
    selectedLawId.value = newLawId;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
    loadLaw(newLawId);
  } else if (newLawId === selectedLawId.value) {
    if (newArticle) {
      const articleStr = String(newArticle);
      if (articleStr !== selectedArticleNumber.value) {
        selectedArticleNumber.value = articleStr;
        activeAction.value = null;
      }
    } else {
      selectedArticleNumber.value = null;
      activeAction.value = null;
    }
  }
});

// When a harvested law becomes available, reload the corpus and select it.
async function onHarvestAvailable(slug) {
  // Best-effort reload — a failure just means the index below may not
  // include the fresh law yet.
  await apiFetch('/api/corpus/reload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ law_ids: [slug] }),
  }).catch(() => {});
  await loadIndex();
  selectLaw(slug);
}

// Initial load from route
if (route.params.lawId) {
  selectedLawId.value = route.params.lawId;
  if (route.params.articleNumber) {
    selectedArticleNumber.value = String(route.params.articleNumber);
  }
  loadLaw(route.params.lawId);
}
loadIndex();

// The bibliotheek reads through the active traject's corpus
// (`/api/trajects/{ref}/corpus/...`) or the global corpus (`/api/corpus/...`)
// depending on `activeTrajectRef`. When the user switches traject in-place
// (e.g. picking another traject from the TrajectMenu while staying in the
// library, or "Geen traject"), the route param changes but the component
// stays mounted — so refetch the index and the open law through the new
// scope. Mirrors EditorApp's `watch(activeTrajectRef)`.
watch(activeTrajectRef, () => {
  // Drop the previous traject's changed-set immediately so the
  // "Bewerkt in dit traject" section doesn't briefly show stale entries
  // (filtered against the also-stale corpus) while the new index loads.
  // `loadIndex` repopulates it for the new scope, or leaves it null in
  // global browse.
  changedLawIds.value = null;
  loadIndex();
  if (selectedLawId.value) {
    lawError.value = null;
    loadLaw(selectedLawId.value);
  }
});
</script>

<template>
        <!-- Full-page "no usable content" states (matching EditorView): shown
             instead of the split-view so the error / CTA spans the full width,
             not the narrow sidebar. -->
        <nldd-page v-if="indexError">
          <nldd-simple-section width="full">
            <nldd-inline-dialog
              variant="alert"
              text="Wetten en regels zijn niet geladen"
              supporting-text="De gegevens konden niet worden opgehaald."
            >
              <nldd-button slot="actions" variant="primary" text="Probeer opnieuw" @click="retryLoadCorpus"></nldd-button>
              <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <nldd-page v-else-if="isInitialLoading">
          <nldd-simple-section width="full">
            <nldd-activity-indicator timing="instant" text="Laden" show-text></nldd-activity-indicator>
          </nldd-simple-section>
        </nldd-page>

        <!-- Nothing curated yet (no favorites, no traject edits, no open law):
             leave the canvas blank — the just-in-time coach-mark on the toolbar
             search field (AppShell) points the user at search. -->
        <nldd-page v-else-if="isEmptyLibrary"></nldd-page>

        <nldd-navigation-split-view v-else @back="onPaneBack">

          <nldd-split-view-pane slot="sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar v-if="!loading" slot="header" :text="sidebarTitle" collapse-anchor="home-titel"></nldd-top-title-bar>

              <nldd-simple-section width="full">
                <nldd-title v-if="!loading" id="home-titel" size="3"><h3>{{ sidebarTitle }}</h3></nldd-title>
                <nldd-spacer v-if="!loading" size="16"></nldd-spacer>
                <nldd-activity-indicator v-if="loading" timing="instant" text="Laden" show-text></nldd-activity-indicator>
                <template v-else>
                  <!-- In a traject, the curated corpus groups sit under a
                       'Corpus {traject}' heading — the peer of the (Fase 4)
                       Werkdocumenten group. Public Home shows no such heading. -->
                  <template v-if="activeTraject?.name && sidebarSections.length > 0">
                    <nldd-title size="4"><h4>Corpus {{ activeTraject.name }}</h4></nldd-title>
                    <nldd-spacer size="12"></nldd-spacer>
                  </template>
                  <template
                    v-for="(section, sectionIndex) in sidebarSections"
                    :key="section.key"
                  >
                    <!-- Gap above every section after the first, so the
                         curated groups read as distinct blocks. -->
                    <nldd-spacer v-if="sectionIndex > 0" size="24"></nldd-spacer>
                    <template v-if="section.title">
                      <nldd-title size="5">
                        <h4>{{ section.title }}</h4>
                        <nldd-button
                          v-if="section.key === 'recent'"
                          slot="actions"
                          size="xs"
                          variant="accent-transparent"
                          text="Wis"
                          @click="clearRecent"
                        ></nldd-button>
                      </nldd-title>
                      <nldd-spacer size="8"></nldd-spacer>
                    </template>
                    <nldd-list variant="simple" arrow-navigation>
                      <nldd-list-item
                        v-for="law in section.laws"
                        :key="`${section.key}-${law.law_id}`"
                        size="md"
                        button
                        :data-law-id="law.law_id"
                        :data-section="section.key"
                        :selected="(law.law_id === selectedLawId && section.key === highlightSection) || undefined"
                        @click="selectLawFromSection(law.law_id, section.key)"
                      >
                        <nldd-text-cell :text="displayName(law)" :supporting-text="law.source_name">
                        </nldd-text-cell>
                        <nldd-spacer-cell size="8"></nldd-spacer-cell>
                        <nldd-icon-cell size="20">
                          <nldd-icon name="chevron-right"></nldd-icon>
                        </nldd-icon-cell>
                      </nldd-list-item>
                    </nldd-list>
                  </template>
                </template>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Secondary Sidebar: Artikelen Lijst — only when a law is
               selected. When deselected the pane is removed from the DOM
               so the navigation-split-view reflows to spatial mode and
               shows the sidebar (Wetten Browser) alongside main. -->
          <nldd-split-view-pane v-if="selectedLawId && !lawError" slot="secondary-sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar
                v-if="!selectedLawLoading"
                slot="header"
                :text="lawName || indexedLawName"
                :back-text="LIBRARY_HOME_BACK_TEXT"
                collapse-anchor="wet-titel"
              ></nldd-top-title-bar>

              <nldd-simple-section width="full">
                <nldd-title v-if="!selectedLawLoading" id="wet-titel" size="3"><h3>{{ lawName }}</h3></nldd-title>
                <nldd-spacer v-if="!selectedLawLoading" size="16"></nldd-spacer>
                <nldd-toolbar v-if="selectedLaw" label="Favorieten">
                  <nldd-toolbar-item slot="start">
                    <nldd-icon-button
                      :icon="favorites?.has(selectedLawId) ? 'heart-filled' : 'heart'"
                      :text="favorites?.has(selectedLawId) ? 'Verwijder uit favorieten' : 'Voeg toe aan favorieten'"
                      @click="onFavoriteClick"
                    ></nldd-icon-button>
                  </nldd-toolbar-item>
                </nldd-toolbar>
                <nldd-spacer v-if="selectedLaw" size="16"></nldd-spacer>
                <nldd-popover ref="favoriteLoginWarning" accessible-label="Inloggen" width="320px">
                  <nldd-container padding="16">
                    <nldd-inline-dialog
                      icon="login"
                      text="Log in om wetten als favoriet te markeren"
                      supporting-text="Zodra je bent ingelogd kun je wetten bewaren en snel terugvinden."
                    >
                      <nldd-button slot="actions" variant="primary" text="Inloggen" @click="login()"></nldd-button>
                      <nldd-button slot="actions" variant="secondary" text="Account aanvragen" :href="accountRequestHref" @click.prevent="goToAccountRequest"></nldd-button>
                    </nldd-inline-dialog>
                  </nldd-container>
                </nldd-popover>
                <nldd-activity-indicator v-if="selectedLawLoading" timing="instant" text="Wet laden" show-text></nldd-activity-indicator>
                <nldd-inline-dialog v-else-if="!selectedLaw" text="Selecteer een wet"></nldd-inline-dialog>
                <nldd-list v-else variant="simple" arrow-navigation>
                  <nldd-list-item
                    v-for="article in articles"
                    :key="article.number"
                    size="md"
                    button
                    :selected="String(article.number) === String(selectedArticleNumber) || undefined"
                    @click="selectArticle(article.number)"
                  >
                    <nldd-text-cell :text="`Artikel ${article.number}`" :supporting-text="articleDescription(article)">
                    </nldd-text-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20">
                      <nldd-icon name="chevron-right"></nldd-icon>
                    </nldd-icon-cell>
                  </nldd-list-item>
                </nldd-list>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Main: Artikel Detail -->
          <nldd-split-view-pane slot="main" :has-content="selectedArticle || lawError || articleNotFound ? true : undefined">
            <nldd-page sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="selectedArticle ? `Artikel ${selectedArticle.number}` : undefined"
                :supporting-text="selectedArticle ? lawName : undefined"
                :back-text="lawError ? LIBRARY_HOME_BACK_TEXT : (lawName || 'Terug')"
                :collapse-anchor="selectedArticle ? 'article-titel' : undefined"
              ></nldd-top-title-bar>

              <nldd-simple-section width="full" v-if="!selectedLawId">
                <nldd-inline-dialog text="Selecteer een wet"></nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="selectedLawLoading">
                <!-- Loading takes precedence over `lawError` to avoid flashing a stale error during a refetch. -->
                <nldd-activity-indicator timing="instant" text="Artikel laden" show-text></nldd-activity-indicator>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="lawError">
                <!-- 404 = law not in active traject; give the user an exit instead of a generic error. -->
                <nldd-inline-dialog
                  v-if="lawErrorIs404"
                  variant="alert"
                  :text="`${indexedLawName} is niet beschikbaar in dit traject`"
                  supporting-text="Wissel van traject via het menu rechtsboven of ga terug naar het overzicht."
                >
                  <nldd-button slot="actions" variant="primary" text="Naar overzicht" @click="goToLibraryRoot"></nldd-button>
                  <nldd-button slot="actions" variant="secondary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
                </nldd-inline-dialog>
                <nldd-inline-dialog
                  v-else
                  variant="alert"
                  :text="`${indexedLawName} is niet geladen`"
                  supporting-text="De gegevens konden niet worden opgehaald."
                >
                  <nldd-button slot="actions" variant="primary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
                  <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="articleNotFound">
                <nldd-inline-dialog
                  variant="alert"
                  :text="`Artikel ${selectedArticleNumber} van ${lawName || indexedLawName} bestaat niet`"
                  supporting-text="Mogelijk klopt de URL niet. Neem contact op als je verwacht dat dit artikel wel bestaat."
                >
                  <nldd-button slot="actions" class="article-not-found__back-button" variant="primary" text="Bekijk artikelen" @click="goToLawRoot"></nldd-button>
                  <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="!selectedArticle">
                <nldd-inline-dialog text="Selecteer een artikel"></nldd-inline-dialog>
              </nldd-simple-section>
              <template v-else>
                <nldd-simple-section width="full">
                  <nldd-title id="article-titel" size="3">
                    <h3>Artikel {{ selectedArticle.number }}</h3>
                    <span slot="subtitle">{{ lawName }}</span>
                  </nldd-title>
                  <nldd-spacer size="16"></nldd-spacer>
                  <nldd-toolbar>
                    <nldd-toolbar-item slot="start">
                      <nldd-tab-bar size="md" @tabchange="onDetailTabChange">
                        <nldd-tab-bar-item data-view="tekst" :selected="detailView === 'tekst' || undefined" text="Tekst"></nldd-tab-bar-item>
                        <nldd-tab-bar-item data-view="machine" :selected="detailView === 'machine' || undefined" text="Machine"></nldd-tab-bar-item>
                        <nldd-tab-bar-item data-view="yaml" :selected="detailView === 'yaml' || undefined" text="YAML"></nldd-tab-bar-item>
                      </nldd-tab-bar>
                    </nldd-toolbar-item>
                    <nldd-toolbar-item slot="end">
                      <nldd-button ref="editButton" v-if="selectedLawId" variant="secondary" text="Bewerken" :href="authenticated ? editLawHref : undefined" @click.prevent="onEditClick"></nldd-button>
                    </nldd-toolbar-item>
                  </nldd-toolbar>
                  <nldd-spacer size="24"></nldd-spacer>
                  <KeepAlive>
                    <ArticleText v-if="detailView === 'tekst'" :article="selectedArticle" centered />
                    <MachineReadable v-else-if="detailView === 'machine'" :article="selectedArticle" @open-action="activeAction = $event" />
                    <YamlView v-else-if="detailView === 'yaml'" :article="selectedArticle" />
                  </KeepAlive>
                </nldd-simple-section>
              </template>
            </nldd-page>
          </nldd-split-view-pane>

        </nldd-navigation-split-view>
  <!-- LibraryApp is a read-only browser; ActionSheet is mounted without editable
       so the output field is hidden and the footer button just closes the sheet. -->
  <ActionSheet :action="activeAction" :article="selectedArticle" :editable="false" @close="activeAction = null" @save="activeAction = null" @edit="editInEditor" />
  <SearchPopover
    ref="searchPopoverRef"
    @select-law="(lawId) => selectLaw(lawId, true)"
    @harvest-available="onHarvestAvailable"
  />
</template>

<style>
/* Unscoped on purpose: nldd-navigation-split-view is a custom element
   with its own shadow root, but the `.full-stack` class is reflected on
   the host element from light-DOM space, so a scoped selector here
   wouldn't match any longer than the scoping attribute the Vue compiler
   would inject. The class name is namespaced (`article-not-found__…`)
   to make accidental collisions outside this view unlikely.

   "Bekijk artikelen" alleen tonen wanneer de artikelenlijst niet naast
   de main pane zichtbaar is (full-stack mode = single pane op mobile). */
nldd-navigation-split-view:not(.full-stack) .article-not-found__back-button {
  display: none;
}
</style>

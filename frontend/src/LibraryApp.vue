<script setup>
import { ref, computed, shallowRef, nextTick, watch, watchEffect } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import yaml from 'js-yaml';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';
import SearchPopover from './components/SearchPopover.vue';
import TrajectMenu from './components/TrajectMenu.vue';
import TrajectDocuments from './components/TrajectDocuments.vue';
import { useAuth } from './composables/useAuth.js';
import { lawFetchError } from './composables/useLaw.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useColorScheme } from './composables/useColorScheme.js';
import { useTrajects } from './composables/useTrajects.js';
import { lawsListUrl, lawUrl, changedLawsUrl } from './composables/corpusUrls.js';
import { SUPPORT_EMAIL } from './constants.js';
import { lastEditorPath, sectionTarget } from './composables/useLastVisitedRoute.js';

const { authenticated, loading: authLoading, oidcConfigured, person, login, logout } = useAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();
const { colorScheme, setColorScheme } = useColorScheme();

// Library home title (sidebar header + home heading) and the label of
// the back-button that returns to it from underlying pages. They differ
// intentionally: the page is titled "RegelRecht", but a back-button
// reads more naturally as "Home".
const LIBRARY_HOME_TITLE = 'RegelRecht';
const LIBRARY_HOME_BACK_TEXT = 'Home';

const colorSchemeOptions = [
  ['auto', 'Systeem'],
  ['light', 'Licht'],
  ['dark', 'Donker'],
];

// Kept in sync with EditorApp.editorPanelFlags so toggling from the library
// affects the editor the next time it mounts.
const editorPanelFlags = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
];

const route = useRoute();
const router = useRouter();

// Active traject (null = global browse). Derived from the URL via
// `route.params.trajectRef`, so the new `library-traject` route makes the
// bibliotheek traject-aware without any extra plumbing.
const { activeTrajectRef } = useTrajects();

// Keep the user's traject scope across in-app navigations. With a traject
// in the URL we stay on `library-traject` / `editor-traject`; without one
// on the plain `library` / `editor`. Mirrors EditorApp.editorRouteFor.
function libraryRouteFor(params = {}) {
  return activeTrajectRef.value
    ? { name: 'library-traject', params: { ...params, trajectRef: activeTrajectRef.value } }
    : { name: 'library', params };
}
function editorRouteFor(lawIdVal, articleNumber) {
  return activeTrajectRef.value
    ? { name: 'editor-traject', params: { trajectRef: activeTrajectRef.value, lawId: lawIdVal, articleNumber } }
    : { name: 'editor', params: { lawId: lawIdVal, articleNumber } };
}

// Tab-bar state + cross-section navigation. The Bibliotheek/Editor tabs
// must light up for both the plain and traject-scoped route variants, and
// switching to the Editor tab must carry the active traject across (see
// sectionTarget).
const isLibraryRoute = computed(
  () => route.name === 'library' || route.name === 'library-traject',
);
// The Editor tab is never the active tab while LibraryApp is mounted
// (this component only serves the library routes), so it needs no
// `:selected` binding — only the cross-section target/href below.
const editorTabTarget = computed(() =>
  sectionTarget(router, lastEditorPath.value, activeTrajectRef.value),
);
const editorTabHref = computed(() => router.resolve(editorTabTarget.value).href);

const laws = ref([]);
const favorites = ref(null);
// Law ids edited in the active traject (branch-vs-base diff). `null` until
// loaded / when no traject is active; a Set once the endpoint resolves.
const changedLawIds = ref(null);
const loading = ref(true);
const indexError = ref(null);
const searchPopoverRef = ref(null);

function openSearch(e, initialSearch = '') {
  searchPopoverRef.value?.show(e?.currentTarget, initialSearch);
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

const selectedLawId = ref(null);
const selectedLaw = shallowRef(null);
const selectedLawLoading = ref(false);
const lawError = ref(null);
const selectedArticleNumber = ref(null);
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

  return sections;
});

const articles = computed(() => selectedLaw.value?.articles ?? []);

/**
 * Humanize a snake_case law identifier into Title Case Words.
 * `burgerlijk_wetboek_boek_5` → `Burgerlijk Wetboek Boek 5`.
 *
 * Used as a consistent fallback in both the sidebar list and the
 * secondary-sidebar header when a law has no explicit `name`.
 *
 * Orphan prevention is gedaan via CSS `text-wrap: pretty` op de
 * tekst-componenten zelf, niet hier — data blijft schoon.
 */
function humanizeLawId(id) {
  return String(id ?? "").replace(/_/g, " ").replace(/\b\w/g, c => c.toUpperCase());
}

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
//   "Art. 5 · Wet op de zorgtoeslag · RegelRecht"
// Most-specific first so browser tab truncation preserves the article number.
// We deliberately omit the "Bibliotheek:" prefix here (unlike the editor) —
// browsing laws is the implicit default, and the law name carries enough
// context. The editor still prefixes because "Editor:" disambiguates the
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
  document.title = detail.length > 0
    ? `${detail.join(' · ')} · RegelRecht`
    : 'Bibliotheek · RegelRecht';
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

// Financieel CV — TIJDELIJKE seed (alleen deze branch, lokale dev).
// Zonder auth/DB geeft /api/favorites niets terug, dus zou de bibliotheek
// leeg zijn. Tot favorieten via auth lopen tonen we hier standaard de
// financieel CV-wetten zodat ze meteen in de zijbalk staan.
// Verwijderen zodra dit via echte favorieten (auth + DB) gaat.
const FINANCIEEL_CV_FALLBACK_LAWS = [
  'ziektewet',                                              // NRP
  'wet_tegemoetkomingen_loondomein',                       // LKV / LIV
  'participatiewet',                                        // LKS
  'wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten', // LDP
  'wet_werk_en_inkomen_naar_arbeidsvermogen',              // bron: WIA (JC/WPA)
  'werkloosheidswet',                                      // bron: WW (PP)
];

async function loadFavorites() {
  let favIds = null;
  try {
    const res = await fetch('/api/favorites');
    if (res.ok) {
      favIds = await res.json();
    } else if (res.status >= 500) {
      console.warn(`Failed to load favorites: ${res.status}`);
    }
  } catch {
    // Not authenticated or endpoint unavailable — no favorites
  }
  // Seed de financieel CV-wetten als er geen server-side favorieten zijn.
  favorites.value = new Set(favIds && favIds.length ? favIds : FINANCIEEL_CV_FALLBACK_LAWS);
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
    const res = await fetch(changedLawsUrl(trajectRef));
    if (!res.ok) return null;
    return new Set(await res.json());
  } catch {
    return null;
  }
}

const togglingFavorites = ref(new Set());

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
    const res = await fetch(`/api/favorites/${encodeURIComponent(lawId)}`, { method });
    if (!res.ok) {
      revert();
    } else {
      // Re-resolve the sidebar's id-set so a newly-favorited law (whose
      // metadata isn't loaded yet, since we only fetch favorites + edits by
      // id) appears in the Favorieten section without a manual reload.
      loadIndex();
    }
  } catch {
    revert();
  } finally {
    togglingFavorites.value.delete(lawId);
  }
}

let loadIndexGeneration = 0;

async function loadIndex() {
  const gen = ++loadIndexGeneration;
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
    if (gen !== loadIndexGeneration) return;
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
    const res = await fetch(lawsListUrl(trajectRef, query));
    if (!res.ok) throw new Error(`Failed to load corpus: ${res.status}`);
    // Gate before and after json(): skip parsing for stale 200s, and catch races during it.
    if (gen !== loadIndexGeneration) return;
    const corpusLaws = await res.json();
    if (gen !== loadIndexGeneration) return;
    laws.value = corpusLaws.sort((a, b) => a.law_id.localeCompare(b.law_id));
  } catch (e) {
    if (gen !== loadIndexGeneration) return;
    indexError.value = e;
  } finally {
    if (gen === loadIndexGeneration) {
      loading.value = false;
    }
  }
}

let loadLawGeneration = 0;

async function loadLaw(lawId) {
  const gen = ++loadLawGeneration;
  try {
    selectedLawLoading.value = true;
    const res = await fetch(lawUrl(activeTrajectRef.value, lawId));
    if (!res.ok) throw lawFetchError(res.status);
    // Gate before and after `res.text()`: skip the body read for stale 200s, and catch races during it.
    if (gen !== loadLawGeneration) return;
    const text = await res.text();
    if (gen !== loadLawGeneration) return;
    selectedLaw.value = yaml.load(text);
    // selectedArticleNumber is set from the route on initial mount and via
    // onBeforeRouteUpdate; we don't validate here so an invalid number
    // surfaces as the articleNotFound error state instead of being silently
    // stripped from the URL.
  } catch (e) {
    if (gen !== loadLawGeneration) return;
    selectedLaw.value = null;
    lawError.value = e;
  } finally {
    if (gen === loadLawGeneration) {
      selectedLawLoading.value = false;
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
  loadLaw(selectedLawId.value);
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

function selectLaw(lawId, focusAfter = false) {
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
  // doesn't delegate focus).
  if (focusAfter) {
    nextTick(() => {
      const item = document.querySelector(`[data-law-id="${CSS.escape(lawId)}"]`);
      const action = item?.shadowRoot?.querySelector('.list-item__action');
      action?.focus?.();
    });
  }
}

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
  await fetch('/api/corpus/reload', {
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
  <nldd-app-view>
    <nldd-bar-split-view>
      <!-- Primary Bar: md only — search and settings as buttons -->
      <nldd-split-view-pane slot="primary-bar-md" only="md">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :href="editorTabHref" @click.prevent="router.push(editorTabTarget)" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="md" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <TrajectMenu id-suffix="lib-md" />
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

      <!-- Primary Bar: lg+ — search as input field in center slot -->
      <nldd-split-view-pane slot="primary-bar-lg" above="lg">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :href="editorTabHref" @click.prevent="router.push(editorTabTarget)" text="Editor"></nldd-tab-bar-item>
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
            <nldd-toolbar-item slot="end">
              <TrajectMenu id-suffix="lib-lg" />
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

      <!-- Main: Navigation Split View -->
      <nldd-split-view-pane slot="main">
        <nldd-navigation-split-view @back="onPaneBack">

          <!-- Sidebar hidden on corpus load failure so the main pane carries the error alone (mirrors law-load failure pattern). -->
          <nldd-split-view-pane v-if="!indexError" slot="sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" :text="LIBRARY_HOME_TITLE" collapse-anchor="home-titel"></nldd-top-title-bar>

              <nldd-simple-section width="full">
                <nldd-title id="home-titel" size="3"><h3>{{ LIBRARY_HOME_TITLE }}</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-inline-dialog v-if="loading" text="Laden..."></nldd-inline-dialog>
                <!-- Nothing curated yet (no favorites, no traject edits): point
                     the user at search rather than dumping the whole corpus. -->
                <nldd-inline-dialog
                  v-else-if="sidebarSections.length === 0"
                  text="Nog niets in je bibliotheek"
                  supporting-text="Zoek een wet om te openen, of markeer wetten als favoriet."
                >
                  <nldd-button slot="actions" variant="primary" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
                </nldd-inline-dialog>
                <template v-else>
                  <template
                    v-for="(section, sectionIndex) in sidebarSections"
                    :key="section.key"
                  >
                    <!-- Gap above every section after the first, so the
                         curated groups read as distinct blocks. -->
                    <nldd-spacer v-if="sectionIndex > 0" size="24"></nldd-spacer>
                    <template v-if="section.title">
                      <nldd-title size="5"><h4>{{ section.title }}</h4></nldd-title>
                      <nldd-spacer size="8"></nldd-spacer>
                    </template>
                    <nldd-list variant="simple">
                      <nldd-list-item
                        v-for="law in section.laws"
                        :key="`${section.key}-${law.law_id}`"
                        size="md"
                        type="button"
                        :data-law-id="law.law_id"
                        :selected="law.law_id === selectedLawId || undefined"
                        @click="selectLaw(law.law_id)"
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
          <nldd-split-view-pane v-if="selectedLawId && !lawError && !indexError" slot="secondary-sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="lawName || indexedLawName || 'Selecteer een wet'"
                :back-text="LIBRARY_HOME_BACK_TEXT"
                collapse-anchor="wet-titel"
              ></nldd-top-title-bar>

              <nldd-simple-section width="full">
                <nldd-title id="wet-titel" size="3"><h3>{{ lawName || 'Selecteer een wet' }}</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-toolbar v-if="authenticated && selectedLaw" label="Favorieten">
                  <nldd-toolbar-item slot="start">
                    <nldd-icon-button
                      :icon="favorites?.has(selectedLawId) ? 'heart-filled' : 'heart'"
                      :text="favorites?.has(selectedLawId) ? 'Verwijder uit favorieten' : 'Voeg toe aan favorieten'"
                      @click="toggleFavorite(selectedLawId)"
                    ></nldd-icon-button>
                  </nldd-toolbar-item>
                </nldd-toolbar>
                <nldd-spacer v-if="authenticated && selectedLaw" size="16"></nldd-spacer>
                <nldd-inline-dialog v-if="selectedLawLoading" text="Laden..."></nldd-inline-dialog>
                <nldd-inline-dialog v-else-if="!selectedLaw" text="Selecteer een wet"></nldd-inline-dialog>
                <nldd-list v-else variant="simple">
                  <nldd-list-item
                    v-for="article in articles"
                    :key="article.number"
                    size="md"
                    type="button"
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
          <nldd-split-view-pane slot="main" :has-content="selectedArticle || lawError || articleNotFound || indexError ? true : undefined">
            <nldd-page sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="selectedArticle ? `Artikel ${selectedArticle.number}` : undefined"
                :supporting-text="selectedArticle ? lawName : undefined"
                :back-text="indexError ? undefined : (lawError ? LIBRARY_HOME_BACK_TEXT : (lawName || 'Terug'))"
                :collapse-anchor="selectedArticle ? 'article-titel' : undefined"
              ></nldd-top-title-bar>

              <nldd-simple-section width="full" v-if="indexError">
                <nldd-inline-dialog
                  variant="alert"
                  text="Wetten en regels zijn niet geladen"
                  supporting-text="De gegevens konden niet worden opgehaald."
                >
                  <nldd-button slot="actions" variant="primary" text="Probeer opnieuw" @click="retryLoadCorpus"></nldd-button>
                  <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="!selectedLawId">
                <nldd-inline-dialog text="Selecteer een wet"></nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="selectedLawLoading">
                <!-- Loading takes precedence over `lawError` to avoid flashing a stale error during a refetch. -->
                <nldd-inline-dialog text="Wet laden…"></nldd-inline-dialog>
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
                      <nldd-tab-bar size="md">
                        <nldd-tab-bar-item :selected="detailView === 'tekst' || undefined" text="Tekst" @click="detailView = 'tekst'"></nldd-tab-bar-item>
                        <nldd-tab-bar-item :selected="detailView === 'machine' || undefined" text="Machine" @click="detailView = 'machine'"></nldd-tab-bar-item>
                        <nldd-tab-bar-item :selected="detailView === 'yaml' || undefined" text="YAML" @click="detailView = 'yaml'"></nldd-tab-bar-item>
                      </nldd-tab-bar>
                    </nldd-toolbar-item>
                    <nldd-toolbar-item slot="end">
                      <nldd-button v-if="selectedLawId" variant="secondary" text="Bewerken" :href="editLawHref" @click.prevent="router.push(editLawTarget)"></nldd-button>
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
      </nldd-split-view-pane>

      <!-- Mobile Bar (sm only): tab bar + icon-buttons for search and settings -->
      <nldd-split-view-pane slot="mobile-bar" only="sm">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar variant="compact" navigation>
                <nldd-tab-bar-item :selected="isLibraryRoute || undefined" icon="books" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :href="editorTabHref" @click.prevent="router.push(editorTabTarget)" icon="edit" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <span>
                <nldd-icon-button size="lg" icon="search" text="Zoeken" @click="openSearch"></nldd-icon-button>
              </span>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <TrajectMenu id-suffix="lib-sm" />
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

  <!-- LibraryApp is a read-only browser; ActionSheet is mounted without editable
       so the output field is hidden and the footer button just closes the sheet. -->
  <ActionSheet :action="activeAction" :article="selectedArticle" :editable="false" @close="activeAction = null" @save="activeAction = null" @edit="editInEditor" />
  <SearchPopover
    ref="searchPopoverRef"
    @select-law="(lawId) => selectLaw(lawId, true)"
    @harvest-available="onHarvestAvailable"
  />
  <!-- Traject-documents browser sheet + edit window, opened from TrajectMenu. -->
  <TrajectDocuments />
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

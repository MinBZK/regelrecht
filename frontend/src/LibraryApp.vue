<script setup>
import { ref, computed, shallowRef, nextTick, watchEffect } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import yaml from 'js-yaml';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';
import SearchPopover from './components/SearchPopover.vue';
import { useAuth } from './composables/useAuth.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useColorScheme } from './composables/useColorScheme.js';
import { SUPPORT_EMAIL } from './constants.js';
import { lastEditorPath } from './composables/useLastVisitedRoute.js';

const { authenticated, loading: authLoading, oidcConfigured, person, login, logout } = useAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();
const { colorScheme, setColorScheme } = useColorScheme();

// Single source of truth for the library home title — used as the
// sidebar header and as the back-text on the secondary-sidebar so the
// two stay in sync.
const LIBRARY_HOME_TITLE = 'Wetten en regels';

const colorSchemeOptions = [
  ['auto', 'Automatisch'],
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

const laws = ref([]);
const favorites = ref(null);
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
    // unknown value is a programmer error, not a user-supplied string.
    // Bail silently: no production code path can reach this branch, and
    // a console.warn would just be noise if a future tab gets added
    // without updating VIEW_TO_HASH.
    const hash = VIEW_TO_HASH[value];
    if (!hash) return;
    if (hash !== route.hash) {
      router.replace({ path: route.path, query: route.query, hash });
    }
  },
});
const activeAction = ref(null);

const sidebarLaws = computed(() => {
  const list = laws.value;
  if (favorites.value) {
    const favList = list.filter(law => favorites.value.has(law.law_id));
    if (favList.length > 0) return favList;
  }
  return list;
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

async function loadFavorites() {
  try {
    const res = await fetch('/api/favorites');
    if (res.ok) {
      const favIds = await res.json();
      favorites.value = new Set(favIds);
    } else if (res.status >= 500) {
      console.warn(`Failed to load favorites: ${res.status}`);
    }
  } catch {
    // Not authenticated or endpoint unavailable — no favorites
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
    if (!res.ok) revert();
  } catch {
    revert();
  } finally {
    togglingFavorites.value.delete(lawId);
  }
}

async function loadIndex() {
  try {
    const [corpusRes] = await Promise.all([
      fetch('/api/corpus/laws?limit=1000'),
      loadFavorites(),
    ]);
    if (!corpusRes.ok) throw new Error(`Failed to load corpus: ${corpusRes.status}`);
    const corpusLaws = await corpusRes.json();

    laws.value = corpusLaws.sort((a, b) => a.law_id.localeCompare(b.law_id));
  } catch (e) {
    indexError.value = e;
  } finally {
    loading.value = false;
  }
}

let loadLawGeneration = 0;

async function loadLaw(lawId) {
  const gen = ++loadLawGeneration;
  try {
    selectedLawLoading.value = true;
    const res = await fetch(`/api/corpus/laws/${encodeURIComponent(lawId)}`);
    if (!res.ok) throw new Error(`Failed to fetch: ${res.status}`);
    if (gen !== loadLawGeneration) return; // stale response, discard
    const text = await res.text();
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
  router.push(`/editor/${encodeURIComponent(selectedLawId.value)}/${encodeURIComponent(selectedArticleNumber.value)}`);
}

function selectLaw(lawId, focusAfter = false) {
  if (lawId !== selectedLawId.value || lawError.value) {
    selectedLawId.value = lawId;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
    router.push({ name: 'library', params: { lawId } });
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
  router.replace({ name: 'library', params: { lawId: selectedLawId.value, articleNumber: articleStr }, hash: route.hash });
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
    router.push({ name: 'library', params: { lawId: selectedLawId.value } });
  }
}

function goToLibraryRoot() {
  router.push({ name: 'library' });
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
</script>

<template>
  <nldd-app-view>
    <nldd-bar-split-view>
      <!-- Primary Bar: md only — search and settings as buttons -->
      <nldd-split-view-pane slot="primary-bar-md" only="md">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md">
                <nldd-tab-bar-item selected text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :href="lastEditorPath" @click.prevent="router.push(lastEditorPath)" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="md" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button id="settings-menu-btn-md" size="md" start-icon="global-settings" text="Instellingen" expandable popovertarget="settings-menu-md"></nldd-button>
              <nldd-menu id="settings-menu-md" anchor="settings-menu-btn-md">
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
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-md-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
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
              <nldd-tab-bar size="md">
                <nldd-tab-bar-item selected text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :href="lastEditorPath" @click.prevent="router.push(lastEditorPath)" text="Editor"></nldd-tab-bar-item>
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
              <nldd-button id="settings-menu-btn-lg" size="md" start-icon="global-settings" text="Instellingen" expandable popovertarget="settings-menu-lg"></nldd-button>
              <nldd-menu id="settings-menu-lg" anchor="settings-menu-btn-lg">
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
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-lg-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
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

          <!-- Sidebar: Wetten Browser. Hidden on corpus load failure so the
               navigation-split-view collapses and the main pane carries the
               error state on its own (mirrors the law-load failure pattern). -->
          <nldd-split-view-pane v-if="!indexError" slot="sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" :text="LIBRARY_HOME_TITLE" collapse-anchor="home-titel"></nldd-top-title-bar>

              <nldd-simple-section full-width>
                <nldd-title id="home-titel" size="3"><h3>{{ LIBRARY_HOME_TITLE }}</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-inline-dialog v-if="loading" text="Laden..."></nldd-inline-dialog>
                <nldd-list v-else variant="simple">
                  <nldd-list-item
                    v-for="law in sidebarLaws"
                    :key="law.law_id"
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
                :back-text="LIBRARY_HOME_TITLE"
                collapse-anchor="wet-titel"
              ></nldd-top-title-bar>

              <nldd-simple-section full-width>
                <nldd-title id="wet-titel" size="3"><h3>{{ lawName || 'Selecteer een wet' }}</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
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
                :back-text="indexError ? undefined : (lawError ? LIBRARY_HOME_TITLE : (lawName || 'Terug'))"
                :collapse-anchor="selectedArticle ? 'article-titel' : undefined"
              ></nldd-top-title-bar>

              <nldd-simple-section full-width v-if="indexError">
                <nldd-inline-dialog
                  variant="alert"
                  text="Wetten en regels zijn niet geladen"
                  supporting-text="De gegevens konden niet worden opgehaald."
                >
                  <nldd-button slot="actions" variant="primary" text="Probeer opnieuw" @click="retryLoadCorpus"></nldd-button>
                  <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section full-width v-else-if="!selectedLawId">
                <nldd-inline-dialog text="Selecteer een wet"></nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section full-width v-else-if="lawError">
                <nldd-inline-dialog
                  variant="alert"
                  :text="`${indexedLawName} is niet geladen`"
                  supporting-text="De gegevens konden niet worden opgehaald."
                >
                  <nldd-button slot="actions" variant="primary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
                  <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section full-width v-else-if="articleNotFound">
                <nldd-inline-dialog
                  variant="alert"
                  :text="`Artikel ${selectedArticleNumber} van ${lawName || indexedLawName} bestaat niet`"
                  supporting-text="Mogelijk klopt de URL niet. Neem contact op als je verwacht dat dit artikel wel bestaat."
                >
                  <nldd-button slot="actions" class="article-not-found__back-button" variant="primary" text="Bekijk artikelen" @click="goToLawRoot"></nldd-button>
                  <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section full-width v-else-if="!selectedArticle">
                <nldd-inline-dialog text="Selecteer een artikel"></nldd-inline-dialog>
              </nldd-simple-section>
              <template v-else>
                <nldd-simple-section full-width>
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
                      <nldd-button v-if="selectedLawId" variant="secondary" text="Bewerken" :href="`/editor/${encodeURIComponent(selectedLawId)}/${encodeURIComponent(selectedArticleNumber)}`" @click.prevent="router.push(`/editor/${encodeURIComponent(selectedLawId)}/${encodeURIComponent(selectedArticleNumber)}`)"></nldd-button>
                    </nldd-toolbar-item>
                  </nldd-toolbar>
                  <nldd-spacer size="24"></nldd-spacer>
                  <KeepAlive>
                    <ArticleText v-if="detailView === 'tekst'" :article="selectedArticle" />
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
              <nldd-tab-bar compact>
                <nldd-tab-bar-item selected icon="stack" text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item :href="lastEditorPath" @click.prevent="router.push(lastEditorPath)" icon="edit" text="Editor"></nldd-tab-bar-item>
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
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-sm-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
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
    :laws="laws"
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

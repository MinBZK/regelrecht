<script setup>
import { ref, computed, shallowRef } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import yaml from 'js-yaml';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';
import SearchWindow from './components/SearchWindow.vue';
import { useAuth } from './composables/useAuth.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';

const { authenticated, loading: authLoading, oidcConfigured, person, login, logout } = useAuth();
const { isEnabled, toggle: toggleFlag } = useFeatureFlags();

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
const searchOpen = ref(false);

function openSearch() {
  if (searchOpen.value) return;
  searchOpen.value = true;
}

const selectedLawId = ref(null);
const selectedLaw = shallowRef(null);
const selectedLawLoading = ref(false);
const lawError = ref(null);
const selectedArticleNumber = ref(null);
const detailView = ref('tekst');
const activeAction = ref(null);

function onDetailViewChange(event) {
  const value = event.target?.value ?? event.detail?.[0];
  if (value) detailView.value = value;
}

const sidebarLaws = computed(() => {
  const list = laws.value;
  if (favorites.value) {
    const favList = list.filter(law => favorites.value.has(law.law_id));
    if (favList.length > 0) return favList;
  }
  return list;
});

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
  return nameRef || selectedLaw.value.$id || '';
});

const selectedArticle = computed(() => {
  if (!selectedArticleNumber.value) return null;
  return articles.value.find(
    (a) => String(a.number) === String(selectedArticleNumber.value)
  ) ?? null;
});

function displayName(law) {
  if (law.name) return law.name;
  return law.law_id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
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

    // Only auto-select if no law specified in route
    if (!route.params.lawId) {
      let startList = laws.value;
      if (favorites.value) {
        const favList = laws.value.filter(l => favorites.value.has(l.law_id));
        if (favList.length > 0) startList = favList;
      }
      if (startList.length > 0) {
        const firstLawId = startList[0].law_id;
        selectedLawId.value = firstLawId;
        loadLaw(firstLawId);
        router.replace({ name: 'library', params: { lawId: firstLawId } });
      }
    }
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
    if (articles.value.length > 0) {
      // Use article from route if valid; otherwise show nothing (empty state).
      const routeArticle = route.params.articleNumber;
      if (routeArticle && articles.value.some(a => String(a.number) === String(routeArticle))) {
        selectedArticleNumber.value = String(routeArticle);
      } else if (routeArticle) {
        // Route had an invalid article number — strip it so the URL reflects the empty state.
        router.replace({ name: 'library', params: { lawId } });
      }
    }
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

function editInEditor() {
  if (!selectedLawId.value || !selectedArticleNumber.value) return;
  activeAction.value = null;
  router.push(`/editor/${encodeURIComponent(selectedLawId.value)}/${encodeURIComponent(selectedArticleNumber.value)}`);
}

function selectLaw(lawId) {
  if (lawId === selectedLawId.value && !lawError.value) return;
  selectedLawId.value = lawId;
  selectedArticleNumber.value = null;
  activeAction.value = null;
  lawError.value = null;
  router.push({ name: 'library', params: { lawId } });
  loadLaw(lawId);
}

function selectArticle(number) {
  const articleStr = String(number);
  if (articleStr === selectedArticleNumber.value) return;
  selectedArticleNumber.value = articleStr;
  activeAction.value = null;
  router.replace({ name: 'library', params: { lawId: selectedLawId.value, articleNumber: articleStr } });
}

// Handle browser back/forward navigation
onBeforeRouteUpdate((to) => {
  const newLawId = to.params.lawId;
  const newArticle = to.params.articleNumber;

  if (!newLawId) {
    // Navigated to /library with no lawId — reset and redirect to first law
    selectedLawId.value = null;
    selectedLaw.value = null;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
    const list = sidebarLaws.value;
    if (list.length > 0) {
      const firstLawId = list[0].law_id;
      selectedLawId.value = firstLawId;
      loadLaw(firstLawId);
      return { name: 'library', params: { lawId: firstLawId } };
    }
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
  loadLaw(route.params.lawId);
}
loadIndex();
</script>

<template>
  <nldd-app-view>
    <nldd-bar-split-view>
      <!-- Primary Bar: App Toolbar -->
      <nldd-split-view-pane slot="primary-bar">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md">
                <nldd-tab-bar-item selected text="Bibliotheek"></nldd-tab-bar-item>
                <nldd-tab-bar-item href="/editor" @click.prevent="router.push('/editor')" text="Editor"></nldd-tab-bar-item>
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
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Main: Navigation Split View -->
      <nldd-split-view-pane slot="main">
        <nldd-navigation-split-view>

          <!-- Sidebar: Wetten Browser -->
          <nldd-split-view-pane slot="sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" text="Wetten en regels" collapse-anchor="home-titel"></nldd-top-title-bar>

              <nldd-simple-section :align="loading || indexError ? 'center' : undefined">
                <nldd-title id="home-titel" size="3"><h3>Wetten en regels</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-inline-dialog v-if="loading" text="Laden..."></nldd-inline-dialog>
                <nldd-inline-dialog v-else-if="indexError" variant="alert" text="Fout bij laden" :supporting-text="indexError.message"></nldd-inline-dialog>
                <nldd-list v-else variant="simple">
                  <nldd-list-item
                    v-for="law in sidebarLaws"
                    :key="law.law_id"
                    size="md"
                    type="button"
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

          <!-- Secondary Sidebar: Artikelen Lijst -->
          <nldd-split-view-pane slot="secondary-sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="lawName || 'Selecteer een wet'"
                back-text="Wetten"
                collapse-anchor="wet-titel"
              ></nldd-top-title-bar>

              <nldd-simple-section :align="selectedLawLoading || lawError || !selectedLaw ? 'center' : undefined">
                <nldd-title id="wet-titel" size="3"><h3>{{ lawName || 'Selecteer een wet' }}</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-inline-dialog v-if="selectedLawLoading" text="Laden..."></nldd-inline-dialog>
                <nldd-inline-dialog v-else-if="lawError" variant="alert" text="Fout bij laden" :supporting-text="lawError.message"></nldd-inline-dialog>
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
          <nldd-split-view-pane slot="main" :has-content="selectedArticle ? true : undefined">
            <nldd-page sticky-header>
              <nldd-top-title-bar
                v-if="selectedArticle"
                slot="header"
                :text="`Artikel ${selectedArticle.number}`"
                :supporting-text="lawName"
                :back-text="lawName || 'Terug'"
                collapse-anchor="article-titel"
              ></nldd-top-title-bar>

              <nldd-simple-section v-if="!selectedArticle" align="center">
                <nldd-inline-dialog text="Selecteer een artikel"></nldd-inline-dialog>
              </nldd-simple-section>
              <template v-else>
                <nldd-simple-section>
                  <nldd-title id="article-titel" size="3">
                    <h3>Artikel {{ selectedArticle.number }}</h3>
                    <span slot="subtitle">{{ lawName }}</span>
                  </nldd-title>
                  <nldd-spacer size="16"></nldd-spacer>
                  <nldd-toolbar>
                    <nldd-toolbar-item slot="start">
                      <nldd-segmented-control size="md" :value="detailView" @change="onDetailViewChange">
                        <nldd-segmented-control-item value="tekst" text="Tekst"></nldd-segmented-control-item>
                        <nldd-segmented-control-item value="machine" text="Machine"></nldd-segmented-control-item>
                        <nldd-segmented-control-item value="yaml" text="YAML"></nldd-segmented-control-item>
                      </nldd-segmented-control>
                    </nldd-toolbar-item>
                    <nldd-toolbar-item slot="end">
                      <a v-if="selectedLawId" :href="`/editor/${encodeURIComponent(selectedLawId)}/${encodeURIComponent(selectedArticleNumber)}`" @click.prevent="router.push(`/editor/${encodeURIComponent(selectedLawId)}/${encodeURIComponent(selectedArticleNumber)}`)">
                        <nldd-button variant="primary" text="Bewerk"></nldd-button>
                      </a>
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
    </nldd-bar-split-view>
  </nldd-app-view>

  <!-- LibraryApp is a read-only browser; ActionSheet is mounted without editable
       so the output field is hidden and the footer button just closes the sheet. -->
  <ActionSheet :action="activeAction" :article="selectedArticle" :editable="false" @close="activeAction = null" @save="activeAction = null" @edit="editInEditor" />
  <SearchWindow
    v-model="searchOpen"
    :laws="laws"
    @select-law="selectLaw"
    @harvest-available="onHarvestAvailable"
  />
</template>

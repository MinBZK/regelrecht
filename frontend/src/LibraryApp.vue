<script setup>
import { ref, computed, shallowRef } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import yaml from 'js-yaml';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';
import { useAuth } from './composables/useAuth.js';

const { authenticated, loading: authLoading, oidcConfigured, person, login, logout } = useAuth();

const route = useRoute();
const router = useRouter();

const laws = ref([]);
const favorites = ref(null);
const loading = ref(true);
const indexError = ref(null);
const search = ref('');

const selectedLawId = ref(null);
const selectedLaw = shallowRef(null);
const selectedLawLoading = ref(false);
const lawError = ref(null);
const selectedArticleNumber = ref(null);
const detailView = ref('machine');
const activeAction = ref(null);

const filteredLaws = computed(() => {
  let list = laws.value;
  if (favorites.value) {
    const favList = list.filter(law => favorites.value.has(law.law_id));
    if (favList.length > 0) list = favList;
  }
  const q = search.value.toLowerCase();
  if (!q) return list;
  return list.filter(law =>
    law.law_id.toLowerCase().includes(q) ||
    displayName(law).toLowerCase().includes(q)
  );
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
  const firstLine = article.text.split('\n')[0];
  return firstLine.length > 80 ? firstLine.slice(0, 80) + '...' : firstLine;
}

async function loadIndex() {
  try {
    const [corpusRes, favRes] = await Promise.all([
      fetch('/api/corpus/laws?limit=1000'),
      fetch('/favorites.json'),
    ]);
    if (!corpusRes.ok) throw new Error(`Failed to load corpus: ${corpusRes.status}`);
    const corpusLaws = await corpusRes.json();

    if (favRes.ok) {
      const favIds = await favRes.json();
      favorites.value = new Set(favIds);
    }

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
      // Use article from route if valid, otherwise select first
      const routeArticle = route.params.articleNumber;
      if (routeArticle && articles.value.some(a => String(a.number) === String(routeArticle))) {
        selectedArticleNumber.value = String(routeArticle);
      } else {
        selectedArticleNumber.value = String(articles.value[0].number);
        // Correct URL if the route had an invalid article number
        if (routeArticle) {
          router.replace({ name: 'library', params: { lawId, articleNumber: selectedArticleNumber.value } });
        }
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
    const list = filteredLaws.value;
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
    } else if (articles.value.length > 0) {
      selectedArticleNumber.value = String(articles.value[0].number);
      activeAction.value = null;
    }
  }
});

// Initial load from route
if (route.params.lawId) {
  selectedLawId.value = route.params.lawId;
  loadLaw(route.params.lawId);
}
loadIndex();
</script>

<template>
  <ndd-app-view>
    <ndd-bar-split-view>
      <!-- Primary Bar: App Toolbar -->
      <ndd-split-view-pane slot="primary-bar">
        <ndd-container padding="8">
          <ndd-toolbar size="md">
            <ndd-toolbar-item slot="start">
              <ndd-tab-bar size="md">
                <ndd-tab-bar-item selected text="Bibliotheek"></ndd-tab-bar-item>
                <ndd-tab-bar-item href="/editor" @click.prevent="router.push('/editor')" text="Editor"></ndd-tab-bar-item>
              </ndd-tab-bar>
            </ndd-toolbar-item>
            <ndd-toolbar-item slot="center" min-width="240px" width="40%">
              <ndd-search-field
                size="md"
                placeholder="Zoeken"
                @input="search = $event.target.value"
              ></ndd-search-field>
            </ndd-toolbar-item>
            <ndd-toolbar-item slot="end">
              <ndd-button-bar size="md">
                <ndd-button id="project-menu-btn" size="md" expandable text="RR Project" popovertarget="project-menu"></ndd-button>
                <ndd-menu id="project-menu" anchor="project-menu-btn">
                  <ndd-menu-item text="Instellingen"></ndd-menu-item>
                  <ndd-menu-item text="Leden"></ndd-menu-item>
                  <ndd-menu-divider></ndd-menu-divider>
                  <ndd-menu-item text="Nieuw project"></ndd-menu-item>
                </ndd-menu>
                <ndd-button-bar-divider></ndd-button-bar-divider>
                <ndd-icon-button id="account-menu-btn" size="md" icon="person-circle" expandable :title="person?.name || 'Account'" popovertarget="account-menu">
                </ndd-icon-button>
                <ndd-menu id="account-menu" anchor="account-menu-btn">
                  <template v-if="!authLoading && authenticated">
                    <ndd-menu-item :text="person?.name || person?.email" disabled></ndd-menu-item>
                    <ndd-menu-divider></ndd-menu-divider>
                    <ndd-menu-item text="Uitloggen" @click="logout"></ndd-menu-item>
                  </template>
                  <template v-else-if="!authLoading && oidcConfigured">
                    <ndd-menu-item text="Inloggen" @click="login"></ndd-menu-item>
                  </template>
                </ndd-menu>
              </ndd-button-bar>
            </ndd-toolbar-item>
          </ndd-toolbar>
        </ndd-container>
      </ndd-split-view-pane>

      <!-- Main: Navigation Split View -->
      <ndd-split-view-pane slot="main">
        <ndd-navigation-split-view>

          <!-- Sidebar: Wetten Browser -->
          <ndd-split-view-pane slot="sidebar" has-content>
            <ndd-page sticky-header>
              <ndd-top-title-bar slot="header" text="Wetten en regels" collapse-anchor="home-titel"></ndd-top-title-bar>

              <ndd-simple-section :align="loading || indexError ? 'center' : undefined">
                <ndd-title id="home-titel" size="3"><h3>Wetten en regels</h3></ndd-title>
                <ndd-spacer size="16"></ndd-spacer>
                <ndd-inline-dialog v-if="loading" text="Laden..."></ndd-inline-dialog>
                <ndd-inline-dialog v-else-if="indexError" variant="alert" text="Fout bij laden" :supporting-text="indexError.message"></ndd-inline-dialog>
                <ndd-list v-else variant="simple">
                  <ndd-list-item
                    v-for="law in filteredLaws"
                    :key="law.law_id"
                    size="md"
                    type="button"
                    :selected="law.law_id === selectedLawId || undefined"
                    @click="selectLaw(law.law_id)"
                  >
                    <ndd-text-cell :text="displayName(law)" :supporting-text="law.source_name">
                    </ndd-text-cell>
                    <ndd-icon-cell slot="end" size="20">
                      <ndd-icon name="chevron-right"></ndd-icon>
                    </ndd-icon-cell>
                  </ndd-list-item>
                </ndd-list>
              </ndd-simple-section>
            </ndd-page>
          </ndd-split-view-pane>

          <!-- Secondary Sidebar: Artikelen Lijst -->
          <ndd-split-view-pane slot="secondary-sidebar" has-content>
            <ndd-page sticky-header>
              <ndd-top-title-bar
                slot="header"
                :text="lawName || 'Selecteer een wet'"
                back-text="Wetten"
                collapse-anchor="wet-titel"
              ></ndd-top-title-bar>

              <ndd-simple-section :align="selectedLawLoading || lawError || !selectedLaw ? 'center' : undefined">
                <ndd-title id="wet-titel" size="3"><h3>{{ lawName || 'Selecteer een wet' }}</h3></ndd-title>
                <ndd-spacer size="16"></ndd-spacer>
                <ndd-toolbar>
                  <ndd-toolbar-item slot="start">
                    <ndd-icon-button icon="heart" title="Favoriet"></ndd-icon-button>
                  </ndd-toolbar-item>
                  <ndd-toolbar-item slot="start">
                    <ndd-button text="Filter"></ndd-button>
                  </ndd-toolbar-item>
                </ndd-toolbar>
                <ndd-spacer size="16"></ndd-spacer>
                <ndd-inline-dialog v-if="selectedLawLoading" text="Laden..."></ndd-inline-dialog>
                <ndd-inline-dialog v-else-if="lawError" variant="alert" text="Fout bij laden" :supporting-text="lawError.message"></ndd-inline-dialog>
                <ndd-inline-dialog v-else-if="!selectedLaw" text="Selecteer een wet"></ndd-inline-dialog>
                <ndd-list v-else variant="simple">
                  <ndd-list-item
                    v-for="article in articles"
                    :key="article.number"
                    size="md"
                    type="button"
                    :selected="String(article.number) === String(selectedArticleNumber) || undefined"
                    @click="selectArticle(article.number)"
                  >
                    <ndd-text-cell :text="`Artikel ${article.number}`" :supporting-text="articleDescription(article)">
                    </ndd-text-cell>
                    <ndd-icon-cell slot="end" size="20">
                      <ndd-icon name="chevron-right"></ndd-icon>
                    </ndd-icon-cell>
                  </ndd-list-item>
                </ndd-list>
              </ndd-simple-section>
            </ndd-page>
          </ndd-split-view-pane>

          <!-- Main: Artikel Detail -->
          <ndd-split-view-pane slot="main" has-content>
            <ndd-page sticky-header>
              <div slot="header">
                <ndd-top-title-bar
                  :text="selectedArticle ? `Artikel ${selectedArticle.number}` : 'Selecteer een artikel'"
                  :back-text="lawName || 'Terug'"
                ></ndd-top-title-bar>
                <ndd-container padding-inline="16">
                  <ndd-toolbar>
                    <ndd-toolbar-item slot="start">
                      <ndd-tab-bar>
                        <ndd-tab-bar-item :selected="detailView === 'tekst' || undefined" @click="detailView = 'tekst'" text="Tekst"></ndd-tab-bar-item>
                        <ndd-tab-bar-item :selected="detailView === 'machine' || undefined" @click="detailView = 'machine'" text="Machine"></ndd-tab-bar-item>
                        <ndd-tab-bar-item :selected="detailView === 'yaml' || undefined" @click="detailView = 'yaml'" text="YAML"></ndd-tab-bar-item>
                      </ndd-tab-bar>
                    </ndd-toolbar-item>
                    <ndd-toolbar-item slot="end">
                      <a v-if="selectedLawId" :href="`/editor/${encodeURIComponent(selectedLawId)}?article=${encodeURIComponent(selectedArticleNumber)}`" @click.prevent="router.push(`/editor/${encodeURIComponent(selectedLawId)}?article=${encodeURIComponent(selectedArticleNumber)}`)">
                        <ndd-button variant="primary" text="Bewerk"></ndd-button>
                      </a>
                      <ndd-button v-else variant="primary" disabled text="Bewerk"></ndd-button>
                    </ndd-toolbar-item>
                  </ndd-toolbar>
                </ndd-container>
              </div>

              <ndd-simple-section v-if="!selectedArticle" align="center">
                <ndd-inline-dialog text="Selecteer een artikel"></ndd-inline-dialog>
              </ndd-simple-section>
              <template v-else>
                <KeepAlive>
                  <ArticleText v-if="detailView === 'tekst'" :article="selectedArticle" />
                  <MachineReadable v-else-if="detailView === 'machine'" :article="selectedArticle" @open-action="activeAction = $event" />
                  <YamlView v-else-if="detailView === 'yaml'" :article="selectedArticle" />
                </KeepAlive>
              </template>
            </ndd-page>
          </ndd-split-view-pane>

        </ndd-navigation-split-view>
      </ndd-split-view-pane>
    </ndd-bar-split-view>
  </ndd-app-view>

  <!-- LibraryApp is a read-only browser; ActionSheet is mounted without editable
       so the output field is hidden and the footer button just closes the sheet. -->
  <ActionSheet :action="activeAction" :article="selectedArticle" :editable="false" @close="activeAction = null" @save="activeAction = null" />
</template>

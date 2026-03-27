<script setup>
import { ref, computed, shallowRef } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import yaml from 'js-yaml';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';

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
  <rr-app-view>
    <rr-bar-split-view>
      <!-- Primary Bar: App Toolbar -->
      <rr-split-view-pane slot="primary-bar">
        <rr-container>
          <rr-toolbar size="md">
            <rr-toolbar-start-area>
              <rr-toolbar-item>
                <rr-tab-bar size="md">
                  <rr-tab-bar-item selected>Bibliotheek</rr-tab-bar-item>
                  <rr-tab-bar-item href="/editor.html">Editor</rr-tab-bar-item>
                </rr-tab-bar>
              </rr-toolbar-item>
            </rr-toolbar-start-area>
            <rr-toolbar-center-area>
              <rr-toolbar-item>
                <rr-search-field
                  size="md"
                  placeholder="Zoeken"
                  @input="search = $event.target.value"
                ></rr-search-field>
              </rr-toolbar-item>
            </rr-toolbar-center-area>
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
        </rr-container>
      </rr-split-view-pane>

      <!-- Main: Navigation Split View -->
      <rr-split-view-pane slot="main">
        <rr-navigation-split-view>

          <!-- Sidebar: Wetten Browser -->
          <rr-split-view-pane slot="sidebar" has-content>
            <rr-page sticky-header>
              <rr-top-title-bar slot="header" toolbar="none" title="Wetten en regels" container="sm"></rr-top-title-bar>

              <rr-simple-section container="sm">
                <div v-if="loading" style="padding: 32px; text-align: center;">Laden...</div>
                <div v-else-if="indexError" style="padding: 32px; text-align: center; color: #c00;">{{ indexError.message }}</div>
                <rr-list v-else variant="simple">
                  <rr-list-item
                    v-for="law in filteredLaws"
                    :key="law.law_id"
                    size="md"
                    type="button"
                    :selected="law.law_id === selectedLawId || undefined"
                    @click="selectLaw(law.law_id)"
                  >
                    <rr-text-cell>
                      <span slot="text">{{ displayName(law) }}</span>
                      <span slot="supporting-text" style="font-size: 11px; color: #888;">{{ law.source_name }}</span>
                    </rr-text-cell>
                    <rr-icon-cell slot="end" size="20">
                      <rr-icon name="chevron-right"></rr-icon>
                    </rr-icon-cell>
                  </rr-list-item>
                </rr-list>
              </rr-simple-section>
            </rr-page>
          </rr-split-view-pane>

          <!-- Secondary Sidebar: Artikelen Lijst -->
          <rr-split-view-pane slot="secondary-sidebar" has-content>
            <rr-page sticky-header>
              <rr-top-title-bar
                slot="header"
                :title="lawName || 'Selecteer een wet'"
                container="sm"
                toolbar="custom"
              >
                <div slot="toolbar-start">
                  <rr-toolbar size="md">
                    <rr-toolbar-start-area>
                      <rr-toolbar-item>
                        <rr-icon-button variant="neutral-tinted" size="s" icon="heart" title="Favoriet">
                        </rr-icon-button>
                      </rr-toolbar-item>
                      <rr-toolbar-item>
                        <rr-button variant="neutral-tinted" size="md">Filter</rr-button>
                      </rr-toolbar-item>
                    </rr-toolbar-start-area>
                    <rr-toolbar-end-area>
                      <rr-toolbar-item>
                        <a v-if="selectedLawId" :href="`/editor.html?law=${encodeURIComponent(selectedLawId)}`">
                          <rr-button variant="accent-filled" size="md">Bewerk</rr-button>
                        </a>
                        <rr-button v-else variant="accent-filled" size="md" disabled>Bewerk</rr-button>
                      </rr-toolbar-item>
                    </rr-toolbar-end-area>
                  </rr-toolbar>
                </div>
              </rr-top-title-bar>

              <rr-simple-section container="sm">
                <div v-if="selectedLawLoading" style="padding: 32px; text-align: center;">Laden...</div>
                <div v-else-if="lawError" style="padding: 32px; text-align: center; color: #c00;">{{ lawError.message }}</div>
                <div v-else-if="!selectedLaw" style="padding: 32px; text-align: center;">Selecteer een wet</div>
                <rr-list v-else variant="simple">
                  <rr-list-item
                    v-for="article in articles"
                    :key="article.number"
                    size="md"
                    type="button"
                    :selected="String(article.number) === String(selectedArticleNumber) || undefined"
                    @click="selectArticle(article.number)"
                  >
                    <rr-text-cell>
                      <span slot="text">Artikel {{ article.number }}</span>
                      <span slot="supporting-text">{{ articleDescription(article) }}</span>
                    </rr-text-cell>
                    <rr-icon-cell slot="end" size="20">
                      <rr-icon name="chevron-right"></rr-icon>
                    </rr-icon-cell>
                  </rr-list-item>
                </rr-list>
              </rr-simple-section>
            </rr-page>
          </rr-split-view-pane>

          <!-- Main: Artikel Detail -->
          <rr-split-view-pane slot="main" has-content>
            <rr-page sticky-header>
              <rr-top-title-bar
                slot="header"
                :title="selectedArticle ? `Artikel ${selectedArticle.number}` : 'Selecteer een artikel'"
                container="sm"
                toolbar="custom"
              >
                <rr-toolbar slot="toolbar-start" size="md">
                  <rr-toolbar-start-area>
                    <rr-toolbar-item>
                      <rr-segmented-control size="md" :value="detailView" @change="detailView = $event.detail.value">
                        <rr-segmented-control-item value="tekst">Tekst</rr-segmented-control-item>
                        <rr-segmented-control-item value="machine">Machine</rr-segmented-control-item>
                        <rr-segmented-control-item value="yaml">YAML</rr-segmented-control-item>
                      </rr-segmented-control>
                    </rr-toolbar-item>
                  </rr-toolbar-start-area>
                  <rr-toolbar-end-area>
                    <rr-toolbar-item>
                      <a v-if="selectedLawId" :href="`/editor.html?law=${encodeURIComponent(selectedLawId)}`">
                        <rr-button variant="accent-filled" size="md">Bewerk</rr-button>
                      </a>
                      <rr-button v-else variant="accent-filled" size="md" disabled>Bewerk</rr-button>
                    </rr-toolbar-item>
                  </rr-toolbar-end-area>
                </rr-toolbar>
              </rr-top-title-bar>

              <rr-simple-section container="sm">
                <div v-if="!selectedArticle" style="padding: 32px; text-align: center;">
                  Selecteer een artikel
                </div>
                <template v-else>
                  <div v-show="detailView === 'tekst'">
                    <ArticleText :article="selectedArticle" />
                  </div>
                  <div v-show="detailView === 'machine'">
                    <MachineReadable :article="selectedArticle" @open-action="activeAction = $event" />
                  </div>
                  <div v-show="detailView === 'yaml'">
                    <YamlView :article="selectedArticle" />
                  </div>
                </template>
              </rr-simple-section>
            </rr-page>
          </rr-split-view-pane>

        </rr-navigation-split-view>
      </rr-split-view-pane>
    </rr-bar-split-view>
  </rr-app-view>

  <ActionSheet :action="activeAction" :article="selectedArticle" @close="activeAction = null" />
</template>

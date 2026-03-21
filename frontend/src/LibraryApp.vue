<script setup>
import { ref, computed, shallowRef } from 'vue';
import yaml from 'js-yaml';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';

const laws = ref([]);
const loading = ref(true);
const error = ref(null);
const search = ref('');

const selectedLawPath = ref(null);
const selectedLaw = shallowRef(null);
const selectedLawLoading = ref(false);
const selectedArticleNumber = ref(null);
const detailView = ref('machine');
const activeAction = ref(null);

const filteredLaws = computed(() => {
  const q = search.value.toLowerCase();
  if (!q) return laws.value;
  return laws.value.filter(law =>
    law.id.toLowerCase().includes(q) ||
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
  if (law.name && !law.name.startsWith('#')) return law.name;
  return law.id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

function articleDescription(article) {
  if (!article.text) return '';
  const firstLine = article.text.split('\n')[0];
  return firstLine.length > 80 ? firstLine.slice(0, 80) + '...' : firstLine;
}

async function loadIndex() {
  try {
    const res = await fetch('/data/index.json');
    if (!res.ok) throw new Error(`Failed to load index: ${res.status}`);
    laws.value = await res.json();
    if (laws.value.length > 0 && !selectedLawPath.value) {
      selectLaw(laws.value[0].path);
    }
  } catch (e) {
    error.value = e;
  } finally {
    loading.value = false;
  }
}

async function loadLaw(path) {
  try {
    selectedLawLoading.value = true;
    const res = await fetch(path);
    if (!res.ok) throw new Error(`Failed to fetch: ${res.status}`);
    const text = await res.text();
    selectedLaw.value = yaml.load(text);
    if (articles.value.length > 0) {
      selectedArticleNumber.value = String(articles.value[0].number);
    }
  } catch (e) {
    selectedLaw.value = null;
  } finally {
    selectedLawLoading.value = false;
  }
}

function selectLaw(path) {
  selectedLawPath.value = path;
  selectedArticleNumber.value = null;
  activeAction.value = null;
  loadLaw(path);
}

function selectArticle(number) {
  selectedArticleNumber.value = String(number);
  activeAction.value = null;
}

loadIndex();
</script>

<template>
  <rr-page sticky-header>
    <!-- Top Toolbar -->
    <rr-toolbar slot="header" size="md">
      <rr-toolbar-start-area>
        <rr-toolbar-item>
          <rr-tab-bar size="md">
            <rr-tab-bar-item selected>Bibliotheek</rr-tab-bar-item>
            <rr-tab-bar-item href="editor.html">Editor</rr-tab-bar-item>
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
            <rr-icon-button variant="neutral-tinted" size="m" has-menu title="Account">
              <img slot="__icon" src="/assets/icons/person.svg" alt="Account" width="24" height="24">
            </rr-icon-button>
          </rr-button-bar>
        </rr-toolbar-item>
      </rr-toolbar-end-area>
    </rr-toolbar>

    <!-- 3-Pane Split View -->
    <rr-side-by-side-split-view panes="3">

      <!-- Pane 1: Wetten Browser -->
      <div slot="pane-1">
        <rr-page sticky-header>
          <rr-top-title-bar slot="header" toolbar="none" title="Wetten en regels" container="sm"></rr-top-title-bar>

          <rr-simple-section container="sm">
            <div v-if="loading" style="padding: 32px; text-align: center;">Laden...</div>
            <rr-list v-else variant="simple">
              <rr-list-item
                v-for="law in filteredLaws"
                :key="law.path"
                size="md"
                type="button"
                :selected="law.path === selectedLawPath || undefined"
                @click="selectLaw(law.path)"
              >
                <rr-text-cell>{{ displayName(law) }}</rr-text-cell>
                <rr-icon-cell slot="end" size="20">
                  <svg width="20" height="20" viewBox="0 0 20 20" fill="none"><path d="M7.5 5L12.5 10L7.5 15" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                </rr-icon-cell>
              </rr-list-item>
            </rr-list>
          </rr-simple-section>
        </rr-page>
      </div>

      <!-- Pane 2: Artikelen Lijst -->
      <div slot="pane-2">
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
                    <rr-icon-button variant="neutral-tinted" size="s" title="Favoriet">
                      <svg slot="__icon" width="20" height="20" viewBox="0 0 20 20" fill="none"><path d="M10 3.5C10 3.5 8 2 5.5 2C3 2 1 3.5 1 6.5C1 12 10 17 10 17C10 17 19 12 19 6.5C19 3.5 17 2 14.5 2C12 2 10 3.5 10 3.5Z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                    </rr-icon-button>
                  </rr-toolbar-item>
                  <rr-toolbar-item>
                    <rr-button variant="neutral-tinted" size="md">Filter</rr-button>
                  </rr-toolbar-item>
                </rr-toolbar-start-area>
                <rr-toolbar-end-area>
                  <rr-toolbar-item>
                    <rr-button
                      variant="accent-filled"
                      size="md"
                      :href="selectedLawPath ? `editor.html?law=${selectedLawPath}` : undefined"
                    >Bewerk</rr-button>
                  </rr-toolbar-item>
                </rr-toolbar-end-area>
              </rr-toolbar>
            </div>
          </rr-top-title-bar>

          <rr-simple-section container="sm">
            <div v-if="selectedLawLoading" style="padding: 32px; text-align: center;">Laden...</div>
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
                  <svg width="20" height="20" viewBox="0 0 20 20" fill="none"><path d="M7.5 5L12.5 10L7.5 15" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                </rr-icon-cell>
              </rr-list-item>
            </rr-list>
          </rr-simple-section>
        </rr-page>
      </div>

      <!-- Pane 3: Artikel Detail -->
      <div slot="pane-3">
        <rr-page sticky-header>
          <rr-top-title-bar
            slot="header"
            :title="selectedArticle ? `Artikel ${selectedArticle.number}` : 'Selecteer een artikel'"
            container="sm"
            toolbar="custom"
          >
            <div slot="toolbar-start">
              <rr-toolbar size="md">
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
                    <rr-button
                      variant="accent-filled"
                      size="md"
                      :href="selectedLawPath ? `editor.html?law=${selectedLawPath}` : undefined"
                    >Bewerk</rr-button>
                  </rr-toolbar-item>
                </rr-toolbar-end-area>
              </rr-toolbar>
            </div>
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
      </div>

    </rr-side-by-side-split-view>
  </rr-page>

  <ActionSheet :action="activeAction" :article="selectedArticle" @close="activeAction = null" />
</template>

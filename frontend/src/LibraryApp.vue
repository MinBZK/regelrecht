<script setup>
import { ref, computed, shallowRef } from 'vue';
import yaml from 'js-yaml';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';

const laws = ref([]);
const favorites = ref(null);
const loading = ref(true);
const indexError = ref(null);
const search = ref('');

const selectedLawPath = ref(null);
const selectedLaw = shallowRef(null);
const selectedLawLoading = ref(false);
const lawError = ref(null);
const selectedArticleNumber = ref(null);
const detailView = ref('machine');
const activeAction = ref(null);

const filteredLaws = computed(() => {
  let list = laws.value;
  if (favorites.value) {
    const favList = list.filter(law => favorites.value.has(law.id));
    if (favList.length > 0) list = favList;
  }
  const q = search.value.toLowerCase();
  if (!q) return list;
  return list.filter(law =>
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

/** Fetch GitHub tree and derive index entries from file paths. */
async function fetchGitHubIndex(source) {
  const { owner, repo, branch, path: basePath } = source.github;
  const treeUrl = `https://api.github.com/repos/${owner}/${repo}/git/trees/${branch}?recursive=1`;
  const res = await fetch(treeUrl);
  if (!res.ok) throw new Error(`GitHub tree API: ${res.status}`);
  const tree = await res.json();

  const prefix = basePath ? `${basePath}/` : '';
  const yamlEntries = tree.tree.filter(e =>
    e.type === 'blob' && e.path.startsWith(prefix) && e.path.endsWith('.yaml')
  );

  // Derive metadata from path: {prefix}{layer}/{law_id}/{date}.yaml
  const latestById = new Map();
  for (const entry of yamlEntries) {
    const rel = entry.path.slice(prefix.length); // e.g. "wet/wet_op_de_zorgtoeslag/2025-01-01.yaml"
    const parts = rel.split('/');
    if (parts.length < 3) continue;
    const layer = parts[0].toUpperCase();
    const lawId = parts[parts.length - 2];
    const date = parts[parts.length - 1].replace('.yaml', '');

    const existing = latestById.get(lawId);
    if (!existing || date > existing.date) {
      latestById.set(lawId, {
        id: lawId,
        name: lawId,
        regulatory_layer: layer,
        publication_date: date,
        date,
        path: `https://raw.githubusercontent.com/${owner}/${repo}/${branch}/${entry.path}`,
        source_id: source.id,
        source_name: source.name,
      });
    }
  }

  return [...latestById.values()];
}

async function loadIndex() {
  try {
    const [indexRes, favRes, registryRes] = await Promise.all([
      fetch('/data/index.json'),
      fetch('/favorites.json'),
      fetch('/corpus-registry.yaml'),
    ]);
    if (!indexRes.ok) throw new Error(`Failed to load index: ${indexRes.status}`);
    const localIndex = await indexRes.json();
    const localIds = new Set(localIndex.map(l => l.id));

    if (favRes.ok) {
      const favIds = await favRes.json();
      favorites.value = new Set(favIds);
    }

    // Fetch GitHub sources at runtime.
    let githubIndex = [];
    if (registryRes.ok) {
      const registryText = await registryRes.text();
      const registry = yaml.load(registryText);
      const githubSources = (registry.sources || [])
        .filter(s => s.type === 'github')
        .sort((a, b) => (a.priority || 0) - (b.priority || 0));
      for (const source of githubSources) {
        try {
          const entries = await fetchGitHubIndex(source);
          githubIndex.push(...entries.filter(e => !localIds.has(e.id)));
          console.log(`Loaded ${entries.length} laws from GitHub source "${source.id}"`);
        } catch (err) {
          console.warn(`Failed to load GitHub source "${source.id}":`, err.message);
        }
      }
    }

    laws.value = [...localIndex, ...githubIndex].sort((a, b) =>
      (a.regulatory_layer || '').localeCompare(b.regulatory_layer || '') || a.id.localeCompare(b.id)
    );

    let startList = laws.value;
    if (favorites.value) {
      const favList = laws.value.filter(l => favorites.value.has(l.id));
      if (favList.length > 0) startList = favList;
    }
    if (startList.length > 0 && !selectedLawPath.value) {
      selectLaw(startList[0].path);
    }
  } catch (e) {
    indexError.value = e;
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
    lawError.value = e;
  } finally {
    selectedLawLoading.value = false;
  }
}

function selectLaw(path) {
  selectedLawPath.value = path;
  selectedArticleNumber.value = null;
  activeAction.value = null;
  lawError.value = null;
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
            <div v-else-if="indexError" style="padding: 32px; text-align: center; color: #c00;">{{ indexError.message }}</div>
            <rr-list v-else variant="simple">
              <rr-list-item
                v-for="law in filteredLaws"
                :key="law.path"
                size="md"
                type="button"
                :selected="law.path === selectedLawPath || undefined"
                @click="selectLaw(law.path)"
              >
                <rr-text-cell>
                  <span slot="text">{{ displayName(law) }}</span>
                  <span slot="supporting-text" style="font-size: 11px; color: #888;">{{ law.source_name }}</span>
                </rr-text-cell>
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

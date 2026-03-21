<script setup>
import { ref, computed } from 'vue';

const laws = ref([]);
const loading = ref(true);
const error = ref(null);
const search = ref('');

const LAYER_LABELS = {
  WET: 'Wetten',
  MINISTERIELE_REGELING: 'Ministeriële regelingen',
  GEMEENTELIJKE_VERORDENING: 'Gemeentelijke verordeningen',
};

/** Format a $id into a readable name (replace underscores, title case). */
function formatId(id) {
  return id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

/** Display name: use name field if it's a real name, otherwise format $id. */
function displayName(law) {
  if (law.name && !law.name.startsWith('#')) return law.name;
  return formatId(law.id);
}

const filteredLaws = computed(() => {
  const q = search.value.toLowerCase();
  if (!q) return laws.value;
  return laws.value.filter(law =>
    law.id.toLowerCase().includes(q) ||
    displayName(law).toLowerCase().includes(q)
  );
});

const groupedLaws = computed(() => {
  const groups = {};
  for (const law of filteredLaws.value) {
    const layer = law.regulatory_layer;
    if (!groups[layer]) groups[layer] = [];
    groups[layer].push(law);
  }
  return groups;
});

async function load() {
  try {
    const res = await fetch('/data/index.json');
    if (!res.ok) throw new Error(`Failed to load index: ${res.status}`);
    laws.value = await res.json();
  } catch (e) {
    error.value = e;
  } finally {
    loading.value = false;
  }
}

load();
</script>

<template>
  <rr-page header-sticky>
    <!-- Header -->
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
            placeholder="Zoeken in wetten..."
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

    <!-- Loading -->
    <div v-if="loading" class="library-status">Laden...</div>

    <!-- Error -->
    <div v-else-if="error" class="library-status library-error">
      Kon de bibliotheek niet laden: {{ error.message }}
    </div>

    <!-- Law list -->
    <div v-else class="library-content">
      <div v-for="(layerLaws, layer) in groupedLaws" :key="layer" class="library-group">
        <h2 class="library-group-title">{{ LAYER_LABELS[layer] || layer }}</h2>
        <div class="library-grid">
          <a
            v-for="law in layerLaws"
            :key="law.path"
            :href="`editor.html?law=${law.path}`"
            class="library-card"
          >
            <div class="library-card-title">{{ displayName(law) }}</div>
            <div class="library-card-meta">
              <span class="library-card-id">{{ law.id }}</span>
              <span class="library-card-date">{{ law.publication_date }}</span>
            </div>
          </a>
        </div>
      </div>

      <div v-if="filteredLaws.length === 0" class="library-status">
        Geen wetten gevonden voor "{{ search }}"
      </div>
    </div>
  </rr-page>
</template>

<style scoped>
.library-status {
  padding: 48px;
  text-align: center;
  color: var(--rr-color-text-secondary, #666);
  font-size: 16px;
}
.library-error {
  color: #c00;
}
.library-content {
  padding: 24px 32px;
  max-width: 1200px;
  margin: 0 auto;
}
.library-group {
  margin-bottom: 32px;
}
.library-group-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0 0 12px 0;
  color: var(--rr-color-text-primary, #1a1a1a);
}
.library-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 12px;
}
.library-card {
  display: block;
  padding: 16px;
  border: 1px solid var(--rr-color-border, #ddd);
  border-radius: 8px;
  background: #fff;
  text-decoration: none;
  color: inherit;
  transition: border-color 0.15s, box-shadow 0.15s;
}
.library-card:hover {
  border-color: var(--rr-color-primary, #0066cc);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}
.library-card-title {
  font-size: 15px;
  font-weight: 500;
  margin-bottom: 6px;
  color: var(--rr-color-text-primary, #1a1a1a);
}
.library-card-meta {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  color: var(--rr-color-text-secondary, #666);
}
.library-card-id {
  font-family: monospace;
  font-size: 12px;
}
</style>

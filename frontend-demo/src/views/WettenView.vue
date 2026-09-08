<script setup>
import { computed, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import YamlNode from '../components/YamlNode.vue';
import OrgLogo from '../components/OrgLogo.vue';
import { useDemo } from '../store/demoStore.js';
import { serviceInfo } from '../data/loadCorpus.js';

// The law browser: every demo law in the sidebar, grouped by the organisation
// that executes it; opened laws as document tabs; the selected law as a
// collapsible YAML tree in which every `source.regulation` is a link that opens
// the referenced law in a new tab. That walk from zorgtoeslag to BRP to the
// penitentiaire beginselenwet is the point of this tab.

const route = useRoute();
const router = useRouter();
const { corpus, profile } = useDemo();

const MAX_TABS = 10;
const openTabs = reactive([]); // law ids
const activeId = ref(null);
const showRaw = ref(false);
const query = ref('');
const expandState = reactive({ paths: [], version: 0, all: null });

const lawIds = computed(() => new Set(corpus.value?.latestById.keys() ?? []));

/** Laws in the sidebar, honouring the profile's whitelist when it has one. */
const sidebarLaws = computed(() => {
  if (!corpus.value) return [];
  const all = [...corpus.value.latestById.values()];
  const allow = profile.value?.sidebar_laws;
  const filtered = allow ? all.filter((l) => allow.includes(l.law_path)) : all;
  const q = query.value.trim().toLowerCase();
  return q ? filtered.filter((l) => `${l.name} ${l.id} ${l.service}`.toLowerCase().includes(q)) : filtered;
});

const groups = computed(() => {
  const byService = new Map();
  for (const law of sidebarLaws.value) {
    const key = law.service ?? 'Overig';
    if (!byService.has(key)) byService.set(key, []);
    byService.get(key).push(law);
  }
  return [...byService.entries()]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([service, laws]) => ({ service, info: serviceInfo(corpus.value, service), laws: laws.sort((a, b) => a.name.localeCompare(b.name)) }));
});

const activeLaw = computed(() => (activeId.value ? corpus.value?.lawById(activeId.value) : null));

function expandedFor(law) {
  const cfg = corpus.value?.config?.expanded_paths ?? {};
  return cfg[law.id] ?? cfg[law.law_path] ?? [];
}

function openLaw(lawId, { replaceRoute = false } = {}) {
  if (!lawIds.value.has(lawId)) return;
  if (!openTabs.includes(lawId)) {
    openTabs.push(lawId);
    if (openTabs.length > MAX_TABS) openTabs.shift();
  }
  activeId.value = lawId;
  expandState.paths = expandedFor(corpus.value.lawById(lawId));
  expandState.all = null;
  expandState.version += 1;
  const target = `/wetten/${encodeURIComponent(lawId)}`;
  if (route.fullPath !== target) (replaceRoute ? router.replace : router.push).call(router, target);
}

function closeTab(lawId) {
  const i = openTabs.indexOf(lawId);
  if (i === -1) return;
  openTabs.splice(i, 1);
  if (activeId.value === lawId) {
    const next = openTabs[i] ?? openTabs[i - 1] ?? null;
    if (next) openLaw(next, { replaceRoute: true });
    else {
      activeId.value = null;
      router.replace('/wetten');
    }
  }
}

function closeAll() {
  openTabs.splice(0, openTabs.length);
  activeId.value = null;
  router.replace('/wetten');
}

function expandAll() {
  expandState.all = true;
  expandState.version += 1;
}
function collapseAll() {
  expandState.all = false;
  expandState.version += 1;
}
function resetExpansion() {
  expandState.all = null;
  expandState.paths = activeLaw.value ? expandedFor(activeLaw.value) : [];
  expandState.version += 1;
}

// Route → state (deep link, and the presentation's hand-off). The profile's
// default law opens when the tab is entered without one.
watch(
  () => [route.name, route.params.lawId, corpus.value, profile.value],
  ([name, lawId]) => {
    if (name !== 'wetten' || !corpus.value) return;
    if (lawId && typeof lawId === 'string') {
      openLaw(decodeURIComponent(lawId), { replaceRoute: true });
    } else if (!activeId.value && profile.value?.default_law) {
      const d = profile.value.default_law;
      const law = corpus.value.lawByPath(d.law_path, d.service);
      if (law) openLaw(law.id, { replaceRoute: true });
    }
  },
  { immediate: true },
);

function onTabChange(e) {
  const id = e.detail?.item?.dataset?.lawId;
  if (id) openLaw(id);
}
function onTabDismiss(e) {
  const id = e.detail?.item?.dataset?.lawId;
  if (id) closeTab(id);
}

function tabInfo(lawId) {
  return corpus.value?.lawById(lawId);
}

const references = computed(() => {
  // Laws this law depends on, for the inspector column.
  const law = activeLaw.value;
  if (!law) return [];
  const ids = new Set();
  const walk = (node) => {
    if (Array.isArray(node)) node.forEach(walk);
    else if (node && typeof node === 'object') {
      if (typeof node.regulation === 'string') ids.add(node.regulation);
      Object.values(node).forEach(walk);
    }
  };
  walk(law.doc.articles);
  return [...ids].map((id) => corpus.value.lawById(id)).filter(Boolean);
});

const referencedBy = computed(() => {
  const law = activeLaw.value;
  if (!law) return [];
  return [...corpus.value.latestById.values()].filter((other) => other.id !== law.id && other.text.includes(`regulation: ${law.id}\n`));
});
</script>

<template>
  <nldd-navigation-split-view sidebar-accessible-label="Wetten" inspector-accessible-label="Verwijzingen">
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <nldd-page sticky-header background="inherit">
        <nldd-container slot="header" padding="12" gap="8">
          <nldd-top-title-bar text="Wetten" :supporting-text="`${sidebarLaws.length} regelingen`"></nldd-top-title-bar>
          <nldd-search-field placeholder="Zoek een wet" size="sm" :value="query" @input="query = $event.detail?.value ?? $event.target.value"></nldd-search-field>
        </nldd-container>
        <nldd-container padding-inline="8" padding-bottom="16">
          <template v-for="group in groups" :key="group.service">
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-container layout="row" gap="8" vertical-alignment="center" padding-inline="8" padding-block="4">
            <OrgLogo :service="group.service" size="sm" />
            <nldd-text-cell size="sm" color="secondary" :text="group.info.name"></nldd-text-cell>
          </nldd-container>
          <nldd-list type="navigation" :accessible-label="group.info.name">
            <nldd-list-item
              v-for="law in group.laws"
              :key="law.id"
              size="sm"
              button
              :selected="law.id === activeId || undefined"
              @click="openLaw(law.id)"
            >
              <nldd-text-cell size="sm" :text="law.name" :supporting-text="law.law_path"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          </template>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="0">
          <nldd-document-tab-bar accessible-label="Geopende wetten" @tabchange="onTabChange" @tabdismiss="onTabDismiss">
            <nldd-document-tab-bar-item
              v-for="id in openTabs"
              :key="id"
              :data-law-id="id"
              :text="tabInfo(id)?.name ?? id"
              :supporting-text="tabInfo(id)?.service ?? ''"
              :selected="id === activeId || undefined"
            ></nldd-document-tab-bar-item>
            <nldd-icon-button slot="end" size="sm" variant="neutral-transparent" icon="dismiss" text="Alles sluiten" @click="closeAll"></nldd-icon-button>
          </nldd-document-tab-bar>
          <nldd-container v-if="activeLaw" padding="8">
            <nldd-toolbar size="sm">
              <nldd-toolbar-title slot="start" :text="activeLaw.name" :supporting-text="`${activeLaw.id} · geldig vanaf ${activeLaw.valid_from}`"></nldd-toolbar-title>
              <nldd-toolbar-item slot="end">
                <nldd-segmented-control size="sm" width="fit-content" :value="showRaw ? 'raw' : 'tree'" @change="showRaw = $event.detail?.value === 'raw'">
                  <nldd-segmented-control-item value="tree" text="Boom"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="raw" text="YAML"></nldd-segmented-control-item>
                </nldd-segmented-control>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end" v-if="!showRaw">
                <nldd-button-bar size="sm">
                  <nldd-icon-button icon="chevron-up-chevron-down" text="Standaardweergave" @click="resetExpansion"></nldd-icon-button>
                  <nldd-icon-button icon="chevron-down" text="Alles openvouwen" @click="expandAll"></nldd-icon-button>
                  <nldd-icon-button icon="chevron-up" text="Alles dichtvouwen" @click="collapseAll"></nldd-icon-button>
                </nldd-button-bar>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end">
                <nldd-button size="sm" variant="neutral-tinted" end-icon="external-link" text="wetten.overheid.nl" :href="activeLaw.doc.url" target="_blank"></nldd-button>
              </nldd-toolbar-item>
            </nldd-toolbar>
          </nldd-container>
        </nldd-container>

        <nldd-simple-section v-if="!activeLaw" height="60vh">
          <nldd-inline-dialog icon="books" text="Kies een wet" supporting-text="Kies links een regeling om de machine-leesbare wet te bekijken."></nldd-inline-dialog>
        </nldd-simple-section>
        <nldd-simple-section v-else width="full">
          <nldd-code-viewer v-if="showRaw" language="yaml" wrap>{{ activeLaw.text }}</nldd-code-viewer>
          <nldd-box v-else>
            <nldd-container padding="12">
              <div class="yaml-tree">
                <YamlNode :key="activeLaw.id" :value="activeLaw.doc" :expanded="expandState" :law-ids="lawIds" @open-law="openLaw" />
              </div>
            </nldd-container>
          </nldd-box>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="inspector" :has-content="!!activeLaw || undefined" background="tinted">
      <nldd-page v-if="activeLaw" background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar text="Verwijzingen"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="12" gap="16">
          <nldd-list variant="box" accessible-label="Uitgevoerd door">
            <nldd-list-item size="md">
              <nldd-cell><OrgLogo :service="activeLaw.service" /></nldd-cell>
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-text-cell overline="Uitgevoerd door" :text="serviceInfo(corpus, activeLaw.service).name"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-container padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Gebruikt gegevens uit"></nldd-text-cell></nldd-container>
<nldd-list variant="box" accessible-label="Gebruikt gegevens uit">
            <nldd-list-item v-if="references.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" text="Geen andere wetten"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-for="ref in references" :key="ref.id" size="sm" button @click="openLaw(ref.id)">
              <nldd-cell><OrgLogo :service="ref.service" size="sm" /></nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell size="sm" :text="ref.name" :supporting-text="ref.id"></nldd-text-cell>
              <nldd-icon-cell icon="chevron-right" size="16"></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-container padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Wordt gebruikt door"></nldd-text-cell></nldd-container>
<nldd-list variant="box" accessible-label="Wordt gebruikt door">
            <nldd-list-item v-if="referencedBy.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" text="Geen andere wetten"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-for="ref in referencedBy" :key="ref.id" size="sm" button @click="openLaw(ref.id)">
              <nldd-cell><OrgLogo :service="ref.service" size="sm" /></nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell size="sm" :text="ref.name" :supporting-text="ref.id"></nldd-text-cell>
              <nldd-icon-cell icon="chevron-right" size="16"></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>

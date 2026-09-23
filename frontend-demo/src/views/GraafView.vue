<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { VueFlow, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import GraphLawNode from '../components/graph/GraphLawNode.vue';
import GraphBoxNode from '../components/graph/GraphBoxNode.vue';
import GraphItemNode from '../components/graph/GraphItemNode.vue';
import OrgLogo from '../components/OrgLogo.vue';
import LawGroupTree from '../components/LawGroupTree.vue';
import { buildGraph, lawShape, neighbourhood } from '../graph/lawGraph.js';
import { fieldSpec, formatValue } from '../data/format.js';
import { lineageFromTrace } from '../data/lineage.js';
import { serviceInfo } from '../data/loadCorpus.js';
import { useDemo } from '../store/demoStore.js';
import { useLocalePath } from '../i18n/useLocalePath.js';
import { useI18n } from '../i18n/index.js';

// Naar een ander tabblad op naam, niet op pad: onder `/en/` leidt een
// letterlijk Nederlands pad de bezoeker ongemerkt het Nederlandse tabblad in.
const { goTo } = useLocalePath();
const { t } = useI18n();

// The dependency graph as the POC drew it: every selected law and its direct
// neighbours as a box with its register sources, its inputs from other laws
// and its outputs (the whole corpus is laid out, so switching to "Alles" adds
// laws around the ones already in view without moving them), an edge
// from each input to the output that supplies it, and on every item the value
// the engine found for the active persona. The profile chooses the laws that
// tell its story (`graph_laws`); the sidebar lets the presenter add or drop
// laws. The canvas is vue-flow, as in the editor: the design system has no
// graph component (documented exception).

const demo = useDemo();
const { corpus, profile, portalLaws, dataVersion } = demo;
// Same store id as the <VueFlow> below, otherwise fitView talks to a different instance.
const { fitView, zoomIn, zoomOut } = useVueFlow({ id: 'demo-graph' });
// The law list is a sheet, closed until asked for; the presets live in the toolbar.
const splitView = ref(null);

const allLaws = computed(() => (corpus.value ? [...corpus.value.latestById.values()].sort((a, b) => a.name.localeCompare(b.name)) : []));
const groups = computed(() => {
  const byService = new Map();
  for (const law of allLaws.value) {
    if (!byService.has(law.service)) byService.set(law.service, []);
    byService.get(law.service).push(law);
  }
  return [...byService.entries()].sort((a, b) => a[0].localeCompare(b[0])).map(([service, laws]) => ({ service, info: serviceInfo(corpus.value, service), laws }));
});

function referencesOf(law) {
  return lawShape(law).inputs.map((i) => i.ref.regulation).filter((id) => corpus.value.latestById.has(id));
}
/** A set of laws plus everything they read from, transitively. */
function withDependencies(laws) {
  const wanted = new Set();
  const queue = [...laws];
  while (queue.length) {
    const law = queue.shift();
    if (wanted.has(law.id)) continue;
    wanted.add(law.id);
    for (const ref of referencesOf(law)) queue.push(corpus.value.lawById(ref));
  }
  return wanted;
}

// ---- selection ----------------------------------------------------------------
const selected = ref(new Set());
const preset = ref('verhaal'); // 'verhaal' | 'portaal' | 'alles' | '' (hand-picked)

function applyPreset(name) {
  if (!corpus.value || !profile.value) return;
  preset.value = name;
  if (name === 'alles') {
    selected.value = new Set(allLaws.value.map((l) => l.id));
  } else if (name === 'portaal') {
    selected.value = withDependencies(portalLaws.value);
  } else {
    const story = (profile.value.graph_laws ?? []).filter((id) => corpus.value.latestById.has(id));
    if (story.length) selected.value = new Set(story);
    else {
      const d = profile.value.default_law;
      const law = d ? corpus.value.lawByPath(d.law_path, d.service) : null;
      selected.value = law ? withDependencies([law]) : new Set();
    }
  }
}
watch([profile, corpus], () => applyPreset('verhaal'), { immediate: true });

function toggle(lawId) {
  const next = new Set(selected.value);
  if (next.has(lawId)) next.delete(lawId);
  else next.add(lawId);
  selected.value = next;
  preset.value = '';
}
function only(lawId) {
  selected.value = new Set([lawId]);
  preset.value = '';
}
// As in the POC: the selected laws and everything directly connected to them.
const shownIds = computed(() => neighbourhood(selected.value, allLaws.value));
const shownLaws = computed(() => allLaws.value.filter((l) => shownIds.value.has(l.id)));

// ---- the persona's values ----------------------------------------------------
// Every shown law is evaluated for the active persona; the outputs go on the
// output items, the register values the trace resolved on the source items,
// and an input shows the value of the output it reads.
const values = computed(() => {
  void dataVersion.value;
  const outputs = {};
  const sources = {};
  const refs = new Map(); // `${law}#${output}` -> raw value, from cross-law references in traces
  const fmt = (lawId, name, v) => formatValue(v, fieldSpec(corpus.value.lawById(lawId)?.doc, name));
  for (const law of shownLaws.value) {
    let evaluation;
    try {
      evaluation = demo.evaluate(law);
    } catch {
      continue;
    }
    if (!evaluation?.ok) continue;
    outputs[law.id] = Object.fromEntries(Object.entries(evaluation.outputs ?? {}).map(([k, v]) => [k, fmt(law.id, k, v)]));
    const walk = (nodes) => {
      for (const n of nodes) {
        if (n.kind === 'value') {
          sources[n.law] ??= {};
          sources[n.law][n.name] = fmt(n.law, n.name, n.value);
        } else {
          refs.set(`${n.law}#${n.name}`, n.value);
          walk(n.children ?? []);
        }
      }
    };
    if (evaluation.trace) walk(lineageFromTrace(evaluation.trace, law.id, demo.personaParams()));
  }
  const inputs = {};
  for (const law of shownLaws.value) {
    for (const input of lawShape(law).inputs) {
      const key = `${input.ref.regulation}#${input.ref.output}`;
      const fromOutputs = outputs[input.ref.regulation]?.[input.ref.output];
      const v = fromOutputs ?? (refs.has(key) ? fmt(input.ref.regulation, input.ref.output, refs.get(key)) : undefined);
      if (v !== undefined) {
        inputs[law.id] ??= {};
        inputs[law.id][input.name] = v;
      }
    }
  }
  return { outputs, sources, inputs };
});

// ---- the graph ------------------------------------------------------------------
const focus = ref(null);
// Every law is laid out so the picture never shifts when "Alles" comes on;
// only the neighbourhood of the selection is visible.
const graph = computed(() => buildGraph(allLaws.value, values.value, focus.value, shownIds.value));

function onNodeClick({ node }) {
  if (node.type === 'law') focus.value = focus.value === node.id ? null : node.id;
  else if (node.type === 'item' && node.data.ref && selected.value.has(node.data.ref.regulation)) focus.value = node.data.ref.regulation;
}
function onNodeDoubleClick({ node }) {
  if (node.type === 'law') goTo('wetten', { lawId: node.id });
}
function onPaneClick() {
  focus.value = null;
}
function refit() {
  setTimeout(() => fitView({ padding: 0.1, nodes: [...shownIds.value] }), 50);
}
// vue-flow's fit-view-on-init runs before the nodes exist (the corpus arrives
// async); refit whenever the set of laws changes.
watch(() => [shownLaws.value.map((l) => l.id).join(), !!focus.value], async () => {
  await nextTick();
  refit();
});

const focusLaw = computed(() => (focus.value ? corpus.value?.lawById(focus.value) : null));
const uses = computed(() => (focus.value ? graph.value.edges.filter((e) => e.data.from === focus.value).map((e) => corpus.value.lawById(e.data.to)) : []));
const usedBy = computed(() => (focus.value ? graph.value.edges.filter((e) => e.data.to === focus.value).map((e) => corpus.value.lawById(e.data.from)) : []));
function unique(laws) {
  return [...new Map(laws.map((l) => [l.id, l])).values()];
}
</script>

<template>
  <nldd-navigation-split-view ref="splitView" primary-sidebar-as-sheet :primary-sidebar-accessible-label="t('wet.graph.sidebar.label')" :inspector-accessible-label="t('wet.graph.inspector.label')">
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <nldd-page sticky-header background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="t('wet.graph.sidebar.label')" :supporting-text="t('wet.graph.selection', { chosen: selected.size, shown: shownLaws.length })"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding-inline="8" padding-bottom="16">
          <LawGroupTree :groups="groups" mode="check" :checked="selected" :supporting-text="(law) => (!selected.has(law.id) && shownIds.has(law.id) ? t('wet.graph.neighbour') : undefined)" @toggle="toggle" />
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-item slot="start">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="books" :text="t('wet.sidebar.label')" :supporting-text="`${selected.size}`" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-title slot="start" :text="t('wet.graph.title')" :supporting-text="t('wet.graph.subtitle', { laws: shownLaws.length, edges: graph.edges.filter((e) => !e.hidden).length, persona: profile?.name ?? t('wet.graph.persona.fallback') })" max-width="480px"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end">
              <nldd-segmented-control size="sm" width="fit-content" :value="preset" @change="applyPreset($event.detail?.value)">
                <nldd-segmented-control-item value="verhaal" :text="t('wet.graph.preset.story')"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="portaal" :text="profile?.name ?? t('wet.graph.preset.portal')"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="alles" :text="t('wet.graph.preset.all')"></nldd-segmented-control-item>
              </nldd-segmented-control>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="binoculars" :text="t('wet.graph.fit')" @click="refit"></nldd-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
        <nldd-simple-section v-if="!shownLaws.length" height="60vh">
          <nldd-inline-dialog icon="centralized-network" :text="t('wet.graph.empty.title')" :supporting-text="t('wet.graph.empty.body')">
            <nldd-button slot="actions" variant="primary" size="sm" :text="t('wet.sidebar.label')" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>
        <div v-else class="graph-canvas">
          <VueFlow
            id="demo-graph"
            :nodes="graph.nodes"
            :edges="graph.edges"
            :min-zoom="0.1"
            :max-zoom="2"
            fit-view-on-init
            :nodes-connectable="false"
            :elevate-edges-on-select="false"
            @node-click="onNodeClick"
            @node-double-click="onNodeDoubleClick"
            @pane-click="onPaneClick"
            @nodes-initialized="refit"
          >
            <template #node-law="{ data }"><GraphLawNode :data="data" /></template>
            <template #node-box="{ data }"><GraphBoxNode :data="data" /></template>
            <template #node-item="{ data }"><GraphItemNode :data="data" /></template>
            <Background />
          </VueFlow>
          <!-- De zoomknoppen van vue-flow vervangen door knoppen uit het design
               system: die bracht zijn eigen stylesheet en eigen vormgeving mee,
               die naast de rest van de app stond en niets van het thema wist.
               `useVueFlow` levert dezelfde acties, dus alleen de knoppen zijn
               anders. -->
          <!-- Liggend, niet staand zoals vue-flow ze zette: nldd-button-bar is
               een horizontale groep en kent geen staande variant. Zelf rechtop
               zetten zou tegen het component in werken; dit is dezelfde bediening
               in de vorm die het design system ervoor heeft. -->
          <!-- `neutral-tinted` is de standaardvariant van de balk, en de enige
               die hier een zichtbare hover geeft. `neutral-base` zette de balk
               op wit met een hover van 0.974: dat verschil zie je niet, en
               omdat de knop geen rand heeft viel hij onder de muis weg in
               plaats van op te lichten. `neutral-transparent` is nog erger, die
               heeft helemaal geen hover-vulling. Getint geeft 0.923 in rust en
               0.898 onder de muis. -->
          <nldd-button-bar class="graph-zoom" variant="neutral-tinted">
            <nldd-icon-button icon="add" :text="t('wet.graph.zoom_in')" @click="zoomIn()"></nldd-icon-button>
            <nldd-icon-button icon="remove" :text="t('wet.graph.zoom_out')" @click="zoomOut()"></nldd-icon-button>
            <nldd-button-bar-divider></nldd-button-bar-divider>
            <!-- `refit()`, niet een kale `fitView()`: die laatste past ook de
                 verborgen wetten in beeld en zoomde dus verder uit dan nodig.
                 Doet nu hetzelfde als "Passend maken" in de werkbalk; die staat
                 er voor wie de balk leest, deze voor wie in het canvas bezig is
                 en zijn hand bij de zoomknoppen heeft. -->
            <nldd-icon-button icon="fit-to-view" :text="t('wet.graph.fit_all')" @click="refit()"></nldd-icon-button>
          </nldd-button-bar>
        </div>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane v-if="focusLaw" slot="inspector" has-content background="tinted">
      <nldd-page background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="focusLaw.name" :supporting-text="serviceInfo(corpus, focusLaw.service).name" :dismiss-text="t('wet.graph.close')" @dismiss="focus = null"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="12" gap="12">
          <nldd-button-group orientation="horizontal" size="sm">
            <nldd-button variant="secondary" size="sm" start-icon="book" :text="t('wet.graph.open_in_laws')" @click="goTo('wetten', { lawId: focusLaw.id })"></nldd-button>
            <nldd-button variant="neutral-tinted" size="sm" :text="t('wet.graph.only_this')" @click="only(focusLaw.id)"></nldd-button>
          </nldd-button-group>
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('wet.graph.reads_from') }}</nldd-text></nldd-container>
            <nldd-list variant="box-base" :accessible-label="t('wet.graph.reads_from')">
              <nldd-list-item v-if="uses.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('wet.graph.no_law_in_view')"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="l in unique(uses)" :key="l.id" size="sm" button @click="focus = l.id">
                <nldd-text-cell size="sm" :text="l.name" :supporting-text="serviceInfo(corpus, l.service).name"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('wet.graph.read_by') }}</nldd-text></nldd-container>
            <nldd-list variant="box-base" :accessible-label="t('wet.graph.read_by')">
              <nldd-list-item v-if="usedBy.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('wet.graph.no_law_in_view')"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="l in unique(usedBy)" :key="l.id" size="sm" button @click="focus = l.id">
                <nldd-text-cell size="sm" :text="l.name" :supporting-text="serviceInfo(corpus, l.service).name"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
